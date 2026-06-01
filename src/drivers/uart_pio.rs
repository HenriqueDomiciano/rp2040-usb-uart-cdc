use embassy_rp::gpio::AnyPin;
use embassy_rp::pio::{StateMachine, Instance, Common};
use embassy_rp::clocks::clk_sys_freq;
use embassy_rp::{Peri, Peripherals}; // Certifique-se de importar o trait Peripheral do embassy-rp
use embassy_rp::pio_programs::uart::{PioUartRx, PioUartTx, PioUartTxProgram};
use fixed::types::extra::U8;
use fixed::FixedU32;

// Trait que adiciona o método estendido ao driver do Embassy
pub trait PioUartExtTx<'d, PIO: Instance, const SM: usize> {
    fn set_baudrate(
        &mut self,
        new_baud: u32,
        cycles_per_bit: u32,
        common: &mut Common<'d, PIO>,
        sm: &mut StateMachine<'d, PIO, SM>,
        pin: Peri<'static,_>,
        program: &'d PioUartTxProgram<'d, PIO>,
    );
}

// Implementamos o Trait diretamente na estrutura do Embassy
impl<'d, PIO: Instance, const SM: usize> PioUartExtTx<'d, PIO, SM> for PioUartTx<'d, PIO, SM> {
    fn set_baudrate(
        &mut self,
        new_baud: u32,
        cycles_per_bit: u32,
        common: &mut Common<'d, PIO>,
        sm: &mut StateMachine<'d, PIO, SM>,
        pin:embassy_rp::Peri<'_, > ,
        program: &'d PioUartTxProgram<'d, PIO>,
    ) {
        // 1. Para o hardware temporariamente para evitar glitches
        sm.set_enable(false);

        // 2. Calcula e aplica o novo divisor de clock na StateMachine
        let pio_freq = new_baud * cycles_per_bit;
        let div = FixedU32::<U8>::from_num((clk_sys_freq() as f64) / (pio_freq as f64));
        sm.set_clock_divider(div);

        // 3. Reativa a StateMachine
        sm.set_enable(true);

        // 4. Recria o driver internamente e substitui o conteúdo atual (*self)
        // Isso é Safe Rust e limpa a configuração antiga instantaneamente
        *self = PioUartTx::new(new_baud, common, sm, pin, program);
    }
}

pub trait PioUartExtRx<'d, PIO: Instance, const SM: usize> {
    fn set_baudrate(
        &mut self,
        new_baud: u32,
        cycles_per_bit: u32,
        common: &mut Common<'d, PIO>,
        sm: &mut StateMachine<'d, PIO, SM>,
        pin: Peripherals,
        program: &'d PioUartTxProgram<'d, PIO>,
    );
}

// Implementamos o Trait diretamente na estrutura do Embassy
impl<'d, PIO: Instance, const SM: usize> PioUartExtRx<'d, PIO, SM> for PioUartRx<'d, PIO, SM> {
    fn set_baudrate(
        &mut self,
        new_baud: u32,
        cycles_per_bit: u32,
        common: &mut Common<'d, PIO>,
        sm: &mut StateMachine<'d, PIO, SM>,
        pin: Peripherals,
        program: &'d PioUartTxProgram<'d, PIO>,
    ) {
        // 1. Para o hardware temporariamente para evitar glitches
        sm.set_enable(false);

        // 2. Calcula e aplica o novo divisor de clock na StateMachine
        let pio_freq = new_baud * cycles_per_bit;
        let div = FixedU32::<U8>::from_num((clk_sys_freq() as f64) / (pio_freq as f64));
        sm.set_clock_divider(div);

        // 3. Reativa a StateMachine
        sm.set_enable(true);

        // 4. Recria o driver internamente e substitui o conteúdo atual (*self)
        // Isso é Safe Rust e limpa a configuração antiga instantaneamente
        *self = PioUartRx::new(new_baud, common, sm, pin, program);
    }
}
