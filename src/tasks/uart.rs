use embassy_rp::pio::StateMachine;
use embassy_rp::pio_programs::uart::{PioUartRx, PioUartTx};
use embassy_rp::peripherals::{
    PIO0, PIO1
};
 
use crate::bridge::channels::BridgeChannels;
use crate::bridge::uart::uart_task; 

#[embassy_executor::task]
pub async fn uart_bridge_task_pio_0_sm_2(
    rx: PioUartRx<'static, PIO0, 2>,
    tx: PioUartTx<'static, PIO0, 3>,
    channels: &'static BridgeChannels,
) {
    uart_task(
        rx,
        tx,
        channels
    ).await;
}
#[embassy_executor::task]
pub async fn uart_bridge_task_pio_0_sm_0(
    rx: PioUartRx<'static, PIO0, 0>,
    tx: PioUartTx<'static, PIO0, 1>,
    channels: &'static BridgeChannels,
    mut state_machine_rx: StateMachine<'static, PIO0, 0>,
    mut state_machine_tx:StateMachine<'static, PIO0, 1>
) {
    uart_task(
        rx,
        tx,
        channels,
        &mut state_machine_rx,
        &mut state_machine_tx
    ).await;
}

#[embassy_executor::task]
pub async fn uart_bridge_task_pio_1_sm_0(
    rx: PioUartRx<'static, PIO1, 0>,
    tx: PioUartTx<'static, PIO1, 1>,
    channels: &'static BridgeChannels,
) {
    uart_task(
        rx,
        tx,
        channels
    ).await;
}