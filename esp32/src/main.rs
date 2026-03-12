#![no_std]
#![no_main]
use esp_backtrace as _;
use esp_bootloader_esp_idf as _;
esp_bootloader_esp_idf::esp_app_desc!();
use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    loop {
        println!("Hello, world!");
        Timer::after_millis(1000).await;
    }
}
