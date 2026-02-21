package main

import (
	"fmt"
	"os"
	"time"

	"github.com/joho/godotenv"
	mqtt "github.com/eclipse/paho.mqtt.golang"
)

type MQTTClient interface {
	Connect() mqtt.Token
	Publish(topic string, qos byte, retained bool, payload interface{}) mqtt.Token
	Subscribe(topic string, qos byte, callback mqtt.MessageHandler) mqtt.Token
}

type Watchdog struct {
	client   MQTTClient
	lastPing time.Time
}

func NewWatchdog(broker string, user string, password string) *Watchdog {
	opts := mqtt.NewClientOptions().AddBroker(broker)
	opts.SetUsername(user)
	opts.SetPassword(password)
	opts.SetClientID("homelab-watchdog")

	return &Watchdog{
		client:   mqtt.NewClient(opts),
		lastPing: time.Now(),
	}
}

func (w *Watchdog) Connect() error {
	if token := w.client.Connect(); token.Wait() && token.Error() != nil {
		return token.Error()
	}
	return nil
}

func (w *Watchdog) Ping(interval time.Duration) {
	ticker := time.NewTicker(interval)
	go func() {
		for range ticker.C {
			w.client.Publish("watchdog/ping-lab", 0, false, "ping")
			fmt.Println("Sent heartbeat to ESP32")
		}
	}()
}

func (w *Watchdog) Subscribe() {
	w.client.Subscribe("watchdog/ping-esp32", 0, func(c mqtt.Client, m mqtt.Message) {
		w.lastPing = time.Now()
		fmt.Println("Received heartbeat from ESP32")
	})
}

func main() {
	godotenv.Load()

	broker := os.Getenv("MQTT_BROKER")
	user := os.Getenv("MQTT_USER")
	password := os.Getenv("MQTT_PASSWORD")

	w := NewWatchdog(broker, user, password)

	if err := w.Connect(); err != nil {
		panic(err)
	}

	w.Subscribe()
	w.Ping(10 * time.Second)

	select {}
}
