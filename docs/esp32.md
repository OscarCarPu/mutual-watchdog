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

##### `create_telegram_client`
Not yet implemented.

##### `send_telegram_message`
Not yet implemented.
