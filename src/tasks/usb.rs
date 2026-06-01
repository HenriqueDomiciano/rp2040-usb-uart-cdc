use embassy_futures::join::join;
use embassy_rp::{peripherals::USB, usb::Driver};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_usb::{class::cdc_acm::CdcAcmClass, driver::EndpointError, UsbDevice};

use crate::bridge::channels::BridgeChannels;

type MyUsbDriver = Driver<'static, USB>;
type MyUsbDevice = UsbDevice<'static, MyUsbDriver>;

#[embassy_executor::task]
pub async fn usb_task(mut usb: MyUsbDevice) -> ! {
    usb.run().await
}

#[allow(dead_code)]
struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

#[embassy_executor::task(pool_size = 3)]
pub async fn usb_bridge_task(
    cdc: CdcAcmClass<'static, Driver<'static, USB>>,
    channels: &'static BridgeChannels,
) {
    let (mut tx_cdc, mut rx_cdc) = cdc.split();
    let mut last_baud = 115200;
    loop {
        // Wait for both sides to be connected
        rx_cdc.wait_connection().await;
        tx_cdc.wait_connection().await;
        defmt::info!("USB connected");
        let disconnect_signal: Signal<CriticalSectionRawMutex, ()> = Signal::new();
        let current_line_coding = tx_cdc.line_coding();
        let new_baud = current_line_coding.data_rate();

        if new_baud != last_baud {
            channels.baud_rate.send(new_baud).await;
            last_baud = new_baud;
        }
        let usb_rx = async {
            let mut buf = [0u8; 64];
            loop {
                match rx_cdc.read_packet(&mut buf).await {
                    Ok(n) if n > 0 => {
                        let mut data = [0u8; 64];
                        data[..n].copy_from_slice(&buf[..n]);
                        channels
                            .usb_to_uart
                            .send(crate::bridge::channels::Packet { data, len: n })
                            .await;
                    }
                    Err(_) => {
                        defmt::info!("USB RX disconnected");
                        disconnect_signal.signal(());
                        break;
                    }
                    _ => {}
                }
            }
        };

        let usb_tx = async {
            let mut buf = [0u8; 64];
            loop {
                let packet = embassy_futures::select::select(
                    channels.uart_to_usb.receive(),
                    disconnect_signal.wait(),
                )
                .await;

                match packet {
                    embassy_futures::select::Either::First(p) => {
                        let mut n = 0;
                        for i in 0..p.len {
                            buf[n] = p.data[i];
                            n += 1;
                        }
                        while n < 64 {
                            match channels.uart_to_usb.try_receive() {
                                Ok(p) => {
                                    for i in 0..p.len {
                                        if n >= 64 {
                                            break;
                                        }
                                        buf[n] = p.data[i];
                                        n += 1;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        match tx_cdc.write_packet(&buf[..n]).await {
                            Ok(_) => {}
                            Err(_) => {
                                defmt::info!("USB TX disconnected");
                                break;
                            }
                        }
                    }
                    embassy_futures::select::Either::Second(_) => {
                        defmt::info!("USB TX got disconnect signal");
                        break;
                    }
                }
            }
        };

        join(usb_rx, usb_tx).await;
        defmt::info!("USB disconnected, waiting for reconnect");
    }
}
