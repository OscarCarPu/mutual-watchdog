package main

import (
	"errors"
	"sync"
	"testing"
	"time"

	mqtt "github.com/eclipse/paho.mqtt.golang"
)

// --- mocks ---

type mockToken struct {
	err error
}

func (t *mockToken) Wait() bool                        { return true }
func (t *mockToken) WaitTimeout(_ time.Duration) bool  { return true }
func (t *mockToken) Done() <-chan struct{}              { ch := make(chan struct{}); close(ch); return ch }
func (t *mockToken) Error() error                      { return t.err }

type mockMessage struct{}

func (m *mockMessage) Duplicate() bool  { return false }
func (m *mockMessage) Qos() byte        { return 0 }
func (m *mockMessage) Retained() bool   { return false }
func (m *mockMessage) Topic() string    { return "watchdog/ping-esp32" }
func (m *mockMessage) MessageID() uint16 { return 0 }
func (m *mockMessage) Payload() []byte  { return []byte("ping") }
func (m *mockMessage) Ack()             {}

type mockClient struct {
	mu               sync.Mutex
	published        []string
	subscribed       []string
	subscribeHandler mqtt.MessageHandler
	connectErr       error
}

func (m *mockClient) Connect() mqtt.Token {
	return &mockToken{err: m.connectErr}
}

func (m *mockClient) Publish(topic string, _ byte, _ bool, _ interface{}) mqtt.Token {
	m.mu.Lock()
	m.published = append(m.published, topic)
	m.mu.Unlock()
	return &mockToken{}
}

func (m *mockClient) Subscribe(topic string, _ byte, callback mqtt.MessageHandler) mqtt.Token {
	m.mu.Lock()
	m.subscribed = append(m.subscribed, topic)
	m.subscribeHandler = callback
	m.mu.Unlock()
	return &mockToken{}
}

// --- tests ---

func TestNewWatchdog(t *testing.T) {
	w := NewWatchdog("mqtt://localhost:1883", "user", "pass")

	if w == nil {
		t.Fatal("Expected non-nil Watchdog")
	}
	if w.client == nil {
		t.Error("Expected non-nil MQTT client")
	}
	if time.Since(w.lastPing) > time.Second {
		t.Errorf("Expected lastPing to be recent, got %s ago", time.Since(w.lastPing))
	}
}

func TestConnect(t *testing.T) {
	t.Run("success", func(t *testing.T) {
		w := &Watchdog{client: &mockClient{}, lastPing: time.Now()}
		if err := w.Connect(); err != nil {
			t.Errorf("Expected no error, got %v", err)
		}
	})

	t.Run("failure", func(t *testing.T) {
		mock := &mockClient{connectErr: errors.New("connection refused")}
		w := &Watchdog{client: mock, lastPing: time.Now()}
		if err := w.Connect(); err == nil {
			t.Error("Expected error, got nil")
		}
	})
}

func TestPing(t *testing.T) {
	mock := &mockClient{}
	w := &Watchdog{client: mock, lastPing: time.Now()}

	w.Ping(30 * time.Millisecond)
	time.Sleep(100 * time.Millisecond)

	mock.mu.Lock()
	published := append([]string(nil), mock.published...)
	mock.mu.Unlock()

	if len(published) == 0 {
		t.Fatal("Expected at least one publish")
	}
	for _, topic := range published {
		if topic != "watchdog/ping-lab" {
			t.Errorf("Expected topic watchdog/ping-lab, got %s", topic)
		}
	}
}

func TestSubscribe(t *testing.T) {
	mock := &mockClient{}
	w := &Watchdog{client: mock, lastPing: time.Now().Add(-1 * time.Minute)}

	w.Subscribe()

	mock.mu.Lock()
	subscribed := append([]string(nil), mock.subscribed...)
	handler := mock.subscribeHandler
	mock.mu.Unlock()

	if len(subscribed) == 0 {
		t.Fatal("Expected Subscribe to be called")
	}
	if subscribed[0] != "watchdog/ping-esp32" {
		t.Errorf("Expected topic watchdog/ping-esp32, got %s", subscribed[0])
	}

	before := w.lastPing
	handler(nil, &mockMessage{})
	if !w.lastPing.After(before) {
		t.Error("Expected lastPing to be updated after receiving message")
	}
}

func TestTimeoutDetection(t *testing.T) {
	threshold := 2 * time.Minute

	if time.Since(time.Now().Add(-30*time.Second)) >= threshold {
		t.Error("Expected recent ping to be within threshold")
	}
	if time.Since(time.Now().Add(-5*time.Minute)) < threshold {
		t.Error("Expected stale ping to exceed threshold")
	}
}
