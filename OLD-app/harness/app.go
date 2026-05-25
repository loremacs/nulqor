package main

import (
	"context"
	"fmt"
	"log"
	"time"

	"github.com/wailsapp/wails/v2/pkg/runtime"

	"harness/internal/api"
	"harness/internal/engine"
	"harness/internal/lmstudio"
)

// App struct
type App struct {
	ctx          context.Context
	engine       *engine.Engine
	lmClient     *lmstudio.Client
	apiServer    *api.Server
	transcriptCh chan engine.TranscriptEvent
}

// NewApp creates a new App application struct
func NewApp(eng *engine.Engine) *App {
	cfg := eng.GetConfig()
	lmClient := lmstudio.NewClient(cfg.LMStudio.BaseURL, cfg.LMStudio.APIKey)

	app := &App{
		engine:   eng,
		lmClient: lmClient,
	}
	app.apiServer = api.NewServer(eng, lmClient)
	return app
}

// startup is called at application startup
func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	if err := a.engine.Start(ctx); err != nil {
		panic(err)
	}

	cfg := a.engine.GetConfig()
	if err := a.apiServer.Start(cfg.Server.Host, cfg.Server.Port); err != nil {
		panic(err)
	}
	log.Printf("Harness live API: http://%s:%d/transcript", cfg.Server.Host, cfg.Server.Port)
	log.Printf("Harness live feed: ws://%s:%d/ws/transcript", cfg.Server.Host, cfg.Server.Port)

	a.transcriptCh = a.engine.SubscribeTranscript()
	go a.forwardTranscriptEvents()
}

func (a *App) forwardTranscriptEvents() {
	for event := range a.transcriptCh {
		runtime.EventsEmit(a.ctx, "transcript-event", transcriptEventPayload(event))
	}
}

// shutdown is called at application termination
func (a *App) shutdown(ctx context.Context) {
	if a.transcriptCh != nil {
		a.engine.UnsubscribeTranscript(a.transcriptCh)
	}
	shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	_ = a.apiServer.Stop(shutdownCtx)
	a.engine.Stop()
}

// TestConnection tests the connection to LM Studio and returns available models
func (a *App) TestConnection(endpoint string) (map[string]interface{}, error) {
	endpoint = lmstudio.NormalizeBaseURL(endpoint)
	a.lmClient = lmstudio.NewClient(endpoint, a.engine.GetConfig().LMStudio.APIKey)

	models, err := a.lmClient.ListModels(a.ctx)
	if err != nil {
		return nil, fmt.Errorf("could not reach LM Studio at %s: %w", endpoint, err)
	}

	if len(models) > 0 {
		a.lmClient.SetModel(models[0])
	}

	return map[string]interface{}{
		"endpoint": endpoint,
		"models":   models,
	}, nil
}

// GetSettings returns harness configuration for the settings UI.
func (a *App) GetSettings() map[string]interface{} {
	cfg := a.engine.GetConfig()
	return map[string]interface{}{
		"server": map[string]interface{}{
			"host": cfg.Server.Host,
			"port": cfg.Server.Port,
		},
		"lmstudio": map[string]interface{}{
			"base_url": cfg.LMStudio.BaseURL,
		},
		"defaults": map[string]interface{}{
			"agent": cfg.Defaults.Agent,
			"model": cfg.Defaults.Model,
		},
		"generation": map[string]interface{}{
			"temperature": cfg.Generation.Temperature,
			"max_tokens":  cfg.Generation.MaxTokens,
			"top_p":       cfg.Generation.TopP,
			"top_k":       cfg.Generation.TopK,
		},
		"paths": map[string]interface{}{
			"skills_dir": cfg.Paths.SkillsDir,
			"agents_dir": cfg.Paths.AgentsDir,
			"rules_dir":  cfg.Paths.RulesDir,
			"runs_dir":   cfg.Paths.RunsDir,
		},
		"human_participant_name": a.engine.HumanParticipantName(),
	}
}

// GetSystemPrompt returns the assembled system prompt for an agent.
func (a *App) GetSystemPrompt(agent string) (string, error) {
	return a.engine.PreviewSystemPrompt(agent)
}

// GetTranscript returns the live active session transcript.
func (a *App) GetTranscript() (map[string]interface{}, error) {
	session, ok := a.engine.GetSessions().GetActiveSession()
	if !ok {
		return map[string]interface{}{"messages": []map[string]interface{}{}}, nil
	}
	return sessionPayload(session), nil
}

// ListSkills returns all available skills
func (a *App) ListSkills() []map[string]interface{} {
	skills := a.engine.GetLoaders().ListSkills()
	result := make([]map[string]interface{}, len(skills))
	for i, skill := range skills {
		result[i] = map[string]interface{}{
			"name":        skill.Name,
			"description": skill.Description,
			"path":        skill.Path,
		}
	}
	return result
}

// ListAgents returns all available agents
func (a *App) ListAgents() map[string]map[string]interface{} {
	agents := a.engine.GetLoaders().ListAgents()
	result := make(map[string]map[string]interface{})
	for name, agent := range agents {
		result[name] = map[string]interface{}{
			"description": agent.Description,
			"path":        agent.Path,
		}
	}
	return result
}

// ListRules returns all available rules
func (a *App) ListRules() []map[string]interface{} {
	rules := a.engine.GetLoaders().GetRules()
	result := make([]map[string]interface{}, len(rules))
	for i, rule := range rules {
		result[i] = map[string]interface{}{
			"name": rule.Name,
			"path": rule.Path,
		}
	}
	return result
}

// ReadFile reads a file's content
func (a *App) ReadFile(path string) (string, error) {
	data, err := a.engine.GetLoaders().ReadFile(path)
	if err != nil {
		return "", err
	}
	return string(data), nil
}

// WriteFile writes content to a file
func (a *App) WriteFile(path, content string) error {
	return a.engine.GetLoaders().WriteFile(path, []byte(content))
}

// ReloadContext reloads skills, agents, and rules from disk.
func (a *App) ReloadContext() error {
	return a.engine.GetLoaders().LoadAll()
}

// GetHumanParticipantName returns the human display name for the active session.
func (a *App) GetHumanParticipantName() string {
	return a.engine.HumanParticipantName()
}

// SetHumanParticipantName sets a custom human display name, or generates one when empty.
func (a *App) SetHumanParticipantName(name string) (string, error) {
	return a.engine.SetHumanParticipantName(name)
}

// GenerateParticipantName returns a new random participant name suggestion.
func (a *App) GenerateParticipantName() string {
	return engine.GenerateParticipantName("human")
}

// SendMessage sends a message to the model and returns the response
func (a *App) SendMessage(message, model, agent string) (map[string]interface{}, error) {
	result, err := a.engine.SendChat(a.ctx, a.lmClient, engine.ChatRequest{
		Message: message,
		Model:   model,
		Agent:   agent,
		Driver:  "human",
	})
	if err != nil {
		return nil, err
	}

	return map[string]interface{}{
		"content":       result.Content,
		"reasoning":     result.Reasoning,
		"system_prompt": result.SystemPrompt,
		"latency":       result.LatencyMs,
		"tokens":        result.Tokens,
	}, nil
}

// SendMessageStream sends a message and emits stream chunks as Wails events.
func (a *App) SendMessageStream(message, model, agent string) (map[string]interface{}, error) {
	streamID := fmt.Sprintf("stream-%d", len(message))
	result, err := a.engine.SendChat(a.ctx, a.lmClient, engine.ChatRequest{
		Message:  message,
		Model:    model,
		Agent:    agent,
		Driver:   "human",
		StreamID: streamID,
		Stream: func(chunk string) error {
			runtime.EventsEmit(a.ctx, "chat-stream", map[string]interface{}{
				"id":      streamID,
				"content": chunk,
			})
			return nil
		},
		ReasoningStream: func(chunk string) error {
			runtime.EventsEmit(a.ctx, "chat-reasoning-stream", map[string]interface{}{
				"id":      streamID,
				"content": chunk,
			})
			return nil
		},
	})
	if err != nil {
		return nil, err
	}

	runtime.EventsEmit(a.ctx, "chat-stream-done", map[string]interface{}{
		"id": streamID,
	})

	return map[string]interface{}{
		"content":       result.Content,
		"reasoning":     result.Reasoning,
		"system_prompt": result.SystemPrompt,
		"latency":       result.LatencyMs,
		"tokens":        result.Tokens,
	}, nil
}
