package main

import (
	"context"
	"encoding/json"
	"errors"
	"log"
	"sync"
	"time"

	"github.com/wailsapp/wails/v2/pkg/runtime"
	"github.com/yuxino/Force/internal/desktop"
	"github.com/yuxino/Force/internal/focus"
)

type App struct {
	ctx           context.Context
	initialView   string
	inspectWindow bool
	client        *focus.Client
	mu            sync.Mutex // Serialize snapshots with mutations: no stale response can undo a completed click.
}

func (a *App) startup(ctx context.Context) { a.ctx = ctx }
func (a *App) ready(ctx context.Context) {
	desktop.Configure(func(event int) {
		switch event {
		case 1:
			runtime.EventsEmit(ctx, "force:expand")
		case 2:
			runtime.EventsEmit(ctx, "force:collapse")
		case 3:
			runtime.EventsEmit(ctx, "force:geometry", desktop.CurrentGeometry())
		case 4:
			runtime.Quit(ctx)
		case 5:
			runtime.EventsEmit(ctx, "force:refresh")
		}
	}, a.readDetails)
	runtime.WindowShow(ctx)
	if a.inspectWindow {
		log.Print(desktop.InspectWindow())
	}
	if a.initialView == "history" || a.initialView == "week" {
		desktop.ShowDetails(a.initialView)
	}
}
func (a *App) shutdown(ctx context.Context) { desktop.Shutdown() }

func (a *App) GetGeometry() desktop.Geometry { return desktop.CurrentGeometry() }
func (a *App) SetExpanded(expanded bool)     { desktop.Expand(expanded) }
func (a *App) Quit()                         { runtime.Quit(a.ctx) }

func (a *App) ReadState() (focus.Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.client.Read(a.ctx, time.Now())
}

func (a *App) SetTagRunning(tag string, running bool) (focus.Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	var err error
	if running {
		err = a.client.Start(a.ctx, tag)
	} else {
		err = a.client.Stop(a.ctx, tag)
	}
	if err != nil {
		return focus.Snapshot{}, err
	}
	return a.client.Read(a.ctx, time.Now())
}

func (a *App) StopAll() (focus.Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	if err := a.client.Stop(a.ctx, ""); err != nil {
		return focus.Snapshot{}, err
	}
	return a.client.Read(a.ctx, time.Now())
}

func (a *App) CreateTag(name, kind string) (focus.Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	if err := a.client.CreateTag(a.ctx, name, kind); err != nil {
		return focus.Snapshot{}, err
	}
	return a.client.Read(a.ctx, time.Now())
}

func (a *App) ReadHistory(startISO, endISO string) ([]focus.Log, error) {
	start, err := time.Parse(time.RFC3339, startISO)
	if err != nil {
		return nil, errors.New("日期无效")
	}
	end, err := time.Parse(time.RFC3339, endISO)
	if err != nil || end.Before(start) || end.Sub(start) > 32*24*time.Hour {
		return nil, errors.New("日期范围无效")
	}
	return a.client.Logs(a.ctx, start, end)
}

func (a *App) ReadWeek() (focus.WeekStats, error) { return a.client.Week(a.ctx) }

func (a *App) HideToMenu() { desktop.HideToMenu() }
func (a *App) ShowDetails(view string) {
	if view == "history" || view == "week" {
		desktop.ShowDetails(view)
	}
}
func (a *App) UpdateMenu(time string) {
	if len(time) > 12 {
		return
	}
	for _, char := range time {
		if (char < '0' || char > '9') && char != ':' {
			return
		}
	}
	desktop.UpdateMenu(time)
}

// The records window can only read. Main-window mutations remain on Wails' bridge.
func (a *App) readDetails(payload string) string {
	var request struct {
		ID     string            `json:"id"`
		Method string            `json:"method"`
		Args   []json.RawMessage `json:"args"`
	}
	var result any
	var err error
	if len(payload) > 4096 || json.Unmarshal([]byte(payload), &request) != nil || len(request.ID) == 0 || len(request.ID) > 128 {
		err = errors.New("请求无效")
	} else {
		switch request.Method {
		case "ReadState":
			if len(request.Args) == 0 {
				result, err = a.ReadState()
			} else {
				err = errors.New("请求无效")
			}
		case "ReadWeek":
			if len(request.Args) == 0 {
				result, err = a.ReadWeek()
			} else {
				err = errors.New("请求无效")
			}
		case "ReadHistory":
			var start, end string
			if len(request.Args) == 2 && json.Unmarshal(request.Args[0], &start) == nil && json.Unmarshal(request.Args[1], &end) == nil {
				result, err = a.ReadHistory(start, end)
			} else {
				err = errors.New("请求无效")
			}
		default:
			err = errors.New("记录窗口只支持读取")
		}
	}
	response := struct {
		ID     string `json:"id"`
		Result any    `json:"result"`
		Error  string `json:"error,omitempty"`
	}{ID: request.ID, Result: result}
	if err != nil {
		response.Error = err.Error()
	}
	raw, _ := json.Marshal(response)
	return string(raw)
}
