use defmt::info;
use embassy_futures::join::join;
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
                info!("Changing baud rate {:?}",baud); 
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
let uart_rx_data_flow = async {
    let mut data = [0u8; 64];
    loop {
        match rx.read(&mut data[0..1]).await {
            Ok(1) => {}
            _ => continue,
        }

        let mut n = 1;
        while n < 64 {
            let next_byte_or_timeout = embassy_futures::select::select(
                rx.read(&mut data[n..n + 1]),
                Timer::after(Duration::from_millis(2)),
            )
            .await;

            match next_byte_or_timeout {
                embassy_futures::select::Either::First(Ok(1)) => {
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
            .send(Packet { data: packet_data, len: n })
            .await;
    }
};

let uart_tx_flow = async {
    loop {
        let packet = channels.usb_to_uart.receive().await;
        tx.write_all(&packet.data[..packet.len]).await.ok();
    }
};

let total_data_flow = join(uart_rx_data_flow, uart_tx_flow);

match embassy_futures::select::select(channels.baud_rate.receive(), total_data_flow).await {
    embassy_futures::select::Either::First(baud_rate) => {
        info!("Received Baud rate: {:?}", baud_rate);
        UartEvent::BaudChange(baud_rate)
    }
    embassy_futures::select::Either::Second(_) => {
        unreachable!()
    }
}
}
