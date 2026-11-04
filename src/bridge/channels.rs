use core::sync::atomic::AtomicU32;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use portable_atomic::AtomicBool;

pub struct Packet {
    pub data: [u8; 64],
    pub len: usize,
}

pub struct BridgeChannels {
    pub usb_to_uart: Channel<CriticalSectionRawMutex, Packet, 128>,
    pub uart_to_usb: Channel<CriticalSectionRawMutex, Packet, 128>,
    pub baud_rate: Channel<CriticalSectionRawMutex, u32, 4>,
    pub current_baud_rate: AtomicU32,
    pub dtr_signal: Signal<CriticalSectionRawMutex, bool>,
    pub dtr: AtomicBool,
}

impl BridgeChannels {
    pub const fn new(baud_rate: u32) -> Self {
        Self {
            usb_to_uart: Channel::new(),
            uart_to_usb: Channel::new(),
            baud_rate: Channel::new(),
            current_baud_rate: AtomicU32::new(baud_rate),
            dtr_signal: Signal::new(),
            dtr : AtomicBool::new(false)
        }
    }
}
