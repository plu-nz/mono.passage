package main

import (
	"embed"
	"log"
	"os"

	"github.com/wailsapp/wails/v3/pkg/application"
)

//go:embed all:dist
var assets embed.FS

func main() {
	var window *application.WebviewWindow
	app := application.New(application.Options{
		Name: "Passage",
		Assets: application.AssetOptions{
			Handler: application.AssetFileServerFS(assets),
		},
		Services: []application.Service{
			application.NewService(&DesktopService{}),
		},
		// A second launch exits and brings the existing window to the front.
		SingleInstance: &application.SingleInstanceOptions{
			UniqueID: instanceID(),
			OnSecondInstanceLaunch: func(application.SecondInstanceData) {
				if window != nil {
					window.UnMinimise()
					window.Show()
					window.Focus()
				}
			},
		},
	})
	themeApp = app
	go watchColorScheme(scheduleThemeRefresh)
	window = app.Window.NewWithOptions(application.WebviewWindowOptions{
		Title:     "Passage",
		Width:     1060,
		Height:    720,
		MinWidth:  740,
		MinHeight: 520,
	})
	if err := app.Run(); err != nil {
		log.Fatal(err)
	}
}

// instanceID keeps a development run (scripts/dev-desktop.sh) from handing
// off to an installed Passage that is already open.
func instanceID() string {
	if os.Getenv("FRONTEND_DEVSERVER_URL") != "" {
		return "nz.plu.passage.dev"
	}
	return "nz.plu.passage"
}
