# MQTT Topics

## Ping

`watchdog/ping` — ESP32 heartbeat. Plain-text payload `Ping`.

## Uptime

`events/uptime/lab` and `events/uptime/watchdog` — derived uptime events.

```json
{ "state": "up", "timestamp": "2026-05-31T04:01:00Z" }
```

`state` is `up` or `down`; `timestamp` is RFC 3339 UTC.
