#![no_std]
#![no_main]

mod bridge;
mod drivers;
mod tasks;

use defmt::info;
use embassy_executor::{Executor, Spawner};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::peripherals::DMA_CH1;
use embassy_rp::usb::InterruptHandler as UsbInterruptHandler;
use embassy_rp::{
    bind_interrupts,
    peripherals::{PIO0, PIO1, UART0, USB},
    pio::{InterruptHandler, Pio},
    uart::BufferedInterruptHandler,
};
use static_cell::StaticCell;

use crate::bridge::channels::BridgeChannels;
use crate::drivers::led::SingleWs2812;
use crate::drivers::uart::PioUart;
use crate::drivers::usb::UsbStack;
use crate::tasks::uart::{
    uart_bridge_task_pio_0_sm_0, uart_bridge_task_pio_0_sm_2, uart_bridge_task_pio_1_sm_0,
};
use crate::tasks::usb::usb_bridge_task;

use {defmt_rtt as _, panic_probe as _};

const STARTING_BAUD_RATE: u32 = 115_200;

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

static BRIDGE0: BridgeChannels = BridgeChannels::new(STARTING_BAUD_RATE);
static BRIDGE1: BridgeChannels = BridgeChannels::new(STARTING_BAUD_RATE);
static BRIDGE2: BridgeChannels = BridgeChannels::new(STARTING_BAUD_RATE);

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
    defmt::info!("Started");
    #[cfg(feature = "rp-2040-zero")]
    {
        use crate::tasks::led::ws2812_task;
        let led_output = Output::new(p.PIN_16, Level::Low);
        let led = SingleWs2812::new(led_output);
        spawn_core1(
            p.CORE1,
            unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
            move || {
                let executor1 = EXECUTOR1.init(Executor::new());
                executor1.run(|spawner| {
                    spawner.spawn(ws2812_task(led).unwrap());
                });
            },
        );
    }
    #[cfg(feature = "rp-2040-board-dev")]
    {
        use crate::tasks::led::blink_led;
        let led_output = Output::new(p.PIN_25, Level::Low);
        spawner.spawn(blink_led(led_output).unwrap());
    }

    let usb_controllers = UsbStack::new(p.USB, Irqs);

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
    enable_pullup(11);
    let uart1 = PioUart::new(115_200, &mut common, sm0, sm1, p.PIN_27, p.PIN_2);
    let uart2 = PioUart::new(115_200, &mut common, sm2, sm3, p.PIN_28, p.PIN_1);
    let uart3 = PioUart::new(
        115_200,
        &mut common_pio1,
        sm0_pio1,
        sm1_pio1,
        p.PIN_11,
        p.PIN_12,
    );

    info!("Started USB task!!!");
    spawner.spawn(tasks::usb::usb_task(usb_controllers.usb).expect("usb_task spawn failed"));
    info!("Started USB bridge 0");
    spawner.spawn(usb_bridge_task(usb_controllers.class0, &BRIDGE0).unwrap());
    spawner.spawn(uart_bridge_task_pio_0_sm_0(uart1.rx, uart1.tx, &BRIDGE0).unwrap());
    info!("Started USB bridge 1");
    spawner.spawn(usb_bridge_task(usb_controllers.class1, &BRIDGE1).unwrap());
    spawner.spawn(uart_bridge_task_pio_0_sm_2(uart2.rx, uart2.tx, &BRIDGE1).unwrap());
    info!("Started USB bridge 2");
    spawner.spawn(usb_bridge_task(usb_controllers.class2, &BRIDGE2).unwrap());
    spawner.spawn(uart_bridge_task_pio_1_sm_0(uart3.rx, uart3.tx, &BRIDGE2).unwrap());
    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}
