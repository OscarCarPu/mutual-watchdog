package main

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
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
	mqttClient   mqtt.Client
	httpClient   *http.Client
	tracker      *UptimeTracker
	lastPing     time.Time
	alertSent    bool
	labPublished bool
	sendMessage  func(string) error
}

type UptimeTracker struct {
	mqttClient         mqtt.Client
	pathPersistentData string
}

type Device string

const (
	DeviceLab      Device = "lab"
	DeviceWatchdog Device = "watchdog"
)

type Status string

const (
	StatusUp   Status = "up"
	StatusDown Status = "down"
)

var deviceTopic = map[Device]string{
	DeviceLab:      TopicUptimeLab,
	DeviceWatchdog: TopicUptimeWatchdog,
}

func (d Device) Topic() string {
	return deviceTopic[d]
}

type UptimeEvent struct {
	State     Status `json:"state"`
	Timestamp string `json:"timestamp"` // RFC 3339 UTC
}

func newUptimeEvent(status Status, timestamp time.Time) UptimeEvent {
	return UptimeEvent{
		State:     status,
		Timestamp: timestamp.UTC().Format(time.RFC3339),
	}
}

type Entry map[Device]time.Time

const persistentDataPath = "/app/data/uptime.json"

func NewUptimeTracker(mqttClient mqtt.Client, path string) (*UptimeTracker, error) {
	ut := &UptimeTracker{mqttClient: mqttClient, pathPersistentData: path}
	if err := ut.ensure(); err != nil {
		return nil, err
	}
	return ut, nil
}

func (ut *UptimeTracker) ensure() error {
	if _, err := os.Stat(ut.pathPersistentData); err == nil {
		return nil
	} else if !os.IsNotExist(err) {
		return err
	}

	if err := os.MkdirAll(filepath.Dir(ut.pathPersistentData), 0755); err != nil {
		return err
	}

	entry := Entry{DeviceLab: {}, DeviceWatchdog: {}}
	out, err := json.MarshalIndent(entry, "", " ")
	if err != nil {
		return err
	}
	return os.WriteFile(ut.pathPersistentData, out, 0644)
}

func (ut *UptimeTracker) storeState(lastPing time.Time, device Device) error {
	if err := ut.ensure(); err != nil {
		return err
	}

	data, err := os.ReadFile(ut.pathPersistentData)
	if err != nil {
		return err
	}

	var entry Entry
	if err := json.Unmarshal(data, &entry); err != nil {
		return err
	}

	entry[device] = lastPing

	out, err := json.MarshalIndent(entry, "", " ")
	if err != nil {
		return err
	}

	return os.WriteFile(ut.pathPersistentData, out, 0644)
}

func (ut *UptimeTracker) loadState(device Device) (time.Time, error) {
	data, err := os.ReadFile(ut.pathPersistentData)
	if err != nil {
		return time.Time{}, err
	}

	var entry Entry
	if err := json.Unmarshal(data, &entry); err != nil {
		return time.Time{}, err
	}

	return entry[device], nil
}

func (ut *UptimeTracker) publish(device Device, status Status, timestamp time.Time) error {
	payload, err := json.Marshal(newUptimeEvent(status, timestamp))
	if err != nil {
		return err
	}

	token := ut.mqttClient.Publish(device.Topic(), 0, false, payload)
	token.Wait()
	return token.Error()
}

const (
	TopicPing           = "watchdog/ping"
	TopicUptimeLab      = "events/uptime/lab"
	TopicUptimeWatchdog = "events/uptime/watchdog"
)

func NewWatchdog(broker string, user string, password string, dataPath string) (*Watchdog, error) {
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
		if !w.labPublished {
			w.publishLabRecovery(time.Now())
			w.labPublished = true
		}
	})
	opts.SetConnectionLostHandler(func(c mqtt.Client, err error) {
		log.Printf("MQTT connection lost: %v\n", err)
	})

	w.mqttClient = mqtt.NewClient(opts)

	tracker, err := NewUptimeTracker(w.mqttClient, dataPath)
	if err != nil {
		return nil, err
	}
	w.tracker = tracker

	return w, nil
}

func (w *Watchdog) Connect() error {
	token := w.mqttClient.Connect()
	token.Wait()
	return token.Error()
}

func (w *Watchdog) onPing(_ mqtt.Client, _ mqtt.Message) {
	w.lastPing = time.Now()
	if w.tracker != nil {
		if err := w.tracker.storeState(w.lastPing, DeviceWatchdog); err != nil {
			log.Printf("Failed to persist watchdog state: %v\n", err)
		}
	}
	if w.alertSent {
		log.Println("Sending recovery alert")
		if err := w.sendMessage("Esp32 watchdog is responding again"); err != nil {
			log.Printf("Failed to send recovery alert: %v\n", err)
		}
		if w.tracker != nil {
			if err := w.tracker.publish(DeviceWatchdog, StatusUp, w.lastPing); err != nil {
				log.Printf("Failed to publish watchdog up event. %v\n", err)
			}
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
				if w.tracker != nil {
					if err := w.tracker.publish(DeviceWatchdog, StatusDown, w.lastPing); err != nil {
						log.Printf("Failed to publish watchdog down event: %v\n", err)
					}
				}
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

func (w *Watchdog) publishLabRecovery(bootTime time.Time) {
	if w.tracker == nil {
		return
	}

	lastPing, err := w.tracker.loadState(DeviceWatchdog)
	if err != nil {
		log.Printf("Failed to load last ping for lab event: %v\n", err)
		return
	}
	if lastPing.IsZero() {
		return
	}

	if err := w.tracker.publish(DeviceLab, StatusDown, lastPing); err != nil {
		log.Printf("Failed to publish lab down event: %v\n", err)
	}
	if err := w.tracker.publish(DeviceLab, StatusUp, bootTime); err != nil {
		log.Printf("Failed to publish lab up event: %v\n", err)
	}
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

	w, err := NewWatchdog(broker, user, password, persistentDataPath)
	if err != nil {
		log.Fatalf("Failed to initialize watchdog: %v\n", err)
	}

	if err := w.Connect(); err != nil {
		log.Printf("Initial MQTT connect pending: %v (will keep retrying)\n", err)
	}

	w.StartTimeoutChecker()

	select {}
}
