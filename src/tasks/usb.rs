use embassy_rp::{peripherals::USB, usb::Driver};
use embassy_usb::{UsbDevice, driver::EndpointError};

type MyUsbDriver = Driver<'static, USB>;
type MyUsbDevice = UsbDevice<'static, MyUsbDriver>;


#[embassy_executor::task]
pub async fn usb_task(mut usb: MyUsbDevice) -> ! {
    usb.run().await
}

#[allow(dead_code)]
struct Disconnected {

}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}