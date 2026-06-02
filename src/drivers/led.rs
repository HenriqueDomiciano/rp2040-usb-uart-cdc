use cortex_m::asm::delay as delay_cycles;
use embassy_rp::gpio::Output;

pub struct SingleWs2812<'d> {
    pin: Output<'d>,
}

impl<'d> SingleWs2812<'d> {
    pub fn new(pin: Output<'d>) -> Self {
        Self { pin }
    }
    #[inline(always)]
    #[link_section = ".data"]
    pub fn write(&mut self, r: u8, g: u8, b: u8) {
        let colors = [g, r, b];

        const TIMER_BASE: u32 = 0x40054000;
        let timer_raw_l = (TIMER_BASE + 0x28) as *const u32;

        loop {
            let mut corrupted = false;
            let start_time = unsafe { timer_raw_l.read_volatile() };

            cortex_m::interrupt::free(|_| {
                for &byte in &colors {
                    for bit in (0..8).rev() {
                        let is_high = (byte >> bit) & 1 == 1;

                        if is_high {
                            self.pin.set_high();
                            delay_cycles(80);

                            self.pin.set_low();
                            delay_cycles(44);
                        } else {
                            self.pin.set_high();
                            delay_cycles(26);

                            self.pin.set_low();
                            delay_cycles(90);
                        }
                    }
                }
            });

            self.pin.set_low();

            let end_time = unsafe { timer_raw_l.read_volatile() };
            let elapsed_us = end_time.wrapping_sub(start_time);

            if elapsed_us > 38 {
                corrupted = true;
            }

            if !corrupted {
                break;
            }

            delay_cycles(7500); // 60us
        }
    }
}
