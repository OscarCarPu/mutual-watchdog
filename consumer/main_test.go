package main

import (
	"encoding/json"
	"path/filepath"
	"testing"
	"time"
)

func newTestWatchdog(lastPing time.Time, alertSent bool) (*Watchdog, *bool) {
	called := false
	w := &Watchdog{
		lastPing:  lastPing,
		alertSent: alertSent,
		sendMessage: func(msg string) error {
			called = true
			return nil
		},
	}
	return w, &called
}

func TestTimeoutChecker(t *testing.T) {
	const timeout = 100 * time.Millisecond

	tests := []struct {
		name          string
		lastPing      time.Time
		alertSent     bool
		wantCall      bool
		wantAlertSent bool
	}{
		{"no timeout", time.Now(), false, false, false},
		{"timeout no alert sent", time.Now().Add(-1 * time.Second), false, true, true},
		{"timeout alert already sent", time.Now().Add(-1 * time.Second), true, false, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			w, called := newTestWatchdog(tt.lastPing, tt.alertSent)

			w.checkTimeout(timeout)

			if *called != tt.wantCall {
				t.Fatalf("called=%v, want=%v", *called, tt.wantCall)
			}

			if w.alertSent != tt.wantAlertSent {
				t.Fatalf("alertSent=%v, want=%v", w.alertSent, tt.wantAlertSent)
			}
		})
	}
}

func TestUptimeTrackerStoreLoad(t *testing.T) {
	tests := []struct {
		name   string
		device Device
	}{
		{"watchdog device", DeviceWatchdog},
		{"lab device", DeviceLab},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "uptime.json")
			ut, err := NewUptimeTracker(nil, path)
			if err != nil {
				t.Fatalf("NewUptimeTracker: %v", err)
			}

			want := time.Now().Truncate(time.Second)
			if err := ut.storeState(want, tt.device); err != nil {
				t.Fatalf("storeState: %v", err)
			}

			got, err := ut.loadState(tt.device)
			if err != nil {
				t.Fatalf("loadState: %v", err)
			}

			if !got.Equal(want) {
				t.Fatalf("loadState=%v, want=%v", got, want)
			}
		})
	}
}

func TestPingReceived(t *testing.T) {
	tests := []struct {
		name          string
		alertSent     bool
		wantCall      bool
		wantAlertSent bool
	}{
		{"no alert sent", false, false, false},
		{"alert sent sends recovery", true, true, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			w, called := newTestWatchdog(time.Now(), tt.alertSent)

			w.onPing(nil, nil)

			if *called != tt.wantCall {
				t.Fatalf("called=%v, want=%v", *called, tt.wantCall)
			}

			if w.alertSent != tt.wantAlertSent {
				t.Fatalf("alertSent=%v, want=%v", w.alertSent, tt.wantAlertSent)
			}
		})
	}
}

func TestNewUptimeEvent(t *testing.T) {
	utc := time.Date(2026, 5, 31, 4, 1, 0, 0, time.UTC)
	cest := time.Date(2026, 5, 31, 6, 1, 0, 0, time.FixedZone("CEST", 2*60*60))

	tests := []struct {
		name          string
		status        Status
		timestamp     time.Time
		wantState     Status
		wantTimestamp string
	}{
		{"up", StatusUp, utc, StatusUp, "2026-05-31T04:01:00Z"},
		{"down", StatusDown, utc, StatusDown, "2026-05-31T04:01:00Z"},
		{"normalizes to utc", StatusUp, cest, StatusUp, "2026-05-31T04:01:00Z"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := newUptimeEvent(tt.status, tt.timestamp)
			if got.State != tt.wantState {
				t.Fatalf("State=%q, want=%q", got.State, tt.wantState)
			}
			if got.Timestamp != tt.wantTimestamp {
				t.Fatalf("Timestamp=%q, want=%q", got.Timestamp, tt.wantTimestamp)
			}
		})
	}
}

func TestUptimeEventJSON(t *testing.T) {
	ts := time.Date(2026, 5, 31, 4, 1, 0, 0, time.UTC)
	got, err := json.Marshal(newUptimeEvent(StatusUp, ts))
	if err != nil {
		t.Fatalf("Marshal: %v", err)
	}

	want := `{"state":"up","timestamp":"2026-05-31T04:01:00Z"}`
	if string(got) != want {
		t.Fatalf("json=%s, want=%s", got, want)
	}
}

func TestDeviceTopic(t *testing.T) {
	tests := []struct {
		name   string
		device Device
		want   string
	}{
		{"lab device", DeviceLab, "events/uptime/lab"},
		{"watchdog device", DeviceWatchdog, "events/uptime/watchdog"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.device.Topic(); got != tt.want {
				t.Fatalf("Topic()=%q, want=%q", got, tt.want)
			}
		})
	}
}
