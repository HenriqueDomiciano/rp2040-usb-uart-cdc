use embassy_futures::{join::join, select::{Either, select}};
use embassy_time::{Duration, Timer};
use embedded_io_async::{Read, Write};

use crate::bridge::channels::{BridgeChannels, Packet};

pub async fn uart_task<R, W>(mut rx: R, mut tx: W, channels: &'static BridgeChannels)
where
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
