package main

import (
	"fmt"
	"log"
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
	checkInterval   time.Duration
	timeoutDuration time.Duration
)

type Watchdog struct {
	mqttClient  mqtt.Client
	httpClient  *http.Client
	lastPing    time.Time
	alertSent   bool
	sendMessage func(string) error
}

const (
	TopicPing           = "watchdog/ping"
	TopicUptimeLab      = "events/uptime/lab"
	TopicUptimeWatchdog = "events/uptime/watchdog"
)

func NewWatchdog(broker string, user string, password string) *Watchdog {
	w := &Watchdog{
		httpClient: &http.Client{},
		lastPing:   time.Now(),
	}
	w.sendMessage = func(msg string) error {
		return sendMessageTelegram(w.httpClient, msg)
	}

	opts := mqtt.NewClientOptions().AddBroker(broker)
	opts.SetUsername(user)
	opts.SetPassword(password)
	opts.SetClientID("homelab-watchdog")
	opts.SetConnectRetry(true)
	opts.SetConnectRetryInterval(5 * time.Second)
	opts.SetAutoReconnect(true)
	opts.SetMaxReconnectInterval(30 * time.Second)
	opts.SetOnConnectHandler(func(c mqtt.Client) {
		log.Println("MQTT connected, subscribing")
		if token := c.Subscribe(TopicPing, 0, w.onPing); token.Wait() && token.Error() != nil {
			log.Printf("Subscribe failed: %v\n", token.Error())
		}
	})
	opts.SetConnectionLostHandler(func(c mqtt.Client, err error) {
		log.Printf("MQTT connection lost: %v\n", err)
	})

	w.mqttClient = mqtt.NewClient(opts)
	return w
}

func (w *Watchdog) Connect() error {
	token := w.mqttClient.Connect()
	token.Wait()
	return token.Error()
}

func (w *Watchdog) onPing(_ mqtt.Client, _ mqtt.Message) {
	w.lastPing = time.Now()
	if w.alertSent {
		log.Println("Sending recovery alert")
		if err := w.sendMessage("Esp32 watchdog is responding again"); err != nil {
			log.Printf("Failed to send recovery alert: %v\n", err)
		}
		w.alertSent = false
	}
}

func (w *Watchdog) checkTimeout(timeout time.Duration) {
	elapsed := time.Since(w.lastPing)
	if elapsed > timeout {
		if !w.alertSent {
			log.Println("Timeout, sending alert")
			if err := w.sendMessage("Esp32 watchdog isn't responding"); err != nil {
				log.Printf("Failed to send alert: %v\n", err)
			} else {
				w.alertSent = true
			}
		} else {
			log.Printf("No response, elapsed: %v\n", elapsed.Round(time.Second))
		}
	}
}

func (w *Watchdog) StartTimeoutChecker() {
	go func() {
		time.Sleep(timeoutDuration)
		ticker := time.NewTicker(checkInterval)
		for range ticker.C {
			w.checkTimeout(timeoutDuration)
		}
	}()
}

func sendMessageTelegram(httpClient *http.Client, message string) error {
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

	resp, err := httpClient.Do(req)
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

	checkInterval, _ = time.ParseDuration(os.Getenv("CHECK_INTERVAL_SECS") + "s")
	timeoutDuration, _ = time.ParseDuration(os.Getenv("TIMEOUT_SECS") + "s")

	w := NewWatchdog(broker, user, password)

	if err := w.Connect(); err != nil {
		log.Printf("Initial MQTT connect pending: %v (will keep retrying)\n", err)
	}

	w.StartTimeoutChecker()

	select {}
}
