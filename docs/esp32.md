# ESP32 docs

- 2 stages: development (esp32 devkit) and production (esp32c3 supermini)

## General info 

- Sincronous working
- Deep sleep 

### Functions

##### `setup_wifi`
Initializes the esp-radio controller, creates the WiFi STA device and embassy-net stack with DHCP. Spawns `connect_wifi` and `net_task` as background tasks. Returns the network stack.

##### `connect_wifi`
Embassy task that manages the WiFi STA lifecycle. Configures the connection with SSID/password from env vars, starts the driver, and calls `connect_async`. On disconnect, waits 5s and reconnects automatically.

##### `create_mqtt_client`
Not yet implemented.

##### `send_mqtt_ping`
Not yet implemented.

##### `send_telegram_message`
Sends a message to the configured Telegram bot. Since Telegram requires HTTPS and there's no HTTP client library compatible with embassy-net 0.7, this is done manually through the full network stack:

1. **DNS** — resolves `api.telegram.org` via `stack.dns_query()`
2. **TCP** — connects to the resolved IP on port 443
3. **RNG** — seeds a ChaCha20 CSPRNG from the hardware RNG (needed by TLS)
4. **TLS 1.3** — handshake via `embedded-tls` with `Aes128GcmSha256` cipher, `NoVerify` (no cert validation in no_std)
5. **HTTP** — writes a raw `POST /bot<token>/sendMessage` request with form-encoded body
6. **Response** — reads and prints the Telegram API JSON response

TLS buffers are heap-allocated: 16640 bytes read (max TLS record size), 1024 bytes write. Combined with the 72KB heap this fits both the ESP32 DevKit (520KB SRAM) and ESP32-C3 SuperMini (400KB SRAM). Using a larger heap (e.g. 110KB) causes stack collisions and crashes during the TLS handshake.
