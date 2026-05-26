use core::f32;

use embassy_executor::Spawner;

use embassy_rp::gpio::Output;

use embassy_time::{Duration, Ticker};

use libm::pow;
use crate::drivers::led::SingleWs2812;

use {defmt_rtt as _, panic_probe as _};


fn breathe(t: f32) -> u8 {
    let value = pow(((libm::sinf(t) + 1.0) * 0.5) as f64, 2.0) * 60.0;
    value as u8
}

#[embassy_executor::task]
pub async fn ws2812_task(mut ws2812: SingleWs2812<'static>) {
    let mut ticker = Ticker::every(Duration::from_millis(40));
    let mut time: f32 = 0.0;
    loop {
            defmt::info!("LED tick");
            let sin_value = breathe(time);
            ws2812.write(0,sin_value,0);
            time += 0.2; 
            if time > f32::consts::TAU
            {
                time = 0.0;
            }   
            ticker.next().await;
        }
}

pub fn spawn(spawner: Spawner, pin:Output<'static>) 
{
    let led_struct = SingleWs2812::new(pin);
    let token = ws2812_task(led_struct).unwrap();
    spawner.spawn(token);
}
