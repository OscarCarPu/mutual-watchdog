use embedded_svc::http::Method;
use embedded_svc::http::client::Client as HttpClient;
use embedded_svc::io::Write;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::http::client::{Configuration as HttpConfig, EspHttpConnection};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};

const TELEGRAM_API_TOKEN: &str = env!("TELEGRAM_API_TOKEN");
const TELEGRAM_CHAT_ID: &str = env!("TELEGRAM_CHAT_ID");

fn send_telegram_message() -> anyhow::Result<()> {
    let config = HttpConfig {
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    };

    let mut connection = EspHttpConnection::new(&config)?;
    let mut client = HttpClient::wrap(&mut connection);

    let url = format!("https://api.telegram.org/bot{TELEGRAM_API_TOKEN}/sendMessage");
    let body = format!("chat_id={TELEGRAM_CHAT_ID}&text=Hi");

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

    loop {
        if let Err(e) = send_telegram_message() {
            println!("Failed to send Telegram message: {e}");
        }
        FreeRtos::delay_ms(10_000);
    }
}
