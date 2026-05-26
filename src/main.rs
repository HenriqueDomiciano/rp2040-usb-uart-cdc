#![no_std]
#![no_main]

use defmt::expect;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::DMA_CH1;
use embassy_rp::usb::InterruptHandler as UsbInterruptHandler;
use embassy_rp::{
    bind_interrupts,
    peripherals::{DMA_CH0, PIO0, PIO1, UART0, USB},
    pio::{InterruptHandler, Pio},
    pio_programs::{
        uart::{PioUartRx, PioUartRxProgram, PioUartTx, PioUartTxProgram},
        ws2812::{PioWs2812, PioWs2812Program},
    },
    uart::{BufferedInterruptHandler, BufferedUart, Config},
    usb::Driver,
};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder,
};
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};
mod bridge;
mod tasks;
mod drivers;

#[link_section = ".uninit"]
static mut _STACK: [u8; 32768] = [0u8; 32768];

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


    let output = Output::new(p.PIN_16, Level::Low); 
    tasks::led::spawn(spawner, output);

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
    let mut builder = embassy_usb::Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 1024]),
        BOS_DESCRIPTOR.init([0; 512]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );

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
    let default_uart_config = Config::default();

    let Pio {
        mut common,
        sm0,
        sm1,
        sm2,
        sm3,
        ..
    } = Pio::new(p.PIO0, Irqs);

    let Pio {
        common: mut common_pio1, sm0:sm0_pio1,sm1:sm1_pio1, ..
    } = Pio::new(p.PIO1, Irqs);
    defmt::info!("PIO 0 OK!!!");

    let tx_prg = PioUartTxProgram::new(&mut common);
    let rx_prg = PioUartRxProgram::new(&mut common);
    let tx_prg_pio1 = PioUartTxProgram::new(&mut common_pio1);
    let rx_prg_pio1 = PioUartRxProgram::new(&mut common_pio1);

    let uart1_tx = PioUartTx::new(115_200, &mut common, sm1, p.PIN_2, &tx_prg);
    let uart1_rx = PioUartRx::new(115_200, &mut common, sm0, p.PIN_3, &rx_prg);

    let uart2_tx = PioUartTx::new(115_200, &mut common, sm3, p.PIN_4, &tx_prg);
    let uart2_rx = PioUartRx::new(115_200, &mut common, sm2, p.PIN_5, &rx_prg);

    let uart3_tx = PioUartTx::new(115_200, &mut common_pio1 , sm1_pio1, p.PIN_6, &tx_prg_pio1);
    let uart3_rx = PioUartRx::new(115_200, &mut common_pio1, sm0_pio1, p.PIN_7, &rx_prg_pio1);


    spawner.spawn(tasks::usb::usb_task(usb).expect("usb_task spawn failed"));
    spawner.spawn(
        tasks::uart::uart_bridge_task_pio_0_sm_0(uart1_rx, uart1_tx, class0)
            .expect("uart_pio_0 spawn failed"),
    );
    spawner.spawn(
        tasks::uart::uart_bridge_task_pio_0_sm_2(uart2_rx, uart2_tx, class1)
            .expect("uart_pio_2 spawn failed"),
    );
    spawner.spawn(
        tasks::uart::uart_bridge_task_pio_1_sm_0(uart3_rx, uart3_tx, class2)
            .expect("uart_pio_2 spawn failed"),
    );
    //spawner
    //    .spawn(tasks::uart::uart_bridge_task(uart_rx, uart_tx, class2).expect("uart spawn failed"));
    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}
