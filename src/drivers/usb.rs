use embassy_rp::{peripherals::USB, usb::Driver, Peri};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    UsbDevice,
};
use static_cell::StaticCell;

use crate::{tasks::usb::TripleCdcControlHandler, Irqs, BRIDGE0, BRIDGE1, BRIDGE2};
#[allow(dead_code)]
pub struct UsbStack {
    pub usb: UsbDevice<'static, Driver<'static, USB>>,
    pub class0: CdcAcmClass<'static, Driver<'static, USB>>,
    pub class1: CdcAcmClass<'static, Driver<'static, USB>>,
    pub class2: CdcAcmClass<'static, Driver<'static, USB>>,
}

impl UsbStack {
    pub fn new(usb_peripheral: Peri<'static, USB>, irqs: Irqs) -> Self {
        let driver = Driver::new(usb_peripheral, irqs);
        let config = {
            let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
            config.manufacturer = Some("Henrique Domiciano");
            config.product = Some("USB-Serial-to-UART");
            config.serial_number = Some("12345678");
            config.max_power = 100;
            config.max_packet_size_0 = 64;
            config
        };
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 1024]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 512]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
        static TRIPLE_HANDLER: StaticCell<TripleCdcControlHandler> = StaticCell::new();
        let triple_handler =
            TRIPLE_HANDLER.init(TripleCdcControlHandler::new(&BRIDGE0, &BRIDGE1, &BRIDGE2));
        let mut builder = embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 1024]),
            BOS_DESCRIPTOR.init([0; 512]),
            &mut [],
            CONTROL_BUF.init([0; 64]),
        );
        builder.handler(triple_handler);

        let class0 = {
            static STATE: StaticCell<State> = StaticCell::new();
            let state = STATE.init(State::new());
            CdcAcmClass::new(&mut builder, state, 64)
        };

        let class1 = {
            static STATE: StaticCell<State> = StaticCell::new();
            let state = STATE.init(State::new());
            CdcAcmClass::new(&mut builder, state, 64)
        };

        let class2 = {
            static STATE: StaticCell<State> = StaticCell::new();
            let state = STATE.init(State::new());
            CdcAcmClass::new(&mut builder, state, 64)
        };
        let usb = builder.build();

        Self {
            usb,
            class0,
            class1,
            class2,
        }
    }
}
