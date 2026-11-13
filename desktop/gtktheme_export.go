package main

import "C"

// passageGtkSettingsChanged is called by GTK on the main thread when a theme,
// color scheme, or font setting changes. It lives apart from gtktheme.go
// because cgo forbids C definitions in a file that uses //export.
//
//export passageGtkSettingsChanged
func passageGtkSettingsChanged() {
	scheduleThemeRefresh()
}
