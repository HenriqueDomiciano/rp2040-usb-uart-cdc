use core::f32;



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
    let mut ticker = Ticker::every(Duration::from_millis(30));
    let mut time: f32 = 0.0;
    let mut counter : u8 = 0; 
    loop {
            defmt::info!("LED tick");
            let sin_value = breathe(time);
            match counter%6 {
               0 => ws2812.write(0, sin_value, 0),
               1 => ws2812.write(sin_value/2, sin_value, 0),
               2 => ws2812.write(sin_value/2, sin_value, sin_value/2),
               3 => ws2812.write(sin_value, sin_value/2, 0),
               4 => ws2812.write(sin_value, sin_value/2, sin_value/2),
               5 => ws2812.write(sin_value/2, sin_value/2, sin_value),
               _ => ws2812.write(0, 0, sin_value), 
                
            }
            ws2812.write(0,sin_value,0);
            time += 0.1; 
            if time > f32::consts::TAU
            {
                counter += 1; 
                time = 0.0;
            }   
            ticker.next().await;
        }
}

