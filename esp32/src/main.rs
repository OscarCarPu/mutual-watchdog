#![no_std]
#![no_main]

extern crate alloc;

use core::time::Duration;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use embassy_executor::Spawner;
use embassy_net::{dns::DnsQueryType, Ipv4Address, Runner, Stack, StackResources, tcp::TcpSocket};
use embassy_time::Timer;
use embedded_io_async::{Read as _, Write as _};
use embedded_tls::{Aes128GcmSha256, NoVerify, TlsConfig, TlsConnection, TlsContext};
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
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

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

    // Send Telegram message
    send_telegram_message(stack, "hello world").await;

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

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

async fn create_mqtt_client() {}

async fn send_mqtt_ping() {}

async fn send_telegram_message(stack: Stack<'static>, message: &str) {
    // DNS resolve api.telegram.org
    let ip_addr = match stack
        .dns_query("api.telegram.org", DnsQueryType::A)
        .await
    {
        Ok(addrs) => addrs[0],
        Err(e) => {
            println!("DNS query failed: {:?}", e);
            return;
        }
    };

    let remote_ip = match ip_addr {
        embassy_net::IpAddress::Ipv4(ip) => ip,
        _ => {
            println!("Expected IPv4 address");
            return;
        }
    };

    // TCP connect to port 443
    let mut rx_buf = [0u8; 4096];
    let mut tx_buf = [0u8; 4096];
    let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

    if let Err(e) = socket.connect((remote_ip, 443)).await {
        println!("TCP connect failed: {:?}", e);
        return;
    }
    println!("TCP connected to api.telegram.org");

    // Seed ChaCha20 RNG from hardware RNG
    let rng = Rng::new();
    let mut seed = [0u8; 32];
    for chunk in seed.chunks_mut(4) {
        let bytes = rng.random().to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    let mut crypto_rng = ChaCha20Rng::from_seed(seed);

    // TLS handshake
    let mut tls_rx = vec![0u8; 16640];
    let mut tls_tx = vec![0u8; 1024];
    let tls_config: TlsConfig<'_, Aes128GcmSha256> = TlsConfig::new()
        .with_server_name("api.telegram.org")
        .enable_rsa_signatures();
    let mut tls = TlsConnection::new(socket, &mut tls_rx, &mut tls_tx);

    if let Err(e) = tls
        .open::<_, NoVerify>(TlsContext::new(&tls_config, &mut crypto_rng))
        .await
    {
        println!("TLS handshake failed: {:?}", e);
        return;
    }
    println!("TLS handshake complete");

    // Build HTTP POST request (matching old esp-idf implementation)
    let body: String = format!("chat_id={}&text={}", TELEGRAM_CHAT_ID, message);
    let request: String = format!(
        "POST /bot{}/sendMessage HTTP/1.1\r\nHost: api.telegram.org\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        TELEGRAM_API_TOKEN, body.len(), body
    );

    // Send request
    if let Err(e) = tls.write_all(request.as_bytes()).await {
        println!("TLS write failed: {:?}", e);
        return;
    }
    tls.flush().await.ok();

    // Read response
    let mut resp_buf = [0u8; 1024];
    match tls.read(&mut resp_buf).await {
        Ok(n) => {
            let resp = core::str::from_utf8(&resp_buf[..n]).unwrap_or("(invalid utf8)");
            println!("Telegram response: {}", resp);
        }
        Err(e) => println!("Read response failed: {:?}", e),
    }

    tls.close().await.ok();
}
