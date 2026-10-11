package main

import (
	"context"
	"errors"
	"sync"
	"time"

	"github.com/wailsapp/wails/v2/pkg/runtime"
	"github.com/yuxino/Force/internal/focus"
	"github.com/yuxino/Force/internal/notch"
)

type App struct {
	ctx    context.Context
	client *focus.Client
	mu     sync.Mutex // Serialize snapshots with mutations: no stale response can undo a completed click.
}

func (a *App) startup(ctx context.Context) { a.ctx = ctx }
func (a *App) ready(ctx context.Context) {
	notch.Configure(func(event int) {
		switch event {
		case 1:
			runtime.EventsEmit(ctx, "force:expand")
		case 2:
			runtime.EventsEmit(ctx, "force:collapse")
		case 3:
			runtime.EventsEmit(ctx, "force:geometry", notch.CurrentGeometry())
		case 4:
			runtime.Quit(ctx)
		case 5:
			runtime.EventsEmit(ctx, "force:refresh")
		}
	})
	runtime.WindowShow(ctx)
}
func (a *App) shutdown(ctx context.Context) { notch.Shutdown() }

func (a *App) GetGeometry() notch.Geometry { return notch.CurrentGeometry() }
func (a *App) SetExpanded(expanded bool)   { notch.Expand(expanded) }
func (a *App) Quit()                       { runtime.Quit(a.ctx) }

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
