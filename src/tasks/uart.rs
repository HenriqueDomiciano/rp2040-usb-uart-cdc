use embassy_rp::pio_programs::uart::{PioUartRx, PioUartTx};
use embassy_rp::uart::{
    Async, Blocking, BufferedUartRx, BufferedUartTx, UartRx, UartTx
};
use embassy_rp::peripherals::{
    PIO0, PIO1, USB
};
use embassy_rp::usb::Driver;
use embassy_usb::class::cdc_acm::CdcAcmClass; 
use crate::bridge::uart::run_bridge; 

#[embassy_executor::task]
pub async fn uart_bridge_task(
    rx: BufferedUartRx,
    tx: BufferedUartTx,
    cdc: CdcAcmClass<'static, Driver<'static, USB>>,
) {
    run_bridge(
        rx,
        tx,
        cdc
    ).await;
}
#[embassy_executor::task]
pub async fn uart_bridge_task_pio_0_sm_0(
    rx: PioUartRx<'static, PIO0, 0>,
    tx: PioUartTx<'static, PIO0, 1>,
    cdc: CdcAcmClass<'static, Driver<'static, USB>>,
) {
    run_bridge(
        rx,
        tx,
        cdc
    ).await;
}

#[embassy_executor::task]
pub async fn uart_bridge_task_pio_0_sm_2(
    rx: PioUartRx<'static, PIO0, 2>,
    tx: PioUartTx<'static, PIO0, 3>,
    cdc: CdcAcmClass<'static, Driver<'static, USB>>,
) {
    run_bridge(
        rx,
        tx,
        cdc
    ).await;
}

#[embassy_executor::task]
pub async fn uart_bridge_task_pio_1_sm_0(
    rx: PioUartRx<'static, PIO1, 0>,
    tx: PioUartTx<'static, PIO1, 1>,
    cdc: CdcAcmClass<'static, Driver<'static, USB>>,
) {
    run_bridge(
        rx,
        tx,
        cdc
    ).await;
}
