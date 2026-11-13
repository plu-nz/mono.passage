package main

import (
	"embed"
	"log"

	"github.com/wailsapp/wails/v3/pkg/application"
)

//go:embed all:dist
var assets embed.FS

func main() {
	app := application.New(application.Options{
		Name: "Passage",
		Assets: application.AssetOptions{
			Handler: application.AssetFileServerFS(assets),
		},
		Services: []application.Service{
			application.NewService(&DesktopService{}),
		},
	})
	go watchColorScheme(func(dark bool) { app.Event.Emit(themeChangedEvent, dark) })
	app.Window.NewWithOptions(application.WebviewWindowOptions{
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
