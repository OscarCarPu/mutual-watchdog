package main

import (
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
