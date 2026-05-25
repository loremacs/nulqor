package main

import (
	"embed"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"

	"harness/internal/engine"
)

//go:embed all:frontend/src
var assets embed.FS

func main() {
	// Load configuration
	cfg, err := engine.LoadConfig("harness.toml")
	if err != nil {
		panic(err)
	}

	// Create engine
	eng := engine.NewEngine(cfg)

	// Create an instance of the app structure
	app := NewApp(eng)

	// Create application with options
	err = wails.Run(&options.App{
		Title:  "Nulqor",
		Width:  1024,
		Height: 768,
		AssetServer: &assetserver.Options{
			Assets: assets,
		},
		BackgroundColour: &options.RGBA{R: 27, G: 38, B: 54, A: 1},
		OnStartup:        app.startup,
		OnShutdown:       app.shutdown,
		Bind: []interface{}{
			app,
		},
	})

	if err != nil {
		println("Error:", err.Error())
	}
}
