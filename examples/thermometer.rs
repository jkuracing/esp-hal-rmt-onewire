#![no_std]
#![no_main]
use embassy_futures::block_on;
use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::{clock::CpuClock, rmt::*, time::Rate};
use esp_hal_rmt_onewire::*;
use esp_println::println;

#[esp_hal::main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let delay = Delay::new();

    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80_u32))
        .unwrap()
        .into_async();
    let mut ow = OneWire::new(rmt.channel0, rmt.channel4, peripherals.GPIO6).unwrap();

    loop {
        println!("Resetting the bus");
        block_on(ow.reset()).unwrap();

        println!("Broadcasting a measure temperature command to all attached sensors");
        for a in [0xCC, 0x44] {
            block_on(ow.send_byte(a)).unwrap();
        }

        println!("Scanning the bus to retrieve the measured temperatures");
        block_on(search(&mut ow));

        println!("Waiting for 10 seconds");
        delay.delay_millis(10_000);
    }
}

// Temperature in C
#[derive(Ord, PartialOrd, PartialEq, Eq, Debug)]
pub struct Temperature(pub fixed::types::I12F4);

const CTOF_FACT: fixed::types::I12F4 = fixed::types::I12F4::lit("1.8");
const CTOF_OFF: fixed::types::I12F4 = fixed::types::I12F4::lit("32");

impl core::fmt::Display for Temperature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(f, "{}°F ({}°C)", self.0 * CTOF_FACT + CTOF_OFF, self.0)?;
        Ok(())
    }
}

pub async fn search<'a>(ow: &mut OneWire<'a>) {
    let mut search = Search::new();
    loop {
        match search.next(ow).await {
            Ok(address) => {
                println!("Reading device {:?}", address);
                ow.reset().await.unwrap();
                ow.send_byte(0x55).await.unwrap();
                ow.send_address(address).await.unwrap();
                ow.send_byte(0xBE).await.unwrap();
                let temp_low = ow
                    .exchange_byte(0xFF)
                    .await
                    .expect("failed to get low byte of temperature");
                let temp_high = ow
                    .exchange_byte(0xFF)
                    .await
                    .expect("failed to get high byte of temperature");
                let temp = fixed::types::I12F4::from_le_bytes([temp_low, temp_high]);
                println!("Temp is: {temp}");
            }
            Err(_) => {
                println!("End of search");
                return;
            }
        }
    }
}
