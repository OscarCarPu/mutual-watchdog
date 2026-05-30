# State Flow

## System topology

Both MQTT broker and consumer run inside the home lab. The ESP32 is the only external node

```mermaid
graph LR
    subgraph external["External"]
        ESP32["ESP32\n(XIAO C3)"]
    end

    subgraph homelab["Home Lab"]
        MQTT["MQTT Broker"]
        Consumer["Consumer"]
    end

    Telegram["Telegram Bot"]

    ESP32 -- "ping every N min" --> MQTT
    MQTT --> Consumer
    Consumer -. "timeout alert" .-> Telegram 
    Consumer -. "recovery alert" .-> Telegram
    ESP32 -. "lab-down alert\n(MQTT unreachable)" .-> Telegram
```

## ESP32 - per wake cycle

Runs once per `PING_INTERVAL_SECS`. The only state that persists across cycles is `ALERT_STATE` in RTC memory.

```mermaid
flowchart TD
    Wake([Wake from deep sleep]) --> WiFi[Connect WiFi + DHCP]
    WiFi --> MQTTtry[Attempt MQTT connect]

    MQTTtry -->|success| Ping[Publish ping to watchdog/ping]
    MQTTtry -->|fail| CheckFail{ALERT_STATE\n== ACTIVE?}

    Ping --> CheckOk{ALERT_STATE\n== ACTIVE?}
    CheckOk -->|Yes| TGRecovery["Telegram: 'lab responding again'\nClear ALERT_STATE"]
    CheckOk -->|No| Sleep 
    TGRecovery --> Sleep

    CheckFail -->|Yes| Sleep[Deep sleep PING_INTERVAL_SECS]
    CheckFail -->|No| TGDown["Telegram: 'lab down'\nSet ALERT_STATE = ACTIVE"]
    TGDown --> Sleep
```

## Consumer -- continuous process

Runs as a Docker container. In-memory state.

```mermaid
flowchart TD
    Boot([Boot]) --> Connect["Connect MQTT\n(auto-reconnect, retries every 5s)"]
    Connect --> Grace[Wait TIMEOUT_SECS grace period]
    Grace --> Tick

    Tick{Tick every CHECK_INTERVAL_SECS} --> Elapsed{time since lastPing\n> TIMEOUT_SECS?}
    Elapsed -->|No| Tick
    Elapsed -->|Yes| AlertSent{alertSent?}
    AlertSent -->|No| SendDown["Telegram: 'ESP32 not responding'\nalertSent = true"]
    AlertSent -->|Yes| Log[Log elapsed time]
    SendDown --> Tick
    Log --> Tick 

    Ping([MQTT ping received]) --> Update[lastPing = now]
    Update --> WasAlert{alertSent?}
    WasAlert -->|No| Idle[idle]
    WasAlert -->|Yes| SendRecovery["Telegram: 'ESP32 responding again'\nalertSent = false"]
```

