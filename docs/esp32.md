# ESP32 docs

- 2 stages: development (esp32 devkit) and production (esp32c3 supermini)

## Code

The base is a template from [template-esp32](https://github.com/OscarCarPu/template-esp32)

### Functions

##### `setup_wifi`
Initializes the esp-radio controller, creates the WiFi STA device and embassy-net stack with DHCP. Spawns `connect_wifi` and `net_task` as background tasks. Returns the network stack.

##### `connect_wifi`
Embassy task that manages the WiFi STA lifecycle. Configures the connection with SSID/password from env vars, starts the driver, and calls `connect_async`. On disconnect, waits 5s and reconnects automatically.

##### `ping_google`
For testing. Will be replaced.

##### `net_task`
Embassy task that runs the embassy-net packet processing loop.

##### `create_mqtt_client`
Not yet implemented.

##### `send_mqtt_ping`
Not yet implemented.

##### `check_mqtt_ping`
Not yet implemented.

##### `create_telegram_client`
Not yet implemented.

##### `send_telegram_message`
Not yet implemented.
