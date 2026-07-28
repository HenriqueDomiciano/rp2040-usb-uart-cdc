use embassy_futures::join::join;
use embassy_rp::{peripherals::USB, usb::Driver};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_usb::{class::cdc_acm::CdcAcmClass, driver::EndpointError, UsbDevice};

use crate::bridge::channels::BridgeChannels;

use embassy_usb::control::{InResponse, OutResponse, Request};
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
}

impl Handler for TripleCdcControlHandler {
    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        if req.request == 0x20 && data.len() >= 4 {
            let new_baud = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

            match req.index {
                0 => {
                    defmt::info!("USB Port 0 -> New Baud: {}", new_baud);
                    let _ = self.bridge0.baud_rate.try_send(new_baud);
                    self.bridge0
                        .current_baud_rate
                        .store(new_baud, core::sync::atomic::Ordering::Relaxed);
                }
                2 => {
                    defmt::info!("USB Port 1 -> New Baud: {}", new_baud);
                    let _ = self.bridge1.baud_rate.try_send(new_baud);
                    self.bridge1
                        .current_baud_rate
                        .store(new_baud, core::sync::atomic::Ordering::Relaxed);
                }
                4 => {
                    defmt::info!("USB Port 2 -> New Baud: {}", new_baud);
                    let _ = self.bridge2.baud_rate.try_send(new_baud);
                    self.bridge2
                        .current_baud_rate
                        .store(new_baud, core::sync::atomic::Ordering::Relaxed);
                }
                _ => {
                    defmt::debug!("SET_LINE_CODING received on wrong interface: {}", req.index);
                }
            }
        }
        Some(OutResponse::Accepted)
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
    defmt::info!("USB CDC Started");
    let (mut tx_cdc, mut rx_cdc) = cdc.split();
    let disconnect_signal: Signal<CriticalSectionRawMutex, ()> = Signal::new();
    loop {
        defmt::info!("USB waiting connection");
        // Wait for both sides to be connected
        rx_cdc.wait_connection().await;
        tx_cdc.wait_connection().await;
        defmt::info!("USB connected");

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
