use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

pub struct Packet {
    pub data: [u8; 64],
    pub len: usize,
}

pub struct BridgeChannels {
    pub usb_to_uart: Channel<CriticalSectionRawMutex, Packet, 4>,
    pub uart_to_usb: Channel<CriticalSectionRawMutex, Packet, 4>,
    pub baud_rate: Channel<CriticalSectionRawMutex, u32, 4>,
}

impl BridgeChannels {
    pub const fn new() -> Self {
        Self {
            usb_to_uart: Channel::new(),
            uart_to_usb: Channel::new(),
            baud_rate: Channel::new(),
        }
    }
}
