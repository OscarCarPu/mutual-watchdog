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

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
const MQTT_SERVER: &str = env!("MQTT_SERVER");
const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");
const TELEGRAM_API_TOKEN: &str = env!("TELEGRAM_API_TOKEN");
const TELEGRAM_CHAT_ID: &str = env!("TELEGRAM_CHAT_ID");
const PING_INTERVAL_SECS: &str = env!("PING_INTERVAL_SECS");
const CHECK_INTERVAL_SECS: &str = env!("CHECK_INTERVAL_SECS");
const TIMEOUT_SECS: &str = env!("TIMEOUT_SECS");

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

#[embassy_executor::task]
async fn create_mqtt_client() {}

#[embassy_executor::task]
async fn send_mqtt_ping() {}

#[embassy_executor::task]
async fn check_mqtt_ping() {}

#[embassy_executor::task]
async fn create_telegram_client() {}

#[embassy_executor::task]
async fn send_telegram_message() {}

#[embassy_executor::task]
async fn connect_wifi() {}
