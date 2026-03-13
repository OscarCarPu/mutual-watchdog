# mutual-watchdog
Mutual watchdog between my home lab (go) and an external esp32 (rust), for notifying me if any of them is down

## Docs 

- [ESP32 docs](docs/esp32.md)
- [Go consumer](docs/consumer.md)

### Configuration

Copy the example env files and fill in your values:

```sh
cp .env.example .env
cp esp32/.env.example esp32/.env
```

**Root `.env`** (shared config, also used by the home lab side):
| Variable | Description |
|---|---|
| `TELEGRAM_API_TOKEN` | Telegram bot API token |
| `TELEGRAM_CHAT_ID` | Telegram chat ID for alerts |
| `MQTT_SERVER` | MQTT broker URL (e.g. `mqtt://192.168.1.135:1883`) |

**`esp32/.env`** (compiled into the ESP32 firmware at build time):
| Variable | Description |
|---|---|
| `WIFI_SSID` | WiFi network name |
| `WIFI_PASSWORD` | WiFi password |
| `MQTT_USER` | MQTT username |
| `MQTT_PASSWORD` | MQTT password |

### Build & Flash

```sh
make esp32-build    # build the firmware
make esp32-flash    # flash and monitor
make esp32-stop     # erase flash
```
