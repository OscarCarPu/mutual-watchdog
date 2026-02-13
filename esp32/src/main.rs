use esp_idf_hal::delay::FreeRtos;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};

fn main() -> anyhow::Result<()> {
    // Link patches for ESP-IDF
    esp_idf_svc::sys::link_patches();

    // Initialize logging
    esp_idf_svc::log::EspLogger::initialize_default();

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
        FreeRtos::delay_ms(1000);
    }
}
