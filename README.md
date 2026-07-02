# mutual-watchdog
Watchdog system where an external ESP32 (Rust) periodically pings a home lab consumer (Go) over MQTT. If the ESP32 stops pinging, the consumer sends a Telegram alert. If MQTT is unreachable, the ESP32 sends its own Telegram alert directly.

## Architecture

The system has two independent alert paths so that a failure anywhere in the chain is caught:

**Normal path** — the ESP32 wakes from deep sleep, connects to WiFi, and publishes a ping to the MQTT broker. The Go consumer subscribes to that topic and resets its timeout on every ping. If no ping arrives within the configured timeout, the consumer fires a Telegram alert.

**Fallback path** — if the ESP32 cannot reach the MQTT broker (broker down, network issue, home lab offline), it skips MQTT entirely and calls the Telegram HTTP API directly over TLS from the firmware. This means the ESP32 monitors the home lab, and the home lab monitors the ESP32 — neither side can silently fail.

Alert state is stored in RTC fast memory (`.rtc_fast.persistent`) so it survives deep sleep without triggering duplicate alerts across cycles.

## Docs

- [State flow](docs/flow_state.md)
- [MQTT topics](docs/topics_mqtt.md)

### Configuration

Copy the example env files and fill in your values:

```sh
cp .env.example .env
cp esp32/.env.example esp32/.env
cp consumer/.env.example consumer/.env
```

**Root `.env`** (shared config):
| Variable | Description |
|---|---|
| `TELEGRAM_API_TOKEN` | Telegram bot API token |
| `TELEGRAM_CHAT_ID` | Telegram chat ID for alerts |
| `MQTT_SERVER` | MQTT broker URL (e.g. `mqtt://192.168.1.135:1883`) |
| `PING_INTERVAL_SECS` | How often the ESP32 pings (used by ESP32 firmware) |
| `CHECK_INTERVAL_SECS` | How often the consumer checks for timeouts |
| `TIMEOUT_SECS` | How long before the consumer considers the ESP32 down |

**`consumer/.env`** (MQTT credentials for the consumer):
| Variable | Description |
|---|---|
| `MQTT_USER` | MQTT username |
| `MQTT_PASSWORD` | MQTT password |

**`esp32/.env`** (compiled into the ESP32 firmware at build time):
| Variable | Description |
|---|---|
| `WIFI_SSID` | WiFi network name |
| `WIFI_PASSWORD` | WiFi password |
| `MQTT_USER` | MQTT username |
| `MQTT_PASSWORD` | MQTT password |

### Build & Flash

```sh
# ESP32 DevKit (dev)
make esp32-build-dev    # build the firmware
make esp32-flash-dev    # flash and monitor
make esp32-stop-dev     # erase flash

# XIAO ESP32-C3 (prod)
make esp32-build-prod   # build the firmware
make esp32-flash-prod   # flash and monitor
make esp32-stop-prod    # erase flash
```

### Consumer

```sh
make consumer-up      # start consumer container
make consumer-down    # stop consumer container
make consumer-logs    # tail consumer logs
```

### Case

The `case/` directory contains a 3D-printable enclosure for the XIAO ESP32-C3 used in the production deployment: `case.scad` (OpenSCAD source) plus exported `case.stl` and `lid.stl`.
