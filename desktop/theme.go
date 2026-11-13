package main

import (
	"log"
	"sync"
	"time"

	"github.com/godbus/dbus/v5"
	"github.com/wailsapp/wails/v3/pkg/application"
)

// systemThemeEvent carries a fresh SystemTheme to the UI whenever the desktop
// theme, color scheme, or font changes.
const systemThemeEvent = "passage:system-theme"

// SystemTheme is the desktop appearance the UI should follow. Colors holds the
// GTK theme colors that were found, keyed by role (fg, bg, base, accent, ...).
type SystemTheme struct {
	Dark   bool              `json:"dark"`
	Colors map[string]string `json:"colors"`
	Font   string            `json:"font"`
}

// SystemTheme reads the current GTK theme on the GTK main thread.
func (*DesktopService) SystemTheme() SystemTheme {
	return application.InvokeSyncWithResult(readSystemTheme)
}

var (
	themeApp     *application.App
	refreshMu    sync.Mutex
	refreshTimer *time.Timer
)

// scheduleThemeRefresh coalesces bursts of change notifications and lets GTK
// finish reloading its theme before the colors are read again.
func scheduleThemeRefresh() {
	refreshMu.Lock()
	defer refreshMu.Unlock()
	if refreshTimer != nil {
		refreshTimer.Stop()
	}
	refreshTimer = time.AfterFunc(150*time.Millisecond, func() {
		if themeApp != nil {
			themeApp.Event.Emit(systemThemeEvent, application.InvokeSyncWithResult(readSystemTheme))
		}
	})
}

// watchColorScheme calls onChange whenever the XDG Settings portal reports a
// new org.freedesktop.appearance color-scheme. GTK settings notifications cover
// theme and font changes; the portal also covers schemes that do not change
// the GTK theme name.
func watchColorScheme(onChange func()) {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		log.Printf("theme: session bus unavailable: %v", err)
		return
	}
	defer conn.Close()
	if err := conn.AddMatchSignal(
		dbus.WithMatchObjectPath("/org/freedesktop/portal/desktop"),
		dbus.WithMatchInterface("org.freedesktop.portal.Settings"),
		dbus.WithMatchMember("SettingChanged"),
		dbus.WithMatchArg(0, "org.freedesktop.appearance"),
		dbus.WithMatchArg(1, "color-scheme"),
	); err != nil {
		log.Printf("theme: cannot watch color scheme: %v", err)
		return
	}
	signals := make(chan *dbus.Signal, 4)
	conn.Signal(signals)
	for range signals {
		onChange()
	}
}
