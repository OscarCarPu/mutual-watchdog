#![no_std]
#![no_main]

extern crate alloc;

use core::time::Duration;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources, dns::DnsQueryType, tcp::TcpSocket};
use embassy_time::{Duration as EmbassyDuration, Timer, with_timeout};
use embedded_io_async::Write as _;
use embedded_tls::{Aes128GcmSha256, TlsConfig, TlsConnection, TlsContext, UnsecureProvider};
use esp_alloc as _;
use esp_backtrace as _;
#[cfg(feature = "esp32c3")]
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::{
    clock::CpuClock,
    rng::Rng,
    rtc_cntl::{
        Rtc,
        sleep::{RtcSleepConfig, TimerWakeupSource},
    },
    timer::timg::TimerGroup,
};
use mutual_watchdog::{AlertAction, ALERT_ACTIVE, compute_alert_action};
use esp_println::println;
use esp_radio::{
    Controller,
    wifi::{ClientConfig, ModeConfig, WifiController, WifiDevice, WifiEvent, WifiStaState},
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use rust_mqtt::{
    Bytes,
    buffer::AllocBuffer,
    client::{
        Client, MqttError,
        options::{ConnectOptions, PublicationOptions},
    },
    config::{KeepAlive, SessionExpiryInterval},
    types::{MqttBinary, MqttString, QoS, TopicName},
};

esp_bootloader_esp_idf::esp_app_desc!();

type MqttClient = Client<'static, TcpSocket<'static>, AllocBuffer, 1, 1, 1>;

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
const TOPIC_PING: &str = "watchdog/ping";

#[unsafe(link_section = ".rtc_fast.persistent")]
static mut ALERT_STATE: u32 = 0;

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // peripherals
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_80MHz));

    esp_alloc::heap_allocator!(size: 96 * 1024);

    println!("RTC: ALERT_STATE=0x{:08X}", unsafe { ALERT_STATE });

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    #[cfg(feature = "esp32")]
    esp_rtos::start(timg0.timer0);
    #[cfg(feature = "esp32c3")]
    {
        let sw_ints = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
        esp_rtos::start(timg0.timer0, sw_ints.software_interrupt0);
    }

    // Small delay to let power stabilize before WiFi radio spike
    Timer::after_millis(500).await;

    // WiFi
    let stack = setup_wifi(&spawner, peripherals.WIFI).await;

    while !stack.is_link_up() {
        Timer::after_millis(500).await;
    }
    println!("WiFi link up!");

    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    stack
        .config_v4()
        .inspect(|c| println!("IPv4 config: {c:?}"));

    let ping_result = match create_mqtt_client(stack).await {
        Ok(mut client) => {
            let result = send_mqtt_ping(&mut client).await;
            drop(client);
            result
        }
        Err(_e) => {
            if let AlertAction::SendAlert = compute_alert_action(false, unsafe { &mut ALERT_STATE }) {
                send_telegram_message(stack, "Home lab isn't responding").await;
            }
            println!("MQTT connect failed, sleeping...");
            deep_sleep(peripherals.LPWR);
        }
    };

    match compute_alert_action(ping_result.is_ok(), unsafe { &mut ALERT_STATE }) {
        AlertAction::SendRecovery => {
            send_telegram_message(stack, "Home lab is responding again").await;
        }
        AlertAction::SendAlert => {
            send_telegram_message(stack, "Home lab isn't responding").await;
        }
        AlertAction::NoAction => {}
    }

    println!("Sleeping...");
    deep_sleep(peripherals.LPWR);
}

fn deep_sleep(lpwr: esp_hal::peripherals::LPWR<'static>) -> ! {
    let mut rtc = Rtc::new(lpwr);
    let timer = TimerWakeupSource::new(Duration::from_secs(PING_INTERVAL_SECS.parse().unwrap()));
    // Keep RTC fast memory powered during deep sleep to persist ALERT_STATE
    let mut cfg = RtcSleepConfig::deep();
    cfg.set_rtc_fastmem_pd_en(false);
    rtc.sleep(&cfg, &[&timer]);
    unreachable!();
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

    Timer::after_millis(2000).await;
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
            controller
                .set_power_saving(esp_radio::wifi::PowerSaveMode::Maximum)
                .unwrap();
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

async fn try_2s<F, T, E>(label: &str, fut: F) -> Result<T, ()>
where
    F: core::future::Future<Output = Result<T, E>>,
    E: core::fmt::Debug,
{
    match with_timeout(EmbassyDuration::from_secs(2), fut).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => {
            println!("{} failed: {:?}", label, e);
            Err(())
        }
        Err(_) => {
            println!("{} timed out", label);
            Err(())
        }
    }
}

async fn create_mqtt_client(stack: Stack<'static>) -> Result<MqttClient, MqttError<'static>> {
    let buffer = mk_static!(AllocBuffer, AllocBuffer);
    let mut client = Client::<'_, _, _, 1, 1, 1>::new(buffer);

    let connect_options = ConnectOptions {
        clean_start: true,
        keep_alive: KeepAlive::default(),
        session_expiry_interval: SessionExpiryInterval::NeverEnd,
        user_name: Some(MqttString::from_slice(MQTT_USER).unwrap()),
        password: Some(MqttBinary::from_slice(MQTT_PASSWORD.as_bytes()).unwrap()),
        will: None,
    };

    let rx_buf = mk_static!([u8; 4096], [0u8; 4096]);
    let tx_buf = mk_static!([u8; 4096], [0u8; 4096]);
    let mut socket = TcpSocket::new(stack, rx_buf, tx_buf);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

    let host_port = MQTT_SERVER.strip_prefix("mqtt://").unwrap_or(MQTT_SERVER);
    let (host, port) = host_port.rsplit_once(':').unwrap_or((host_port, "1883"));
    let remote_ip: embassy_net::Ipv4Address = host.parse().unwrap();
    let remote_port: u16 = port.parse().unwrap();
    if try_2s("MQTT TCP connect", socket.connect((remote_ip, remote_port)))
        .await
        .is_err()
    {
        return Err(MqttError::Network(embedded_io_async::ErrorKind::Other));
    }

    if try_2s(
        "MQTT CONNECT",
        client.connect(
            socket,
            &connect_options,
            Some(MqttString::from_slice("watchdog-esp32").unwrap()),
        ),
    )
    .await
    .is_err()
    {
        return Err(MqttError::Network(embedded_io_async::ErrorKind::Other));
    }

    Ok(client)
}

async fn send_mqtt_ping(client: &mut MqttClient) -> Result<(), MqttError<'static>> {
    let topic = unsafe { TopicName::new_unchecked(MqttString::from_slice(TOPIC_PING).unwrap()) };
    let pub_options = PublicationOptions {
        retain: false,
        topic: topic.as_borrowed(),
        qos: QoS::AtMostOnce,
    };
    client
        .publish(&pub_options, Bytes::Borrowed(b"Ping"))
        .await?;
    Ok(())
}

async fn send_telegram_message(stack: Stack<'static>, message: &str) {
    // DNS resolve api.telegram.org
    let Ok(addrs) = try_2s(
        "DNS query",
        stack.dns_query("api.telegram.org", DnsQueryType::A),
    )
    .await
    else {
        return;
    };
    let ip_addr = addrs[0];

    let embassy_net::IpAddress::Ipv4(remote_ip) = ip_addr;

    // TCP connect to port 443
    let mut rx_buf = [0u8; 4096];
    let mut tx_buf = [0u8; 4096];
    let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

    if try_2s("TCP connect", socket.connect((remote_ip, 443)))
        .await
        .is_err()
    {
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
    let crypto_rng = ChaCha20Rng::from_seed(seed);

    // TLS handshake
    let mut tls_rx = vec![0u8; 16640];
    let mut tls_tx = vec![0u8; 1024];
    let tls_config = TlsConfig::new()
        .with_server_name("api.telegram.org")
        .enable_rsa_signatures();
    let mut tls = TlsConnection::new(socket, &mut tls_rx, &mut tls_tx);

    if try_2s(
        "TLS handshake",
        tls.open(TlsContext::new(
            &tls_config,
            UnsecureProvider::new::<Aes128GcmSha256>(crypto_rng),
        )),
    )
    .await
    .is_err()
    {
        return;
    }
    println!("TLS handshake complete");

    // Build HTTP POST request (matching old esp-idf implementation)
    let body: String = format!("chat_id={}&text={}", TELEGRAM_CHAT_ID, message);
    let request: String = format!(
        "POST /bot{}/sendMessage HTTP/1.1\r\nHost: api.telegram.org\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        TELEGRAM_API_TOKEN,
        body.len(),
        body
    );

    // Send request
    if try_2s("TLS write", tls.write_all(request.as_bytes()))
        .await
        .is_err()
    {
        return;
    }
    let _ = try_2s("TLS flush", tls.flush()).await;

    // Read response
    let mut resp_buf = [0u8; 1024];
    if let Ok(n) = try_2s("TLS read", tls.read(&mut resp_buf)).await {
        let resp = core::str::from_utf8(&resp_buf[..n]).unwrap_or("(invalid utf8)");
        println!("Telegram response: {}", resp);
    }

    let _ = with_timeout(EmbassyDuration::from_secs(2), tls.close()).await;
}
