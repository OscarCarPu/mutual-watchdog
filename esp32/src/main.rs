#![no_std]
#![no_main]

use core::time::Duration;

use embassy_executor::Spawner;
use embassy_net::{Ipv4Address, Runner, Stack, StackResources, tcp::TcpSocket};
use embassy_time::Timer;
use esp_alloc as _;
use esp_backtrace as _;
#[cfg(feature = "esp32c3")]
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::{
    clock::CpuClock,
    rng::Rng,
    rtc_cntl::{Rtc, sleep::TimerWakeupSource},
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_radio::{
    Controller,
    wifi::{ClientConfig, ModeConfig, WifiController, WifiDevice, WifiEvent, WifiStaState},
};

esp_bootloader_esp_idf::esp_app_desc!();

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

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
async fn main(spawner: Spawner) -> ! {
    // peripherals
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    #[cfg(feature = "esp32")]
    esp_rtos::start(timg0.timer0);
    #[cfg(feature = "esp32c3")]
    {
        let sw_ints = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
        esp_rtos::start(timg0.timer0, sw_ints.software_interrupt0);
    }

    // WiFi
    let stack = setup_wifi(&spawner, peripherals.WIFI).await;

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after_millis(500).await;
    }
    println!("WiFi link up!");

    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    stack
        .config_v4()
        .inspect(|c| println!("IPv4 config: {c:?}"));

    // Pinging
    ping_server(stack).await;

    // deep sleep
    let mut rtc = Rtc::new(peripherals.LPWR);
    let timer = TimerWakeupSource::new(Duration::from_secs(15));
    rtc.sleep_deep(&[&timer]);
}

async fn setup_wifi(
    spawner: &Spawner,
    wifi: esp_hal::peripherals::WIFI<'static>,
) -> Stack<'static> {
    let esp_radio_ctrl = &*mk_static!(Controller<'static>, esp_radio::init().unwrap());

    let (controller, interfaces) =
        esp_radio::wifi::new(esp_radio_ctrl, wifi, Default::default()).unwrap();

    let device = interfaces.sta;

    let net_config = embassy_net::Config::dhcpv4(Default::default());

    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let (stack, runner) = embassy_net::new(
        device,
        net_config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    spawner.spawn(connect_wifi(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    stack
}

#[embassy_executor::task]
async fn connect_wifi(mut controller: WifiController<'static>) {
    println!("Device capabilities: {:?}", controller.capabilities());

    loop {
        match esp_radio::wifi::sta_state() {
            WifiStaState::Connected => {
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after_millis(5000).await;
            }
            _ => {}
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = ModeConfig::Client(
                ClientConfig::default()
                    .with_ssid(WIFI_SSID.try_into().unwrap())
                    .with_password(WIFI_PASSWORD.try_into().unwrap()),
            );
            controller.set_config(&client_config).unwrap();
            println!("Starting WiFi...");
            controller.start_async().await.unwrap();
            println!("WiFi started!");
        }

        println!("Connecting to '{}'...", WIFI_SSID);
        match controller.connect_async().await {
            Ok(_) => println!("WiFi connected!"),
            Err(e) => {
                println!("WiFi connect failed: {:?}", e);
                Timer::after_millis(5000).await;
            }
        }
    }
}

async fn ping_server(stack: Stack<'static>) {
    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 1024];

    let ip = Ipv4Address::new(192, 168, 1, 132);
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(5)));

    match socket.connect((ip, 8001)).await {
        Ok(_) => println!("Ping {} OK", ip),
        Err(e) => println!("Ping {} failed: {:?}", ip, e),
    }
    socket.close();
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

#[embassy_executor::task]
async fn create_mqtt_client() {}

#[embassy_executor::task]
async fn send_mqtt_ping() {}

#[embassy_executor::task]
async fn create_telegram_client() {}

#[embassy_executor::task]
async fn send_telegram_message() {}
