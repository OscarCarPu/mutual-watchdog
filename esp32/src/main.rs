use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use embedded_svc::http::Method;
use embedded_svc::http::client::Client as HttpClient;
use embedded_svc::io::Write;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::http::client::{Configuration as HttpConfig, EspHttpConnection};
use esp_idf_svc::mqtt::client::{EspMqttClient, EventPayload, MqttClientConfiguration, QoS};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};

const TELEGRAM_API_TOKEN: &str = env!("TELEGRAM_API_TOKEN");
const TELEGRAM_CHAT_ID: &str = env!("TELEGRAM_CHAT_ID");
const MQTT_SERVER: &str = env!("MQTT_SERVER");
const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");

fn create_mqtt_client(
    last_ping_time: Arc<Mutex<SystemTime>>,
    alert_sent: Arc<Mutex<bool>>,
) -> anyhow::Result<EspMqttClient<'static>> {
    let config = MqttClientConfiguration {
        username: Some(MQTT_USER),
        password: Some(MQTT_PASSWORD),
        client_id: Some("esp32-watchdog"),
        ..Default::default()
    };

    let client = EspMqttClient::new_cb(MQTT_SERVER, &config, move |event| {
        if let EventPayload::Received { topic, .. } = event.payload() {
            if topic == Some("watchdog/ping-lab") {
                if let Ok(mut time) = last_ping_time.lock() {
                    *time = SystemTime::now();
                }

                if let Ok(mut sent) = alert_sent.lock() {
                    *sent = false;
                }
            }
        }
    })?;

    println!("MQTT client connected");
    Ok(client)
}

fn send_mqtt_ping(client: &mut EspMqttClient<'static>) {
    match client.publish("watchdog/ping-esp32", QoS::AtMostOnce, false, b"ping") {
        Ok(_) => println!("Sent MQTT ping"),
        Err(e) => println!("Error sending MQTT ping: {:?}", e),
    }
}

fn send_telegram_message(text: &str) {
    let result = (|| -> anyhow::Result<()> {
        let config = HttpConfig {
            use_global_ca_store: true,
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            ..Default::default()
        };
        let mut connection = EspHttpConnection::new(&config)?;
        let mut client = HttpClient::wrap(&mut connection);

        let url = format!("https://api.telegram.org/bot{TELEGRAM_API_TOKEN}/sendMessage");
        let body = format!("chat_id={TELEGRAM_CHAT_ID}&text={text}");

        let headers = [
            ("Content-Type", "application/x-www-form-urlencoded"),
            ("Content-Length", &body.len().to_string()),
        ];

        let mut request = client.request(Method::Post, &url, &headers)?;
        request.write_all(body.as_bytes())?;
        request.flush()?;
        let response = request.submit()?;
        println!("Telegram response status: {}", response.status());
        Ok(())
    })();

    if let Err(e) = result {
        println!("Error sending Telegram message: {:?}", e);
    }
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("Telegram bot configured for chat {TELEGRAM_CHAT_ID}");

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: env!("WIFI_SSID").try_into().unwrap(),
        password: env!("WIFI_PASSWORD").try_into().unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;

    wifi.start()?;
    println!("Connecting to WiFi");
    wifi.connect()?;
    println!("Connected to WiFi");
    wifi.wait_netif_up()?;
    println!("WiFi netif up");

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;
    println!("IP: {:?}", ip_info);

    let last_ping_time = Arc::new(Mutex::new(SystemTime::now()));
    let alert_sent = Arc::new(Mutex::new(false));

    let mut mqtt_client = create_mqtt_client(last_ping_time.clone(), alert_sent.clone())?;
    println!("MQTT client created");
    FreeRtos::delay_ms(2_000);

    while let Err(e) = mqtt_client.subscribe("watchdog/ping-lab", QoS::AtMostOnce) {
        println!("Error subscribing ({:?}), retrying", e);
        FreeRtos::delay_ms(1_000);
    }
    println!("Subscribed to topic");

    let mut prev_alert_sent = false;

    loop {
        send_mqtt_ping(&mut mqtt_client);

        let mut trigger_alert = false;
        if let Ok(time) = last_ping_time.lock() {
            if let Ok(elapsed) = SystemTime::now().duration_since(*time) {
                println!("Elapsed: {:?}", elapsed);
                if elapsed > Duration::from_secs(120) {
                    println!("Timeout, triggering alert");
                    trigger_alert = true;
                }
            }
        }

        if trigger_alert {
            if let Ok(mut sent) = alert_sent.lock() {
                if !*sent {
                    println!("Sending alert");
                    send_telegram_message("Home lab isn't responding");
                    *sent = true;
                    prev_alert_sent = true;
                } else {
                    println!("Alert already sent");
                }
            }
        } else if prev_alert_sent {
            if let Ok(sent) = alert_sent.lock() {
                if !*sent {
                    println!("Sending recovery alert");
                    send_telegram_message("Home lab is responding again");
                    prev_alert_sent = false;
                }
            }
        }

        println!("Sleeping");
        FreeRtos::delay_ms(10_000);
    }
}
