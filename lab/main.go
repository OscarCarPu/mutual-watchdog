package main

import (
	"fmt"
	"net/http"
	"net/url"
	"os"
	"time"

	mqtt "github.com/eclipse/paho.mqtt.golang"
	"github.com/joho/godotenv"
)

var (
	telegramToken   string
	telegramChatID  string
	pingInterval    time.Duration
	checkInterval   time.Duration
	timeoutDuration time.Duration
)

type Watchdog struct {
	mqttClient mqtt.Client
	httpClient *http.Client
	lastPing   time.Time
	alertSent  bool
}

func NewWatchdog(broker string, user string, password string) *Watchdog {
	opts := mqtt.NewClientOptions().AddBroker(broker)
	opts.SetUsername(user)
	opts.SetPassword(password)
	opts.SetClientID("homelab-watchdog")

	return &Watchdog{
		mqttClient: mqtt.NewClient(opts),
		httpClient: &http.Client{},
		lastPing:   time.Now(),
	}
}

func (w *Watchdog) Connect() error {
	if token := w.mqttClient.Connect(); token.Wait() && token.Error() != nil {
		return token.Error()
	}
	return nil
}

func (w *Watchdog) Ping() {
	ticker := time.NewTicker(pingInterval)
	go func() {
		for range ticker.C {
			w.mqttClient.Publish("watchdog/ping-lab", 0, false, "ping")
			fmt.Println("Sent heartbeat to ESP32")
		}
	}()
}

func (w *Watchdog) Subscribe() {
	w.mqttClient.Subscribe("watchdog/ping-esp32", 0, func(c mqtt.Client, m mqtt.Message) {
		w.lastPing = time.Now()
		if w.alertSent {
			fmt.Println("Sending recovery alert")
			if err := w.sendMessage("Esp32 watchdog is responding again"); err != nil {
				fmt.Printf("Failed to send recovery alert: %v\n", err)
			}
			w.alertSent = false
		}
		fmt.Println("Received heartbeat from ESP32")
	})

	go func() {
		ticker := time.NewTicker(checkInterval)
		for range ticker.C {
			elapsed := time.Since(w.lastPing)
			fmt.Printf("Elapsed: %v\n", elapsed.Round(time.Second))
			if elapsed > timeoutDuration {
				if !w.alertSent {
					fmt.Println("Timeout, sending alert")
					if err := w.sendMessage("Esp32 watchdog isn't responding"); err != nil {
						fmt.Printf("Failed to send alert: %v\n", err)
					} else {
						w.alertSent = true
					}
				} else {
					fmt.Println("Alert already sent")
				}
			}
			fmt.Println("Sleeping")
		}
	}()
}

func (w *Watchdog) sendMessage(message string) error {
	rawURL := fmt.Sprintf(
		"https://api.telegram.org/bot%s/sendMessage?chat_id=%s&text=%s",
		telegramToken,
		telegramChatID,
		url.QueryEscape(message),
	)

	req, err := http.NewRequest("POST", rawURL, nil)
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

	resp, err := w.httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	return nil
}

func main() {
	godotenv.Load()

	telegramToken = os.Getenv("TELEGRAM_API_TOKEN")
	telegramChatID = os.Getenv("TELEGRAM_CHAT_ID")

	broker := os.Getenv("MQTT_SERVER")
	user := os.Getenv("MQTT_USER")
	password := os.Getenv("MQTT_PASSWORD")

	pingInterval, _ = time.ParseDuration(os.Getenv("PING_INTERVAL_SECS") + "s")
	checkInterval, _ = time.ParseDuration(os.Getenv("CHECK_INTERVAL_SECS") + "s")
	timeoutDuration, _ = time.ParseDuration(os.Getenv("TIMEOUT_SECS") + "s")

	w := NewWatchdog(broker, user, password)

	if err := w.Connect(); err != nil {
		panic(err)
	}

	w.Subscribe()
	w.Ping()

	select {}
}
