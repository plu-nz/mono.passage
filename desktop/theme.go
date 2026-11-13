package main

import (
	"log"

	"github.com/godbus/dbus/v5"
)

// themeChangedEvent tells the UI that the desktop color scheme changed. The
// payload is true for dark. Wails v3.0.0 reads the scheme at startup but does
// not deliver its Linux change event, so Passage watches the portal itself.
const themeChangedEvent = "passage:theme-changed"

// watchColorScheme calls onChange whenever the XDG Settings portal reports a
// new org.freedesktop.appearance color-scheme (1 = prefer dark).
func watchColorScheme(onChange func(dark bool)) {
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
	for signal := range signals {
		if len(signal.Body) < 3 {
			continue
		}
		value, ok := signal.Body[2].(dbus.Variant)
		if !ok {
			continue
		}
		scheme, ok := value.Value().(uint32)
		if !ok {
			continue
		}
		onChange(scheme == 1)
	}
}
