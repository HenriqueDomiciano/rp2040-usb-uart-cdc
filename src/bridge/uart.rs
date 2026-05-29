use embassy_futures::{
    join::join,
    select::{select, Either},
};
use embassy_rp::clocks::clk_sys_freq;
use embassy_time::{Duration, Timer};
use embedded_io_async::{Read, Write};
use crate::bridge::channels::{BridgeChannels, Packet};

pub async fn uart_task<R, W>(
    mut rx: R,
    mut tx: W,
    channels: &'static BridgeChannels,
    pio_index: usize,
    sm_rx: usize,
    sm_tx: usize,
) where
    R: Read + 'static,
    W: Write + 'static,
{
    let uart_rx = async {
        let mut data = [0u8; 64];

        loop {
            if let Some(new_baud) = channels.baud_rate.try_take() 
            {
                defmt::info!("UART Rx Baud Rate changed to {}",new_baud);
                set_pio_baud(pio_index, sm_rx, new_baud);
                set_pio_baud(pio_index, sm_tx, new_baud);
            }
            
            match select(rx.read(&mut data[0..1]), channels.baud_rate.wait(),).await
            {
                Either::First(Ok(1)) => {}
                Either::Second(new_baud) => {
                    set_pio_baud(pio_index, sm_rx, new_baud);
                    set_pio_baud(pio_index, sm_tx, new_baud);
                    continue;
                }
                _ => continue,
            }

            let mut n = 1;

            while n < 64 {
                let next_byte_or_timeout = select(
                    rx.read(&mut data[n..n + 1]),
                    Timer::after(Duration::from_millis(2)),
                )
                .await;

                match next_byte_or_timeout {
                    Either::First(Ok(1)) => {
                        n += 1;
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

fn set_pio_baud(pio_index: usize, sm: usize, baud: u32) {
    let  baud = baud.clamp(300,921600); 
    let div = clk_sys_freq() / (8 * baud);
    let div_int = (div as u16).max(1);

    match pio_index {
        0 => rp_pac::PIO0.sm(sm).clkdiv().write(|w| {
            w.set_int(div_int);
            w.set_frac(0);
        }),
        1 => rp_pac::PIO1.sm(sm).clkdiv().write(|w| {
            w.set_int(div_int);
            w.set_frac(0);
        }),
        _ => {}
    }
}
