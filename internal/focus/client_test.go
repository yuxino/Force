package focus

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestReadSnapshotAndMonthRange(t *testing.T) {
	now := time.Date(2026, 10, 12, 0, 20, 0, 0, time.FixedZone("Shanghai", 8*3600))
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Query().Get("userId") != OwnerID {
			t.Errorf("wrong profile")
		}
		switch r.URL.Path {
		case "/focus/tags":
			if r.URL.Query().Get("includeArchived") != "true" {
				t.Error("archived tags are needed for historical kind lookup")
			}
			_, _ = w.Write([]byte(`[{"id":"tag-1","name":"学习","kind":"positive","sortOrder":0,"archivedAt":null}]`))
		case "/focus/current":
			_, _ = w.Write([]byte(`[{"id":"s-1","tag":"学习","startTime":"2026-10-11T15:00:00.123Z"}]`))
		case "/focus/logs":
			if r.URL.Query().Get("startDate") != "2026-09-30T16:00:00Z" {
				t.Errorf("wrong local month start: %s", r.URL.RawQuery)
			}
			if r.URL.Query().Get("endDate") != "2026-10-31T15:59:59.999Z" {
				t.Error("wrong local month end")
			}
			_, _ = w.Write([]byte(`[{"id":"l-1","tag":"学习","startTime":"2026-10-11T01:00:00Z","endTime":"2026-10-11T02:00:00Z","duration":3600}]`))
		default:
			t.Errorf("unexpected path %s", r.URL.Path)
			http.NotFound(w, r)
		}
	}))
	defer server.Close()
	client, _ := NewClient(server.URL)
	state, err := client.Read(context.Background(), now)
	if err != nil {
		t.Fatal(err)
	}
	if len(state.Tags) != 1 || len(state.Sessions) != 1 || len(state.Logs) != 1 || state.SyncedAt == "" {
		t.Fatalf("incomplete snapshot: %+v", state)
	}
	if state.Sessions[0].StartTime != "2026-10-11T15:00:00.123Z" {
		t.Fatal("lost fractional server timestamp")
	}
}

func TestLegacySessions(t *testing.T) {
	for _, body := range []string{`null`, `[]`, `{"id":"one","tag":"工作","startTime":"2026-10-12T00:00:00Z"}`} {
		t.Run(body, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { _, _ = w.Write([]byte(body)) }))
			defer server.Close()
			client, _ := NewClient(server.URL)
			sessions, err := client.Sessions(context.Background())
			if err != nil {
				t.Fatal(err)
			}
			if sessions == nil {
				t.Fatal("must encode [] rather than null")
			}
		})
	}
}

func TestNullCollectionsRemainSafeForFrontend(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { _, _ = w.Write([]byte(`null`)) }))
	defer server.Close()
	client, _ := NewClient(server.URL)
	state, err := client.Read(context.Background(), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if state.Tags == nil || state.Logs == nil || state.Sessions == nil {
		t.Fatal("null collections would crash the frontend rather than show an empty state")
	}
}

func TestExplicitMutations(t *testing.T) {
	var requests []map[string]string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" || r.Header.Get("Content-Type") != "application/json" {
			t.Error("wrong method/content type")
		}
		var body map[string]string
		if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
			t.Fatal(err)
		}
		body["path"] = r.URL.Path
		requests = append(requests, body)
		_, _ = w.Write([]byte(`{}`))
	}))
	defer server.Close()
	client, _ := NewClient(server.URL)
	if err := client.Start(context.Background(), "操作系统"); err != nil {
		t.Fatal(err)
	}
	if err := client.Stop(context.Background(), "操作系统"); err != nil {
		t.Fatal(err)
	}
	if err := client.Stop(context.Background(), ""); err != nil {
		t.Fatal(err)
	}
	if err := client.CreateTag(context.Background(), "  阅读  ", "positive"); err != nil {
		t.Fatal(err)
	}
	if requests[0]["path"] != "/focus/start" || requests[0]["tag"] != "操作系统" || requests[0]["userId"] != OwnerID {
		t.Fatal(requests)
	}
	if _, ok := requests[2]["tag"]; ok {
		t.Fatal("stop-all must omit tag")
	}
	if requests[3]["name"] != "阅读" || requests[3]["kind"] != "positive" {
		t.Fatal(requests)
	}
}

func TestHTTPFailureAndAlreadyStopped(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.WriteHeader(404) }))
	defer server.Close()
	client, _ := NewClient(server.URL)
	if err := client.Stop(context.Background(), "学习"); err != nil {
		t.Fatal("already stopped should reconcile", err)
	}
	if _, err := client.Sessions(context.Background()); err == nil {
		t.Fatal("failed read must not replace state with empty data")
	}
	if err := client.Start(context.Background(), "学习"); err == nil {
		t.Fatal("failed write must remain an error")
	}
}

func TestAddressAndTagValidation(t *testing.T) {
	for _, address := range []string{"http://example.com", "ftp://localhost", "https://user:pass@example.com", "https://example.com?key=x"} {
		if _, err := NewClient(address); err == nil {
			t.Errorf("accepted invalid address %s", address)
		}
	}
	client, _ := NewClient("http://localhost:3459")
	if err := client.CreateTag(context.Background(), " ", "positive"); err == nil {
		t.Fatal("blank tag accepted")
	}
	if err := client.CreateTag(context.Background(), "工作", "bad"); err == nil {
		t.Fatal("invalid kind accepted")
	}
}
