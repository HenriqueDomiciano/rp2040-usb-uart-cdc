use core::usize;

use embassy_futures::{
    join::join,
    select::{select, select3, Either, Either3},
};
use embassy_rp::{
    clocks::clk_sys_freq,
    pio::{Instance, StateMachine},
    pio_programs::clock_divider,
};
use embassy_time::{Duration, Timer};
use embedded_io_async::{Read, Write};
use fixed::{types::extra::U8, FixedU32};

use crate::bridge::channels::{BridgeChannels, Packet};

pub async fn uart_task<'d, PIO: Instance, const SM_RX: usize, const SM_TX: usize, R, W>(
    mut rx: R,
    mut tx: W,
    channels: &'static BridgeChannels,
    state_machine_rx: &mut StateMachine<'d, PIO, SM_RX>,
    state_machine_tx: &mut StateMachine<'d, PIO, SM_TX>,
) where
    R: Read + 'static,
    W: Write + 'static,
{
    let uart_rx = async {
        let mut data = [0u8; 64];
        loop {
            match rx.read(&mut data[0..1]).await {
                Ok(1) => {}
                _ => continue,
            }

            let mut n = 1;

            while n < 64 {
                let next_byte_or_timeout = select3(
                    rx.read(&mut data[n..n + 1]),
                    Timer::after(Duration::from_millis(2)),
                    channels.baud_rate.receive(),
                )
                .await;

                match next_byte_or_timeout {
                    Either3::First(Ok(1)) => {
                        n += 1;
                    }
                    Either3::Third(baud_rate) => {
                        change_baud_rate_of_pio(state_machine_tx, baud_rate, 8);
                        change_baud_rate_of_pio(state_machine_rx, baud_rate, 8);
                    }
                    _ => {
                        break;
                    }
                }
            }
            let mut packet_data = [0u8; 64];
            packet_data[..n].copy_from_slice(&data[..n]);
            channels
                .uart_to_usb
                .send(Packet {
                    data: packet_data,
                    len: n,
                })
                .await;
        }
    };

    let uart_tx = async {
        loop {
            let packet = channels.usb_to_uart.receive().await;
            tx.write_all(&packet.data[..packet.len]).await.ok();
        }
    };

    join(uart_rx, uart_tx).await;
}

fn change_baud_rate_of_pio<'d, PIO: Instance, const SM: usize>(
    state_machine: &mut StateMachine<'d, PIO, SM>,
    target_baud: u32,
    cycles_per_bit: u32,
) {
    state_machine.set_enable(false);
    state_machine.clear_fifos();
    state_machine.restart();

    let sys_clock = clk_sys_freq();
    let pio_freq = target_baud * cycles_per_bit;
    let div_estimated = sys_clock as f64 / pio_freq as f64;
    let clock_divider = FixedU32::<U8>::from_num(div_estimated);
    state_machine.set_clock_divider(clock_divider);
    state_machine.set_enable(true);
}
