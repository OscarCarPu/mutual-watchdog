# Consumer docs

Go service that runs on the home lab, subscribing to MQTT heartbeats from the ESP32 and alerting via Telegram when the ESP32 goes silent.

## How it works

1. Connects to the MQTT broker. The subscription to `watchdog/ping` is wired through the on-connect handler, so it survives reconnects.
2. Starts a timeout checker goroutine that waits one full `TIMEOUT_SECS` before its first check (avoids a false alert at boot), then ticks every `CHECK_INTERVAL_SECS`.
3. If `TIMEOUT_SECS` have elapsed since the last ping, sends a Telegram alert and sets `alertSent` so duplicates are suppressed.
4. When a ping arrives again, sends a recovery alert and clears `alertSent`.

The consumer is **receive-only** — it does not publish any MQTT messages. The ESP32 is responsible for sending periodic pings.

## Architecture

Runs as a Docker container via `docker compose`. The `docker-compose.yml` at the project root defines the `consumer-watchdog` service.

The MQTT client is configured for resilience: `SetConnectRetry(true)` with a 5s retry interval, and `SetAutoReconnect(true)` with up to a 30s backoff. This is why the initial `Connect` failure is non-fatal — the client keeps retrying in the background.

### Functions

##### `NewWatchdog`
Builds the `Watchdog` and its MQTT client. Initializes `lastPing` to `time.Now()` so the first timeout window starts from boot. Configures auto-reconnect, connect-retry, and an `OnConnectHandler` that subscribes to `watchdog/ping` on every (re)connect.

##### `Connect`
Connects to the MQTT broker. Returns the error rather than panicking — `main` logs it and lets the client retry in the background.

##### `onPing`
Subscription callback. Updates `lastPing`. If a previous timeout alert was active, sends a Telegram recovery message and clears `alertSent`.

##### `StartTimeoutChecker`
Spawns a goroutine that sleeps for `timeoutDuration` (boot grace period), then ticks every `checkInterval`. On each tick, if `time.Since(lastPing) > timeoutDuration` and no alert has been sent yet, sends the Telegram alert and sets `alertSent`. Subsequent timeouts only log, until a ping clears the state.

##### `sendMessage`
Sends a Telegram message via the Bot API (`POST /sendMessage` with URL-encoded parameters).

## Future work

- Persist uptime / liveness history for both the consumer and the ESP32 watchdog to an external database, so availability can be queried beyond the in-memory `lastPing` window.
