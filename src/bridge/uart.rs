use embassy_futures::select::{select, select3, Either, Either3};
use embassy_time::{Duration, Timer};
use embedded_io_async::{Read, Write};

use crate::bridge::channels::{BridgeChannels, Packet};
use embassy_rp::pio_programs::uart::{PioUartRx, PioUartTx};
pub enum UartEvent {
    BaudChange(u32),
}
pub trait BaudRateControl {
    fn change_baud_rate(&mut self, baud: u32);
}

impl<'d, PIO: embassy_rp::pio::Instance, const SM: usize> BaudRateControl
    for PioUartRx<'d, PIO, SM>
{
    fn change_baud_rate(&mut self, baud: u32) {
        PioUartRx::change_baud_rate(self, baud);
    }
}

impl<'d, PIO: embassy_rp::pio::Instance, const SM: usize> BaudRateControl
    for PioUartTx<'d, PIO, SM>
{
    fn change_baud_rate(&mut self, baud: u32) {
        PioUartTx::change_baud_rate(self, baud);
    }
}

pub async fn uart_bridge_supervisor<R, W>(
    mut rx: R,
    mut tx: W,
    channels: &'static BridgeChannels,
) where
    R: Read + BaudRateControl + 'static,
    W: Write + BaudRateControl + 'static,
{
    loop {
        match uart_task(&mut rx, &mut tx, channels).await {
            UartEvent::BaudChange(baud) => {
                rx.change_baud_rate(baud);
                tx.change_baud_rate(baud);
            }
        }
    }
}

pub async fn uart_task<R, W>(
    mut rx: R,
    mut tx: W,
    channels: &'static BridgeChannels,
) -> UartEvent
where
    R: Read,
    W: Write,
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
                        return UartEvent::BaudChange(baud_rate);
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

    match select(uart_rx, uart_tx).await {
        Either::First(event) => event,
        Either::Second(_) => unreachable!(),
    }
}
