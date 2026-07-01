use embassy_rp::{
    pio::{Common, Instance, PioPin, StateMachine},
    pio_programs::uart::{PioUartRx, PioUartRxProgram, PioUartTx, PioUartTxProgram},
    Peri,
};

pub struct PioUart<RX, TX> {
    pub rx: RX,
    pub tx: TX,
}

impl<P, const RX_SM: usize, const TX_SM: usize>
    PioUart<PioUartRx<'static, P, RX_SM>, PioUartTx<'static, P, TX_SM>>
where
    P: Instance,
{
    pub fn new(
        baud: u32,
        common: &mut Common<'static, P>,
        rx_sm: StateMachine<'static, P, RX_SM>,
        tx_sm: StateMachine<'static, P, TX_SM>,
        rx_pin: Peri<'static, impl PioPin>,
        tx_pin: Peri<'static, impl PioPin>,
    ) -> Self {
        let tx_prg = PioUartTxProgram::new(common);
        let rx_prg = PioUartRxProgram::new(common);
        Self {
            rx: PioUartRx::new(baud, common, rx_sm, rx_pin, &rx_prg),
            tx: PioUartTx::new(baud, common, tx_sm, tx_pin, &tx_prg),
        }
    }
}
