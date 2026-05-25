package engine

import (
	"context"
	"log"
	"sync"
)

// Engine is the core engine that ties all components together
type Engine struct {
	cfg      *Config
	loaders  *Loaders
	watcher  *Watcher
	sessions *SessionManager
	prompt   *PromptAssembler

	mu        sync.RWMutex
	eventMu   sync.RWMutex
	eventSubs []chan TranscriptEvent

	observerMu sync.RWMutex
	observers  map[string]*observerState
	eventLog   []LogEntry
	logSeq     int64
}

// NewEngine creates a new engine instance
func NewEngine(cfg *Config) *Engine {
	loaders := NewLoaders(cfg)
	sessions := NewSessionManager()
	prompt := NewPromptAssembler(loaders)

	engine := &Engine{
		cfg:       cfg,
		loaders:   loaders,
		sessions:  sessions,
		prompt:    prompt,
		observers: make(map[string]*observerState),
	}

	// Set up reload callbacks
	loaders.SetReloadCallbacks(
		func(skills []*Skill) {
			log.Printf("Skills reloaded: %d skills", len(skills))
		},
		func(agents map[string]*Agent) {
			log.Printf("Agents reloaded: %d agents", len(agents))
		},
		func(rules []*Rule) {
			log.Printf("Rules reloaded: %d rules", len(rules))
		},
	)

	return engine
}

// Start initializes and starts the engine
func (e *Engine) Start(ctx context.Context) error {
	// Load all context
	if err := e.loaders.LoadAll(); err != nil {
		return err
	}

	// Start file watcher
	watcher, err := NewWatcher(e.cfg, e.loaders)
	if err != nil {
		log.Printf("Warning: failed to start file watcher: %v", err)
	} else {
		e.watcher = watcher
		if err := watcher.Start(); err != nil {
			log.Printf("Warning: failed to start watching: %v", err)
		}
	}

	// Create default session if none exists
	if _, ok := e.sessions.GetActiveSession(); !ok {
		e.sessions.CreateSession("Default")
	}
	_ = e.HumanParticipantName()

	return nil
}

// Stop shuts down the engine
func (e *Engine) Stop() {
	if e.watcher != nil {
		e.watcher.Stop()
	}
}

// GetConfig returns the engine configuration
func (e *Engine) GetConfig() *Config {
	return e.cfg
}

// GetLoaders returns the loaders
func (e *Engine) GetLoaders() *Loaders {
	return e.loaders
}

// GetSessions returns the session manager
func (e *Engine) GetSessions() *SessionManager {
	return e.sessions
}

// GetPrompt returns the prompt assembler
func (e *Engine) GetPrompt() *PromptAssembler {
	return e.prompt
}
