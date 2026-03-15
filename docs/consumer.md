# Consumer docs

Go service that runs on the home lab, subscribing to MQTT heartbeats from the ESP32 and alerting via Telegram when the ESP32 goes silent.

## How it works

1. Connects to the MQTT broker and subscribes to `watchdog/ping`
2. Every `CHECK_INTERVAL_SECS`, checks how long since the last ping
3. If `TIMEOUT_SECS` have passed without a ping, sends a Telegram alert
4. When the ESP32 starts pinging again, sends a recovery alert

The consumer is **receive-only** — it does not publish any MQTT messages. The ESP32 is responsible for sending periodic pings.

## Architecture

Runs as a Docker container via `docker compose`. The `docker-compose.yml` at the project root defines the `consumer-watchdog` service.

### Functions

##### `NewWatchdog`
Creates a new Watchdog with an MQTT client configured for the given broker, username, and password. Initializes `lastPing` to `time.Now()` so the first timeout window starts from boot.

##### `Connect`
Connects to the MQTT broker. Panics on failure in `main`.

##### `Subscribe`
Subscribes to `watchdog/ping`. On each received message, updates `lastPing` and sends a recovery Telegram alert if a previous timeout alert was active. Spawns a goroutine that ticks every `CHECK_INTERVAL_SECS` and checks `lastPing` against `TIMEOUT_SECS`.

##### `sendMessage`
Sends a Telegram message via the Bot API (`POST /sendMessage` with URL-encoded parameters).
