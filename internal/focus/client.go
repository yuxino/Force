package focus

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

const DefaultURL = "https://nichijou-api.yuxino.cn"
const OwnerID = "dog" // Nichijou FocusWidget.FOCUS_OWNER_ID; this is a profile ID, not a credential.

type Tag struct {
	ID         string  `json:"id"`
	Name       string  `json:"name"`
	Kind       string  `json:"kind"`
	SortOrder  int     `json:"sortOrder"`
	ArchivedAt *string `json:"archivedAt"`
}

type Session struct {
	ID        string `json:"id"`
	Tag       string `json:"tag"`
	StartTime string `json:"startTime"`
}

type Log struct {
	ID        string `json:"id"`
	Tag       string `json:"tag"`
	StartTime string `json:"startTime"`
	EndTime   string `json:"endTime"`
	Duration  int    `json:"duration"`
}

type Snapshot struct {
	Tags     []Tag     `json:"tags"`
	Sessions []Session `json:"sessions"`
	Logs     []Log     `json:"logs"`
	SyncedAt string    `json:"syncedAt"`
}

type WeekStats struct {
	WeekStart     string `json:"weekStart"`
	WeekEnd       string `json:"weekEnd"`
	TotalDuration int    `json:"totalDuration"`
	TotalCount    int    `json:"totalCount"`
	DailyStats    []struct {
		Date          string `json:"date"`
		TotalDuration int    `json:"totalDuration"`
		Count         int    `json:"count"`
	} `json:"dailyStats"`
	TagStats []struct {
		Tag      string `json:"tag"`
		Duration int    `json:"duration"`
		Count    int    `json:"count"`
	} `json:"tagStats"`
}

type Client struct {
	base string
	http *http.Client
}

func NewClient(base string) (*Client, error) {
	u, err := url.Parse(base)
	if err != nil || u.Host == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" {
		return nil, errors.New("服务地址无效")
	}
	local := u.Hostname() == "localhost" || u.Hostname() == "127.0.0.1" || u.Hostname() == "::1"
	if u.Scheme != "https" && !(u.Scheme == "http" && local) {
		return nil, errors.New("服务地址必须使用 HTTPS（本地测试除外）")
	}
	return &Client{base: strings.TrimRight(base, "/"), http: &http.Client{Timeout: 15 * time.Second}}, nil
}

func (c *Client) request(ctx context.Context, method, path string, body any, out any) error {
	var data io.Reader
	if body != nil {
		encoded, err := json.Marshal(body)
		if err != nil {
			return err
		}
		data = bytes.NewReader(encoded)
	}
	req, err := http.NewRequestWithContext(ctx, method, c.base+path, data)
	if err != nil {
		return err
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("Cache-Control", "no-cache")
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	response, err := c.http.Do(req)
	if err != nil {
		return errors.New("连接中断，请检查网络后重试")
	}
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		if response.StatusCode == 404 && path == "/focus/stop" {
			// Another device may already have stopped it. Reconcile with a fresh snapshot.
			return nil
		}
		return fmt.Errorf("同步失败（HTTP %d），请刷新后重试", response.StatusCode)
	}
	if out == nil {
		_, err = io.Copy(io.Discard, response.Body)
		return err
	}
	if err := json.NewDecoder(io.LimitReader(response.Body, 16<<20)).Decode(out); err != nil {
		return errors.New("无法读取服务器数据，请重试")
	}
	return nil
}

func (c *Client) Sessions(ctx context.Context) ([]Session, error) {
	var raw json.RawMessage
	if err := c.request(ctx, "GET", "/focus/current?userId="+OwnerID, nil, &raw); err != nil {
		return nil, err
	}
	sessions := []Session{}
	if string(raw) == "null" {
		return sessions, nil
	}
	if len(raw) > 0 && raw[0] == '[' {
		if err := json.Unmarshal(raw, &sessions); err != nil {
			return nil, err
		}
	} else {
		var single Session
		if err := json.Unmarshal(raw, &single); err != nil {
			return nil, err
		}
		sessions = append(sessions, single)
	}
	return sessions, nil
}

func (c *Client) Tags(ctx context.Context) ([]Tag, error) {
	tags := []Tag{}
	err := c.request(ctx, "GET", "/focus/tags?userId="+OwnerID+"&includeArchived=true", nil, &tags)
	if tags == nil {
		tags = []Tag{}
	}
	return tags, err
}

func (c *Client) Logs(ctx context.Context, start, end time.Time) ([]Log, error) {
	query := url.Values{"userId": {OwnerID}, "startDate": {start.UTC().Format(time.RFC3339Nano)}, "endDate": {end.UTC().Format(time.RFC3339Nano)}}
	logs := []Log{}
	err := c.request(ctx, "GET", "/focus/logs?"+query.Encode(), nil, &logs)
	if logs == nil {
		logs = []Log{}
	}
	return logs, err
}

func (c *Client) Read(ctx context.Context, now time.Time) (Snapshot, error) {
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	start := time.Date(now.Year(), now.Month(), 1, 0, 0, 0, 0, now.Location())
	end := start.AddDate(0, 1, 0).Add(-time.Millisecond)
	var snapshot Snapshot
	errors := make(chan error, 3)
	go func() { var err error; snapshot.Tags, err = c.Tags(ctx); errors <- err }()
	go func() { var err error; snapshot.Sessions, err = c.Sessions(ctx); errors <- err }()
	go func() { var err error; snapshot.Logs, err = c.Logs(ctx, start, end); errors <- err }()
	for i := 0; i < 3; i++ {
		if err := <-errors; err != nil {
			cancel()
			return Snapshot{}, err
		}
	}
	snapshot.SyncedAt = time.Now().UTC().Format(time.RFC3339Nano)
	return snapshot, nil
}

func (c *Client) Start(ctx context.Context, tag string) error {
	return c.request(ctx, "POST", "/focus/start", map[string]string{"userId": OwnerID, "tag": tag}, nil)
}

func (c *Client) Stop(ctx context.Context, tag string) error {
	body := map[string]string{"userId": OwnerID}
	if tag != "" {
		body["tag"] = tag
	}
	return c.request(ctx, "POST", "/focus/stop", body, nil)
}

func (c *Client) CreateTag(ctx context.Context, name, kind string) error {
	name = strings.TrimSpace(name)
	if name == "" || len([]rune(name)) > 60 {
		return errors.New("标签名称须为 1–60 个字")
	}
	if kind != "positive" && kind != "negative" && kind != "neutral" && kind != "leisure" {
		return errors.New("标签类别无效")
	}
	return c.request(ctx, "POST", "/focus/tags", map[string]string{"userId": OwnerID, "name": name, "kind": kind}, nil)
}

func (c *Client) Week(ctx context.Context) (WeekStats, error) {
	var stats WeekStats
	err := c.request(ctx, "GET", "/focus/week-stats?userId="+OwnerID, nil, &stats)
	return stats, err
}
