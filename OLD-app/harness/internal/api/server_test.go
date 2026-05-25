package api

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"harness/internal/engine"
	"harness/internal/lmstudio"
)

func TestObserverRegisterCatchUpAndAckHTTP(t *testing.T) {
	eng := testEngine(t)
	srv := NewServer(eng, lmstudio.NewClient("http://localhost:1234/v1", ""))
	mux := srv.Handler()

	// Register observer A
	rec := httptest.NewRecorder()
	body := bytes.NewBufferString(`{"name":"Cursor-test-a"}`)
	req := httptest.NewRequest(http.MethodPost, "/observers/register", body)
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("register status=%d body=%s", rec.Code, rec.Body.String())
	}

	// Register observer B
	rec = httptest.NewRecorder()
	body = bytes.NewBufferString(`{"name":"Windsurf-test-b"}`)
	req = httptest.NewRequest(http.MethodPost, "/observers/register", body)
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("register B status=%d body=%s", rec.Code, rec.Body.String())
	}

	// Append a transcript event after both registered
	session := eng.GetSessions().CreateSession("API-Test")
	user := engine.Message{ID: "m1", Role: "user", Content: "hello from test"}
	if err := eng.GetSessions().AddMessage(session.ID, user); err != nil {
		t.Fatalf("AddMessage: %v", err)
	}
	eng.EmitMessageAddedForTest(user)

	// Observer A should see the event
	rec = httptest.NewRecorder()
	req = httptest.NewRequest(http.MethodGet, "/observers/catch-up?observer=Cursor-test-a", nil)
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("catch-up A status=%d body=%s", rec.Code, rec.Body.String())
	}
	var catchA map[string]interface{}
	if err := json.Unmarshal(rec.Body.Bytes(), &catchA); err != nil {
		t.Fatalf("decode catch-up A: %v", err)
	}
	eventsA, _ := catchA["events"].([]interface{})
	if len(eventsA) != 1 {
		t.Fatalf("observer A expected 1 event, got %d", len(eventsA))
	}

	// Auto-ack flush for A
	rec = httptest.NewRecorder()
	req = httptest.NewRequest(http.MethodGet, "/observers/catch-up?observer=Cursor-test-a&auto_ack=true", nil)
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("catch-up auto_ack status=%d body=%s", rec.Code, rec.Body.String())
	}

	// Observer B still has pending event
	rec = httptest.NewRecorder()
	req = httptest.NewRequest(http.MethodGet, "/observers/catch-up?observer=Windsurf-test-b", nil)
	mux.ServeHTTP(rec, req)
	var catchB map[string]interface{}
	if err := json.Unmarshal(rec.Body.Bytes(), &catchB); err != nil {
		t.Fatalf("decode catch-up B: %v", err)
	}
	eventsB, _ := catchB["events"].([]interface{})
	if len(eventsB) != 1 {
		t.Fatalf("observer B expected 1 pending event, got %d", len(eventsB))
	}

	// Ack B explicitly
	rec = httptest.NewRecorder()
	body = bytes.NewBufferString(`{"name":"Windsurf-test-b"}`)
	req = httptest.NewRequest(http.MethodPost, "/observers/ack", body)
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("ack status=%d body=%s", rec.Code, rec.Body.String())
	}

	// List observers
	rec = httptest.NewRecorder()
	req = httptest.NewRequest(http.MethodGet, "/observers", nil)
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("list status=%d body=%s", rec.Code, rec.Body.String())
	}
}

func TestRegisterObserverRejectsDuplicateNamesCaseInsensitive(t *testing.T) {
	eng := testEngine(t)
	srv := NewServer(eng, lmstudio.NewClient("http://localhost:1234/v1", ""))
	mux := srv.Handler()

	for _, name := range []string{"Agent-One", "agent-one"} {
		rec := httptest.NewRecorder()
		body := bytes.NewBufferString(`{"name":"` + name + `"}`)
		req := httptest.NewRequest(http.MethodPost, "/observers/register", body)
		mux.ServeHTTP(rec, req)
		if rec.Code != http.StatusOK {
			t.Fatalf("register %q status=%d body=%s", name, rec.Code, rec.Body.String())
		}
	}

	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/observers", nil)
	mux.ServeHTTP(rec, req)

	var resp struct {
		Observers []struct {
			Name string `json:"name"`
		} `json:"observers"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("decode list: %v", err)
	}
	if len(resp.Observers) != 1 {
		t.Fatalf("expected 1 observer entry, got %d", len(resp.Observers))
	}
}

func TestMessageRequiresRegisteredObserver(t *testing.T) {
	eng := testEngine(t)
	srv := NewServer(eng, lmstudio.NewClient("http://localhost:1234/v1", ""))
	mux := srv.Handler()

	rec := httptest.NewRecorder()
	body := bytes.NewBufferString(`{"message":"hello","observer_name":"Cursor-missing"}`)
	req := httptest.NewRequest(http.MethodPost, "/message", body)
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("expected 400 for unregistered observer, got %d body=%s", rec.Code, rec.Body.String())
	}

	rec = httptest.NewRecorder()
	body = bytes.NewBufferString(`{"message":"hello"}`)
	req = httptest.NewRequest(http.MethodPost, "/message", body)
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("expected 400 for missing observer_name, got %d body=%s", rec.Code, rec.Body.String())
	}
}

func TestTranscriptIncludesHash(t *testing.T) {
	eng := testEngine(t)
	srv := NewServer(eng, lmstudio.NewClient("http://localhost:1234/v1", ""))
	mux := srv.Handler()

	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/transcript", nil)
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("transcript status=%d body=%s", rec.Code, rec.Body.String())
	}

	var resp map[string]interface{}
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("decode transcript: %v", err)
	}
	if resp["transcript_hash"] == "" {
		t.Fatalf("expected transcript_hash in response")
	}
}

func testEngine(t *testing.T) *engine.Engine {
	t.Helper()
	cfg, err := engine.LoadConfig("../../harness.toml")
	if err != nil {
		t.Fatalf("LoadConfig: %v", err)
	}
	eng := engine.NewEngine(cfg)
	if err := eng.Start(t.Context()); err != nil {
		t.Fatalf("Start: %v", err)
	}
	t.Cleanup(eng.Stop)
	return eng
}
