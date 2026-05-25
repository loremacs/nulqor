package mcp

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"time"
)

// RemoteHarness talks to a running harness HTTP API (e.g. wails dev on :8080).
type RemoteHarness struct {
	baseURL    string
	httpClient *http.Client
}

func NewRemoteHarness(baseURL string) *RemoteHarness {
	return &RemoteHarness{
		baseURL: baseURL,
		httpClient: &http.Client{
			Timeout: 120 * time.Second,
		},
	}
}

func (h *RemoteHarness) RegisterObserver(name string) (map[string]interface{}, error) {
	return h.postJSON("/observers/register", map[string]string{"name": name})
}

func (h *RemoteHarness) CatchUp(observer string, autoAck bool) (map[string]interface{}, error) {
	values := url.Values{}
	values.Set("observer", observer)
	if autoAck {
		values.Set("auto_ack", "true")
	}
	return h.getJSON("/observers/catch-up?" + values.Encode())
}

func (h *RemoteHarness) AckObserver(name string) (map[string]interface{}, error) {
	return h.postJSON("/observers/ack", map[string]string{"name": name})
}

func (h *RemoteHarness) SendMessage(observerName, message, model, agent string) (map[string]interface{}, error) {
	body := map[string]string{
		"message":       message,
		"model":         model,
		"agent":         agent,
		"observer_name": observerName,
	}
	return h.postJSON("/message", body)
}

func (h *RemoteHarness) ListObservers() (map[string]interface{}, error) {
	return h.getJSON("/observers")
}

func (h *RemoteHarness) getJSON(path string) (map[string]interface{}, error) {
	req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, h.baseURL+path, nil)
	if err != nil {
		return nil, err
	}
	return h.do(req)
}

func (h *RemoteHarness) postJSON(path string, payload interface{}) (map[string]interface{}, error) {
	data, err := json.Marshal(payload)
	if err != nil {
		return nil, err
	}
	req, err := http.NewRequestWithContext(context.Background(), http.MethodPost, h.baseURL+path, bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	return h.do(req)
}

func (h *RemoteHarness) do(req *http.Request) (map[string]interface{}, error) {
	resp, err := h.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode >= 400 {
		return nil, fmt.Errorf("harness API %s: %s", resp.Status, string(body))
	}

	var out map[string]interface{}
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	return out, nil
}
