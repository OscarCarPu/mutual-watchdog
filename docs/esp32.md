# ESP32 docs

- 2 stages: development (ESP32 DevKit) and production (XIAO ESP32-C3)

## General info

- Synchronous working: wakes up, connects WiFi, publishes MQTT ping, then deep sleeps
- Deep sleep interval is configured via `PING_INTERVAL_SECS`

## Recovery alerting

Mirrors the consumer's recovery pattern using an `ALERT_STATE` variable persisted in RTC fast memory (`.rtc_fast.persistent` linker section). A magic number (`0xA1E27001`) means "alert active"; any other value (including garbage after flash) means "no alert".

- On first MQTT failure (connect or publish): sends "Home lab isn't responding" via Telegram and sets `ALERT_STATE` to the magic value
- On subsequent failures: alert is suppressed (no duplicates)
- On successful MQTT ping after a failure: sends "Home lab is responding again" and clears `ALERT_STATE`
- The MQTT client is explicitly dropped before sending Telegram to avoid exhausting the `StackResources<3>` socket pool (DHCP + MQTT + DNS/TCP for Telegram would exceed 3 slots)

### RTC memory gotchas

- **`sleep_deep()` powers down RTC memory by default** on both ESP32 and ESP32-C3. The `deep_sleep` function uses `rtc.sleep()` with a custom `RtcSleepConfig` that sets `rtc_fastmem_pd_en(false)` to keep RTC fast memory alive. See [esp-hal#2516](https://github.com/esp-rs/esp-hal/issues/2516).
- **WiFi driver overwrites parts of RTC fast memory** on ESP32. Using a `u32` magic number instead of a `bool` avoids false positives from corrupted memory — garbage will never accidentally match `0xA1E27001`.
- The `.rtc_fast.persistent` section is `NOLOAD`, so the `= 0` initializer in source code has no effect. On first power-on, the value is whatever was in RTC RAM.

## Network timeouts

Every network operation (TCP connect, MQTT CONNECT, DNS, TLS handshake, TLS read/write) is wrapped in `try_2s`, a 2-second `with_timeout` helper. Without this, a half-open socket or unresponsive broker could keep the firmware awake indefinitely and prevent it from reaching `deep_sleep`, which would both drain power and (more importantly) break the watchdog contract — a stuck ESP32 looks identical to a healthy one from the consumer's perspective only if it eventually pings, so a stuck ESP32 that never sleeps + reboots is a false negative the consumer can't catch.

### Functions

##### `main`
Entry point. Initializes peripherals, heap allocator, and timer group. Sets up WiFi, waits for link and DHCP, then connects to MQTT and publishes a ping. On MQTT failure, sends a Telegram alert (suppressed on consecutive failures via RTC-persisted `ALERT_STATE`). On successful ping after a failure, sends a recovery alert. The MQTT client is dropped before any Telegram calls to free its socket slot. Always enters deep sleep at the end.

##### `try_2s`
Helper that wraps any fallible async future in a 2-second `embassy_time::with_timeout`. Logs and returns `Err(())` on either inner error or timeout. Used everywhere a network call could otherwise hang.

##### `deep_sleep`
Configures the RTC timer wakeup source with `PING_INTERVAL_SECS` and enters deep sleep using a custom `RtcSleepConfig` that keeps RTC fast memory powered (`rtc_fastmem_pd_en = false`). Never returns.

##### `setup_wifi`
Initializes the esp-radio controller, creates the WiFi STA device and embassy-net stack with DHCP. Spawns `connect_wifi` and `net_task` as background tasks. Returns the network stack.

##### `connect_wifi`
Embassy task that manages the WiFi STA lifecycle. Configures the connection with SSID/password from env vars, starts the driver, sets `PowerSaveMode::Maximum` to keep current draw down between pings, and calls `connect_async`. On disconnect, waits 5s and reconnects automatically.

##### `create_mqtt_client`
Parses `MQTT_SERVER` (supports `mqtt://host:port` format), opens a TCP connection to the broker, then performs the MQTT handshake with credentials from env vars. Uses `mk_static!` for buffer allocations so the client can be returned and used across function boundaries. Returns `Result<MqttClient, MqttError>`.

##### `send_mqtt_ping`
Publishes a "Ping" message to the `watchdog/ping` topic with QoS 0 (at most once, fire-and-forget). The consumer subscribes to this topic to detect ESP32 liveness.

##### `send_telegram_message`
Sends a message to the configured Telegram bot. Since Telegram requires HTTPS and there's no HTTP client library compatible with embassy-net, this is done manually through the full network stack:

1. **DNS** — resolves `api.telegram.org` via `stack.dns_query()`
2. **TCP** — connects to the resolved IP on port 443
3. **RNG** — seeds a ChaCha20 CSPRNG from the hardware RNG (needed by TLS)
4. **TLS 1.3** — handshake via `embedded-tls` with `Aes128GcmSha256` cipher, `UnsecureProvider` (no cert validation in no_std)
5. **HTTP** — writes a raw `POST /bot<token>/sendMessage` request with form-encoded body
6. **Response** — reads and prints the Telegram API JSON response

TLS buffers are heap-allocated: 16640 bytes read (max TLS record size), 1024 bytes write. Combined with the 96KB heap this fits both the ESP32 DevKit (520KB SRAM) and the XIAO ESP32-C3 (400KB SRAM). Pushing the heap higher (e.g. 110KB) causes stack collisions and crashes during the TLS handshake, so 96KB is the current ceiling.
