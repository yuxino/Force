package main

import (
	"embed"
	"flag"
	"log"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/mac"
	"github.com/wailsapp/wails/v2/pkg/runtime"
	"github.com/yuxino/Force/internal/focus"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
	base := flag.String("api-base-url", focus.DefaultURL, "Nichijou API address (HTTPS or localhost test server)")
	flag.Parse()
	client, err := focus.NewClient(*base)
	if err != nil {
		log.Fatal(err)
	}
	app := &App{client: client}
	err = wails.Run(&options.App{
		Title: "Force", Width: 456, Height: 500, Frameless: true, DisableResize: true,
		AlwaysOnTop: true, StartHidden: true, BackgroundColour: &options.RGBA{R: 0, G: 0, B: 0, A: 0},
		AssetServer: &assetserver.Options{Assets: assets},
		OnStartup:   app.startup, OnDomReady: app.ready, OnShutdown: app.shutdown,
		Bind:               []interface{}{app},
		SingleInstanceLock: &options.SingleInstanceLock{UniqueId: "app.yuxino.force", OnSecondInstanceLaunch: func(data options.SecondInstanceData) { runtime.EventsEmit(app.ctx, "force:expand") }},
		Mac: &mac.Options{TitleBar: mac.TitleBarHidden(), WebviewIsTransparent: true, WindowIsTranslucent: false,
			Appearance: mac.NSAppearanceNameDarkAqua, DisableZoom: true, DisableEscapeExitsFullscreen: true,
			Preferences: &mac.Preferences{TabFocusesLinks: mac.Enabled}},
	})
	if err != nil {
		log.Fatal(err)
	}
}
