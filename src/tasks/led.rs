use core::f32;

use defmt::info;
use embassy_rp::gpio::Output;
use embassy_time::{Duration, Ticker, Timer};

use crate::drivers::led::SingleWs2812;
use libm::pow;

use {defmt_rtt as _, panic_probe as _};

fn breathe(t: f32) -> u8 {
    let value = pow(((libm::sinf(t) + 1.0) * 0.5) as f64, 2.0) * 60.0;
    value as u8
}

#[embassy_executor::task]
pub async fn ws2812_task(mut ws2812: SingleWs2812<'static>) {
    info!("Starting LED breathing task");
    let mut ticker = Ticker::every(Duration::from_millis(30));
    let mut time: f32 = 0.0;
    loop {
        defmt::info!("LED tick");
        let sin_value = breathe(time);
        ws2812.write(0, sin_value, 0);
        time += 0.1;
        if time > f32::consts::TAU {
            time = 0.0;
        }
        ticker.next().await;
    }
}

#[embassy_executor::task]
pub async fn blink_led(mut pin: Output<'static>) {
    const LED_BLINK_TIME: u64 = 500;
    info!("Starting LED blink task");
    loop {
        pin.set_high();
        Timer::after(Duration::from_millis(LED_BLINK_TIME)).await;
        pin.set_low();
        Timer::after(Duration::from_millis(LED_BLINK_TIME)).await;
    }
}
