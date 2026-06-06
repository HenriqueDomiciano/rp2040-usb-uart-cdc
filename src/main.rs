#![no_std]
#![no_main]
use defmt::info;
use embassy_executor::{Executor, Spawner};
use {defmt_rtt as _, panic_probe as _};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::peripherals::DMA_CH1;
use embassy_rp::usb::InterruptHandler as UsbInterruptHandler;
use embassy_rp::{
    bind_interrupts,
    peripherals::{PIO0, PIO1, UART0, USB},
    pio::{InterruptHandler, Pio},
    pio_programs::uart::{PioUartRx, PioUartRxProgram, PioUartTx, PioUartTxProgram},
    uart::{BufferedInterruptHandler, Config},
    usb::Driver,
};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use static_cell::StaticCell;

use crate::{drivers::led::SingleWs2812, tasks::usb::TripleCdcControlHandler};
use crate::tasks::uart::{
    uart_bridge_task_pio_0_sm_0, uart_bridge_task_pio_0_sm_2, uart_bridge_task_pio_1_sm_0,
};
use crate::tasks::usb::usb_bridge_task;

use {defmt_rtt as _, panic_probe as _};
mod bridge;
mod drivers;
mod tasks;
use bridge::channels::BridgeChannels;

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

static BRIDGE0: BridgeChannels = BridgeChannels::new();
static BRIDGE1: BridgeChannels = BridgeChannels::new();
static BRIDGE2: BridgeChannels = BridgeChannels::new();

fn enable_pullup(pin: u32) {
    unsafe {
        const PAD_BANK0_BASE: u32 = 0x4001c000;

        *((PAD_BANK0_BASE + (pin * 4) + 4) as *mut u32) |= 1 << 3;
    }
}
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    PIO1_IRQ_0 => InterruptHandler<PIO1>;
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH1>;
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Started");

    let led_output = Output::new(p.PIN_16, Level::Low);
    let led = SingleWs2812::new(led_output);
    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                spawner.spawn(tasks::led::ws2812_task(led).unwrap());
            });
        },
    );
    let driver = Driver::new(p.USB, Irqs);
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
    let triple_handler = TRIPLE_HANDLER.init(TripleCdcControlHandler::new(
        &BRIDGE0,
        &BRIDGE1,
        &BRIDGE2,
    ));
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
    let _default_uart_config = Config::default();

    let Pio {
        mut common,
        sm0,
        sm1,
        sm2,
        sm3,
        ..
    } = Pio::new(p.PIO0, Irqs);

    let Pio {
        common: mut common_pio1,
        sm0: sm0_pio1,
        sm1: sm1_pio1,
        sm2: _sm2_pio1,
        ..
    } = Pio::new(p.PIO1, Irqs);

    enable_pullup(27);
    enable_pullup(28);
    enable_pullup(12);
    let tx_prg = PioUartTxProgram::new(&mut common);
    let rx_prg = PioUartRxProgram::new(&mut common);
    let tx_prg_pio1 = PioUartTxProgram::new(&mut common_pio1);
    let rx_prg_pio1 = PioUartRxProgram::new(&mut common_pio1);

    let uart1_tx = PioUartTx::new(115_200, &mut common, sm1, p.PIN_2, &tx_prg);
    let uart1_rx = PioUartRx::new(115_200, &mut common, sm0, p.PIN_27, &rx_prg);

    let uart2_tx = PioUartTx::new(115_200, &mut common, sm3, p.PIN_1, &tx_prg);
    let uart2_rx = PioUartRx::new(115_200, &mut common, sm2, p.PIN_28, &rx_prg);

    let uart3_tx = PioUartTx::new(115_200, &mut common_pio1, sm1_pio1, p.PIN_11, &tx_prg_pio1);
    let uart3_rx = PioUartRx::new(115_200, &mut common_pio1, sm0_pio1, p.PIN_12, &rx_prg_pio1);
    info!("Started USB task!!!"); 
    spawner.spawn(tasks::usb::usb_task(usb).expect("usb_task spawn failed"));
    info!("Started USB bridge 0");
    spawner.spawn(usb_bridge_task(class0, &BRIDGE0).unwrap());
    spawner.spawn(uart_bridge_task_pio_0_sm_0(uart1_rx, uart1_tx, &BRIDGE0).unwrap());
    info!("Started USB bridge 1");
    spawner.spawn(usb_bridge_task(class1, &BRIDGE1).unwrap());
    spawner.spawn(uart_bridge_task_pio_0_sm_2(uart2_rx, uart2_tx, &BRIDGE1).unwrap());
    info!("Started USB bridge 2");
    spawner.spawn(usb_bridge_task(class2, &BRIDGE2).unwrap());
    spawner.spawn(uart_bridge_task_pio_1_sm_0(uart3_rx, uart3_tx, &BRIDGE2).unwrap());
    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}
