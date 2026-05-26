use embassy_futures::join::{join}; 

use embassy_rp::usb::Driver;

use embassy_rp::peripherals::USB;

use embassy_usb::class::cdc_acm::CdcAcmClass; 

use embedded_io_async::{Read, Write};

pub async fn run_bridge<R, W>(
    mut rx: R,
    mut tx: W,
    cdc: CdcAcmClass<'static, Driver<'static, USB>>,
)
where
    R: Read,
    W: Write
{
    let (mut tx_cdc, mut rx_cdc) = cdc.split();

    let usb_to_uart = async {
        let mut buff = [0u8;64]; 
        loop
        {
            let n = rx_cdc.read_packet(&mut buff).await.unwrap(); 
            tx.write(&buff[..n]).await.unwrap(); 
        }
    }; 
    let uart_to_usb = async {
        let mut buff = [0u8;64]; 
        loop
        {
            let n = rx.read(&mut buff).await.unwrap(); 
            tx_cdc.write_packet(&buff[..n]).await.unwrap(); 
        }
    }; 
    join(usb_to_uart,uart_to_usb).await;
}