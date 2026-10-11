package main

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"

	"github.com/yuxino/Force/internal/focus"
)

func TestRecordsBridgeRejectsMutationsAndInvalidRanges(t *testing.T) {
	app := &App{} // Any accidental backend call would fail this test.
	requests := []string{
		`{"id":"1","method":"SetTagRunning","args":["工作",true]}`,
		`{"id":"2","method":"StopAll","args":[]}`,
		`{"id":"3","method":"CreateTag","args":["工作","positive"]}`,
		`{"id":"4","method":"ReadHistory","args":["bad","bad"]}`,
		`{"id":"5","method":"ReadHistory","args":["2026-10-12T00:00:00Z","2026-10-10T00:00:00Z"]}`,
		`{"id":"6","method":"ReadHistory","args":["2026-01-01T00:00:00Z","2026-10-12T00:00:00Z"]}`,
		`{"id":"7","method":"ReadState","args":[1]}`,
		`{"id":"8","method":"ReadWeek","args":[1]}`,
		`{"id":"9","method":"ReadHistory","args":[1,2]}`,
		`{`, strings.Repeat("x", 4097),
	}
	for _, payload := range requests {
		var response struct {
			Error string `json:"error"`
		}
		if err := json.Unmarshal([]byte(app.readDetails(payload)), &response); err != nil || response.Error == "" {
			t.Fatalf("read-only records bridge accepted %q: %+v (%v)", payload, response, err)
		}
	}
}

func TestRecordsBridgeReadsThroughSharedClient(t *testing.T) {
	var writes atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			writes.Add(1)
			http.Error(w, "write", 500)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		if r.URL.Path == "/focus/week-stats" {
			_, _ = w.Write([]byte(`{"dailyStats":[],"tagStats":[]}`))
			return
		}
		_, _ = w.Write([]byte(`[]`))
	}))
	defer server.Close()
	client, err := focus.NewClient(server.URL)
	if err != nil {
		t.Fatal(err)
	}
	app := &App{ctx: context.Background(), client: client}
	requests := []string{
		`{"id":"state","method":"ReadState","args":[]}`,
		`{"id":"week","method":"ReadWeek","args":[]}`,
		`{"id":"history","method":"ReadHistory","args":["2026-10-01T00:00:00Z","2026-10-31T23:59:59Z"]}`,
	}
	for _, payload := range requests {
		var response struct {
			ID     string          `json:"id"`
			Result json.RawMessage `json:"result"`
			Error  string          `json:"error"`
		}
		if err := json.Unmarshal([]byte(app.readDetails(payload)), &response); err != nil || response.Error != "" || response.ID == "" || string(response.Result) == "null" {
			t.Fatalf("records response failed: %+v (%v)", response, err)
		}
	}
	if writes.Load() != 0 {
		t.Fatal("records window wrote to the service")
	}
}
