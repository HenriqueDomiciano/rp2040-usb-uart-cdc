use core::sync::atomic::Ordering;

use embassy_futures::select::{select, select3};
use embassy_rp::{peripherals::USB, usb::Driver};
use embassy_usb::{class::cdc_acm::CdcAcmClass, driver::EndpointError, UsbDevice};

use crate::bridge::channels::BridgeChannels;

use embassy_usb::control::{InResponse, OutResponse, Recipient, Request, RequestType};
use embassy_usb::Handler;

pub struct TripleCdcControlHandler {
    bridge0: &'static BridgeChannels,
    bridge1: &'static BridgeChannels,
    bridge2: &'static BridgeChannels,
}

impl TripleCdcControlHandler {
    pub fn new(
        bridge0: &'static BridgeChannels,
        bridge1: &'static BridgeChannels,
        bridge2: &'static BridgeChannels,
    ) -> Self {
        Self {
            bridge0,
            bridge1,
            bridge2,
        }
    }
    fn bridge_for(&self, index: u16) -> Option<&'static BridgeChannels> {
        match index {
            0 => Some(self.bridge0),
            2 => Some(self.bridge1),
            4 => Some(self.bridge2),
            _ => None,
        }
    }
}

impl Handler for TripleCdcControlHandler {
    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        if req.request_type != RequestType::Class || req.recipient != Recipient::Interface {
            return None;
        }
        let bridge = self.bridge_for(req.index)?;
        match req.request {
            0x22 => {
                let dtr = (req.value & 0x01) != 0;
                bridge.dtr.store(dtr, Ordering::Release);
                bridge.dtr_signal.signal(dtr);
                None
            },
            0x20 if data.len() >= 7 => {
                let new_baud = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let _ = bridge.baud_rate.try_send(new_baud);
                bridge
                    .current_baud_rate
                    .store(new_baud, core::sync::atomic::Ordering::Relaxed);
                Some(OutResponse::Accepted)
            }
            _ => None,
        }
    }

    fn control_in<'a>(&'a mut self, req: Request, data: &'a mut [u8]) -> Option<InResponse<'a>> {
        if req.request == 0x21 && data.len() >= 7 {
            let mut baud: u32 = 115_200;
            match req.index {
                0 => {
                    defmt::info!("USB Port 0 -> Baud: {}", baud);
                    baud = self
                        .bridge0
                        .current_baud_rate
                        .load(core::sync::atomic::Ordering::Relaxed);
                }
                2 => {
                    defmt::info!("USB Port 1 -> Baud: {}", baud);
                    baud = self
                        .bridge1
                        .current_baud_rate
                        .load(core::sync::atomic::Ordering::Relaxed);
                }
                4 => {
                    defmt::info!("USB Port 2 -> Baud: {}", baud);
                    baud = self
                        .bridge2
                        .current_baud_rate
                        .load(core::sync::atomic::Ordering::Relaxed);
                }
                _ => {
                    defmt::debug!("Command 21 receicved on wrong interface: {}", req.index);
                }
            }
            data[0..4].copy_from_slice(&baud.to_le_bytes());
            data[4] = 0; // 1 stop bit
            data[5] = 0; // Parity: None
            data[6] = 8; // 8 data bits
            return Some(InResponse::Accepted(&data[..7]));
        }
        None
    }
}

#[embassy_executor::task]
pub async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    defmt::info!("USB Started");
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

    loop {
        channels.dtr_signal.reset();          // forget stale state from the last session
        rx_cdc.wait_connection().await;
        tx_cdc.wait_connection().await;
        defmt::info!("USB connected");
         while !channels.dtr.load(Ordering::Acquire) {
            let drain = async {
                loop {
                    let _ = channels.uart_to_usb.receive().await; // dropped
                }
            };
            select(drain, channels.dtr_signal.wait()).await;
        }

        // Start the session clean.
        channels.uart_to_usb.clear();
        channels.usb_to_uart.clear();

        defmt::info!("Port opened, waiting for data");

        let usb_rx = async {
            let mut buf = [0u8; 64];
            loop {
                match rx_cdc.read_packet(&mut buf).await {
                    Ok(n) if n > 0 => {
                        let mut data = [0u8; 64];
                        data[..n].copy_from_slice(&buf[..n]);
                        channels.usb_to_uart
                            .send(crate::bridge::channels::Packet { data, len: n })
                            .await;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        };

        let usb_tx = async {
            loop {
                let p = channels.uart_to_usb.receive().await;
                if tx_cdc.write_packet(&p.data[..p.len]).await.is_err() {
                    break;
                }
            }
        };

        let dtr_lost = async {
            while channels.dtr_signal.wait().await {}
        };

        select3(usb_rx, usb_tx, dtr_lost).await;

        channels.uart_to_usb.clear();
        channels.usb_to_uart.clear();
        defmt::info!("USB disconnected, waiting for reconnect");
    }
}
