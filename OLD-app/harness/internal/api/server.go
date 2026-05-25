package api

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/gorilla/websocket"

	"harness/internal/engine"
	"harness/internal/lmstudio"
)

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

// Server exposes the harness over HTTP and WebSocket.
type Server struct {
	engine   *engine.Engine
	lmClient **lmstudio.Client
	http     *http.Server
	mu       sync.Mutex
}

func NewServer(eng *engine.Engine, lmClient *lmstudio.Client) *Server {
	clientRef := lmClient
	return &Server{
		engine:   eng,
		lmClient: &clientRef,
	}
}

func (s *Server) client() *lmstudio.Client {
	return *s.lmClient
}

func (s *Server) setClient(client *lmstudio.Client) {
	*s.lmClient = client
}

func (s *Server) Handler() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("GET /health", s.handleHealth)
	mux.HandleFunc("GET /models", s.handleModels)
	mux.HandleFunc("POST /connect", s.handleConnect)
	mux.HandleFunc("GET /skills", s.handleSkills)
	mux.HandleFunc("GET /agents", s.handleAgents)
	mux.HandleFunc("GET /rules", s.handleRules)
	mux.HandleFunc("POST /reload", s.handleReload)
	mux.HandleFunc("GET /system-prompt", s.handleSystemPrompt)
	mux.HandleFunc("GET /transcript", s.handleTranscript)
	mux.HandleFunc("POST /message", s.handleMessage)
	mux.HandleFunc("GET /ws/transcript", s.handleTranscriptWS)
	mux.HandleFunc("GET /ws/chat", s.handleChatWS)
	mux.HandleFunc("POST /observers/register", s.handleRegisterObserver)
	mux.HandleFunc("GET /observers", s.handleListObservers)
	mux.HandleFunc("GET /observers/catch-up", s.handleCatchUp)
	mux.HandleFunc("POST /observers/ack", s.handleAckObserver)
	return mux
}

func (s *Server) Start(host string, port int) error {
	cfg := s.engine.GetConfig()
	if host == "" {
		host = cfg.Server.Host
	}
	if port == 0 {
		port = cfg.Server.Port
	}

	s.http = &http.Server{
		Addr:              fmt.Sprintf("%s:%d", host, port),
		Handler:           s.Handler(),
		ReadHeaderTimeout: 10 * time.Second,
	}

	go func() {
		if err := s.http.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			fmt.Printf("Harness API error: %v\n", err)
		}
	}()

	return nil
}

func (s *Server) Stop(ctx context.Context) error {
	if s.http == nil {
		return nil
	}
	return s.http.Shutdown(ctx)
}

func (s *Server) ListenAndServe(host string, port int) error {
	addr := fmt.Sprintf("%s:%d", host, port)
	return http.ListenAndServe(addr, s.Handler())
}

func (s *Server) handleHealth(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, map[string]string{"status": "ok"})
}

func (s *Server) handleModels(w http.ResponseWriter, r *http.Request) {
	models, err := s.client().ListModels(r.Context())
	if err != nil {
		writeError(w, http.StatusBadGateway, err)
		return
	}
	writeJSON(w, map[string]interface{}{"models": models})
}

func (s *Server) handleConnect(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Endpoint string `json:"endpoint"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, err)
		return
	}
	if req.Endpoint == "" {
		req.Endpoint = s.engine.GetConfig().LMStudio.BaseURL
	}

	endpoint := lmstudio.NormalizeBaseURL(req.Endpoint)
	client := lmstudio.NewClient(endpoint, s.engine.GetConfig().LMStudio.APIKey)
	models, err := client.ListModels(r.Context())
	if err != nil {
		writeError(w, http.StatusBadGateway, err)
		return
	}
	if len(models) > 0 {
		client.SetModel(models[0])
	}
	s.setClient(client)

	writeJSON(w, map[string]interface{}{
		"endpoint": endpoint,
		"models":   models,
	})
}

func (s *Server) handleSkills(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, s.engine.GetLoaders().ListSkills())
}

func (s *Server) handleAgents(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, s.engine.GetLoaders().ListAgents())
}

func (s *Server) handleRules(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, s.engine.GetLoaders().GetRules())
}

func (s *Server) handleReload(w http.ResponseWriter, r *http.Request) {
	if err := s.engine.GetLoaders().LoadAll(); err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}
	writeJSON(w, map[string]string{"status": "reloaded"})
}

func (s *Server) handleSystemPrompt(w http.ResponseWriter, r *http.Request) {
	agent := r.URL.Query().Get("agent")
	prompt, err := s.engine.PreviewSystemPrompt(agent)
	if err != nil {
		writeError(w, http.StatusBadRequest, err)
		return
	}
	writeJSON(w, map[string]string{"system_prompt": prompt})
}

func (s *Server) handleTranscript(w http.ResponseWriter, r *http.Request) {
	session, ok := s.engine.GetSessions().GetActiveSession()
	if !ok {
		writeJSON(w, map[string]interface{}{
			"transcript_hash": s.engine.TranscriptHash(),
			"messages":        []engine.Message{},
		})
		return
	}
	writeJSON(w, map[string]interface{}{
		"transcript_hash": s.engine.TranscriptHash(),
		"session":         session,
	})
}

func (s *Server) handleRegisterObserver(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Name string `json:"name"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, err)
		return
	}
	obs, err := s.engine.RegisterObserver(req.Name)
	if err != nil {
		writeError(w, http.StatusBadRequest, err)
		return
	}
	writeJSON(w, obs)
}

func (s *Server) handleListObservers(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, map[string]interface{}{"observers": s.engine.ListObservers()})
}

func (s *Server) handleCatchUp(w http.ResponseWriter, r *http.Request) {
	name := r.URL.Query().Get("observer")
	if name == "" {
		writeError(w, http.StatusBadRequest, fmt.Errorf("observer query param is required"))
		return
	}
	autoAck := r.URL.Query().Get("auto_ack") == "true"
	result, err := s.engine.CatchUp(name, autoAck)
	if err != nil {
		writeError(w, http.StatusBadRequest, err)
		return
	}
	writeJSON(w, result)
}

func (s *Server) handleAckObserver(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Name string `json:"name"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, err)
		return
	}
	obs, err := s.engine.AckObserver(req.Name)
	if err != nil {
		writeError(w, http.StatusBadRequest, err)
		return
	}
	writeJSON(w, obs)
}

func (s *Server) handleMessage(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Message      string `json:"message"`
		Model        string `json:"model"`
		Agent        string `json:"agent"`
		ObserverName string `json:"observer_name"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, err)
		return
	}
	if strings.TrimSpace(req.Message) == "" {
		writeError(w, http.StatusBadRequest, fmt.Errorf("message field is required"))
		return
	}
	if strings.TrimSpace(req.ObserverName) == "" {
		writeError(w, http.StatusBadRequest, fmt.Errorf("observer_name is required; call POST /observers/register first"))
		return
	}
	if err := s.engine.RequireObserverRegistered(req.ObserverName); err != nil {
		writeError(w, http.StatusBadRequest, err)
		return
	}

	result, err := s.engine.SendChat(r.Context(), s.client(), engine.ChatRequest{
		Message:      req.Message,
		Model:        req.Model,
		Agent:        req.Agent,
		Driver:       "ide",
		ObserverName: req.ObserverName,
	})
	if err != nil {
		writeError(w, http.StatusBadGateway, err)
		return
	}
	writeJSON(w, result)
}

func (s *Server) handleTranscriptWS(w http.ResponseWriter, r *http.Request) {
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		return
	}
	defer conn.Close()

	events := s.engine.SubscribeTranscript()
	defer s.engine.UnsubscribeTranscript(events)

	snapshot, ok := s.engine.GetSessions().GetActiveSession()
	if ok {
		copySession := *snapshot
		_ = conn.WriteJSON(engine.TranscriptEvent{
			Type:    engine.EventTranscriptSnapshot,
			Session: &copySession,
		})
	} else {
		_ = conn.WriteJSON(engine.TranscriptEvent{
			Type: engine.EventTranscriptSnapshot,
		})
	}

	for {
		select {
		case event, ok := <-events:
			if !ok {
				return
			}
			if err := conn.WriteJSON(event); err != nil {
				return
			}
		}
	}
}

func (s *Server) handleChatWS(w http.ResponseWriter, r *http.Request) {
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		return
	}
	defer conn.Close()

	for {
		var req struct {
			Type         string `json:"type"`
			Message      string `json:"message"`
			Model        string `json:"model"`
			Agent        string `json:"agent"`
			ObserverName string `json:"observer_name"`
		}
		if err := conn.ReadJSON(&req); err != nil {
			return
		}
		if req.Type != "message" {
			continue
		}
		if strings.TrimSpace(req.ObserverName) == "" {
			_ = conn.WriteJSON(map[string]interface{}{
				"type":  "error",
				"error": "observer_name is required; register an observer first",
			})
			continue
		}
		if err := s.engine.RequireObserverRegistered(req.ObserverName); err != nil {
			_ = conn.WriteJSON(map[string]interface{}{
				"type":  "error",
				"error": err.Error(),
			})
			continue
		}

		ctx := context.Background()
		_, err := s.engine.SendChat(ctx, s.client(), engine.ChatRequest{
			Message:      req.Message,
			Model:        req.Model,
			Agent:        req.Agent,
			Driver:       "ide",
			ObserverName: req.ObserverName,
			Stream: func(chunk string) error {
				return conn.WriteJSON(map[string]interface{}{
					"type":    "chunk",
					"content": chunk,
				})
			},
		})
		if err != nil {
			_ = conn.WriteJSON(map[string]interface{}{
				"type":  "error",
				"error": err.Error(),
			})
			continue
		}
		_ = conn.WriteJSON(map[string]interface{}{"type": "done"})
	}
}

func writeJSON(w http.ResponseWriter, payload interface{}) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(payload)
}

func writeError(w http.ResponseWriter, status int, err error) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
}
