package main

/*
#cgo pkg-config: gtk4
#include <stdlib.h>
#include <gtk/gtk.h>

extern void passageGtkSettingsChanged(void);

static GtkWidget *passage_probe = NULL;

// passage_lookup_color resolves a named color (@define-color) from the active
// GTK theme. It returns FALSE when the theme does not define the name.
static gboolean passage_lookup_color(const char *name, GdkRGBA *out) {
	if (passage_probe == NULL) {
		passage_probe = g_object_ref_sink(gtk_label_new(NULL));
	}
	G_GNUC_BEGIN_IGNORE_DEPRECATIONS
	gboolean found = gtk_style_context_lookup_color(gtk_widget_get_style_context(passage_probe), name, out);
	G_GNUC_END_IGNORE_DEPRECATIONS
	return found;
}

static char *passage_font_name(void) {
	char *font = NULL;
	g_object_get(gtk_settings_get_default(), "gtk-font-name", &font, NULL);
	return font;
}

static void passage_settings_notify(GObject *object, GParamSpec *pspec, gpointer data) {
	passageGtkSettingsChanged();
}

static void passage_watch_settings(void) {
	GtkSettings *settings = gtk_settings_get_default();
	const char *signals[] = {
		"notify::gtk-theme-name",
		"notify::gtk-application-prefer-dark-theme",
		"notify::gtk-interface-color-scheme",
		"notify::gtk-font-name",
	};
	for (guint i = 0; i < G_N_ELEMENTS(signals); i++) {
		g_signal_connect(settings, signals[i], G_CALLBACK(passage_settings_notify), NULL);
	}
}
*/
import "C"

import (
	"fmt"
	"math"
	"unsafe"

	"github.com/wailsapp/wails/v3/pkg/application"
)

// systemColorNames maps each UI role to GTK named colors, newest naming
// (libadwaita-style) first, then the classic GTK theme names.
var systemColorNames = map[string][]string{
	"fg":          {"window_fg_color", "theme_fg_color"},
	"bg":          {"window_bg_color", "theme_bg_color"},
	"base":        {"view_bg_color", "theme_base_color"},
	"text":        {"view_fg_color", "theme_text_color"},
	"accent":      {"accent_bg_color", "theme_selected_bg_color"},
	"accent-fg":   {"accent_fg_color", "theme_selected_fg_color"},
	"border":      {"borders"},
	"placeholder": {"placeholder_text_color"},
	"error":       {"error_color"},
	"warning":     {"warning_color"},
	"success":     {"success_color"},
}

var watchingGtkSettings bool

// readSystemTheme must run on the GTK main thread.
func readSystemTheme() SystemTheme {
	if !watchingGtkSettings {
		C.passage_watch_settings()
		watchingGtkSettings = true
	}
	theme := SystemTheme{Colors: map[string]string{}}
	var bg C.GdkRGBA
	for role, names := range systemColorNames {
		for _, name := range names {
			var color C.GdkRGBA
			cname := C.CString(name)
			found := C.passage_lookup_color(cname, &color) != 0
			C.free(unsafe.Pointer(cname))
			if found {
				theme.Colors[role] = cssColor(color)
				if role == "bg" {
					bg = color
				}
				break
			}
		}
	}
	if _, ok := theme.Colors["bg"]; ok {
		theme.Dark = luminance(bg) < 0.5
	} else if app := application.Get(); app != nil {
		theme.Dark = app.Env.IsDarkMode()
	}
	if font := C.passage_font_name(); font != nil {
		theme.Font = C.GoString(font)
		C.g_free(C.gpointer(font))
	}
	return theme
}

func cssColor(c C.GdkRGBA) string {
	channel := func(v C.float) int { return int(math.Round(float64(v) * 255)) }
	return fmt.Sprintf("rgba(%d, %d, %d, %.3g)", channel(c.red), channel(c.green), channel(c.blue), float64(c.alpha))
}

// luminance is the WCAG relative luminance of an sRGB color.
func luminance(c C.GdkRGBA) float64 {
	linear := func(v C.float) float64 {
		x := float64(v)
		if x <= 0.03928 {
			return x / 12.92
		}
		return math.Pow((x+0.055)/1.055, 2.4)
	}
	return 0.2126*linear(c.red) + 0.7152*linear(c.green) + 0.0722*linear(c.blue)
}
