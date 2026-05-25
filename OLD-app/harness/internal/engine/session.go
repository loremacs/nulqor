package engine

import (
	"strings"
	"time"
)

// Message represents a single message in a conversation
type Message struct {
	ID        string            `json:"id"`
	Role      string            `json:"role"` // system, user, assistant, tool
	Content   string            `json:"content"`
	Timestamp time.Time         `json:"timestamp"`
	Metadata  MessageMetadata   `json:"metadata"`
}

type MessageMetadata struct {
	Driver            string `json:"driver"`           // participant id: human or observer name
	ParticipantName   string `json:"participant_name"` // display name in chat history
	ReasoningContent  string `json:"reasoning_content,omitempty"`
	Model             string `json:"model"`
	LatencyMs   int    `json:"latency_ms"`
	TokenCount  int    `json:"token_count"`
	ToolCallID  string `json:"tool_call_id,omitempty"`
	ToolName    string `json:"tool_name,omitempty"`
}

// Session represents a conversation session
type Session struct {
	ID                   string     `json:"id"`
	Name                 string     `json:"name"`
	Messages             []Message  `json:"messages"`
	Agent                string     `json:"agent"`
	Model                string     `json:"model"`
	HumanParticipantName string     `json:"human_participant_name"`
	CreatedAt            time.Time  `json:"created_at"`
	UpdatedAt            time.Time  `json:"updated_at"`
}

// SessionManager manages conversation sessions
type SessionManager struct {
	sessions map[string]*Session
	activeID string
}

// NewSessionManager creates a new session manager
func NewSessionManager() *SessionManager {
	return &SessionManager{
		sessions: make(map[string]*Session),
	}
}

// CreateSession creates a new session
func (sm *SessionManager) CreateSession(name string) *Session {
	id := generateID()
	session := &Session{
		ID:        id,
		Name:      name,
		Messages:  []Message{},
		Agent:     "default",
		Model:     "",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	sm.sessions[id] = session
	sm.activeID = id
	return session
}

// GetSession retrieves a session by ID
func (sm *SessionManager) GetSession(id string) (*Session, bool) {
	s, ok := sm.sessions[id]
	return s, ok
}

// GetActiveSession returns the currently active session
func (sm *SessionManager) GetActiveSession() (*Session, bool) {
	if sm.activeID == "" {
		return nil, false
	}
	return sm.GetSession(sm.activeID)
}

// SetActiveSession sets the active session
func (sm *SessionManager) SetActiveSession(id string) bool {
	if _, ok := sm.sessions[id]; !ok {
		return false
	}
	sm.activeID = id
	return true
}

// ListSessions returns all sessions
func (sm *SessionManager) ListSessions() []*Session {
	sessions := make([]*Session, 0, len(sm.sessions))
	for _, s := range sm.sessions {
		sessions = append(sessions, s)
	}
	return sessions
}

// ForkSession creates a new session from a point in an existing session
func (sm *SessionManager) ForkSession(sourceID string, atMessageIndex int, name string) (*Session, error) {
	source, ok := sm.GetSession(sourceID)
	if !ok {
		return nil, ErrSessionNotFound
	}

	if atMessageIndex < 0 || atMessageIndex > len(source.Messages) {
		return nil, ErrInvalidMessageIndex
	}

	newSession := sm.CreateSession(name)
	newSession.Agent = source.Agent
	newSession.Model = source.Model

	// Copy messages up to the fork point
	newSession.Messages = make([]Message, atMessageIndex)
	copy(newSession.Messages, source.Messages[:atMessageIndex])

	return newSession, nil
}

// AddMessage adds a message to a session
func (sm *SessionManager) AddMessage(sessionID string, msg Message) error {
	session, ok := sm.GetSession(sessionID)
	if !ok {
		return ErrSessionNotFound
	}
	session.Messages = append(session.Messages, msg)
	session.UpdatedAt = time.Now()
	return nil
}

// SetAgent sets the agent for a session
func (sm *SessionManager) SetAgent(sessionID, agent string) error {
	session, ok := sm.GetSession(sessionID)
	if !ok {
		return ErrSessionNotFound
	}
	session.Agent = agent
	session.UpdatedAt = time.Now()
	return nil
}

// SetHumanParticipantName sets or auto-generates the human display name on the active session.
func (e *Engine) SetHumanParticipantName(name string) (string, error) {
	session, ok := e.sessions.GetActiveSession()
	if !ok {
		session = e.sessions.CreateSession("Default")
	}

	name = strings.TrimSpace(name)
	if name == "" {
		session.HumanParticipantName = e.generateUniqueParticipantName("human")
		session.UpdatedAt = time.Now()
		return session.HumanParticipantName, nil
	}

	sanitized, err := SanitizeDisplayName(name)
	if err != nil {
		return "", err
	}
	session.HumanParticipantName = sanitized
	session.UpdatedAt = time.Now()
	return session.HumanParticipantName, nil
}

// HumanParticipantName returns the active session human display name, generating one if needed.
func (e *Engine) HumanParticipantName() string {
	session, ok := e.sessions.GetActiveSession()
	if !ok {
		session = e.sessions.CreateSession("Default")
	}
	if session.HumanParticipantName == "" {
		session.HumanParticipantName = e.generateUniqueParticipantName("human")
		session.UpdatedAt = time.Now()
	}
	return session.HumanParticipantName
}

// SetModel sets the model for a session
func (sm *SessionManager) SetModel(sessionID, model string) error {
	session, ok := sm.GetSession(sessionID)
	if !ok {
		return ErrSessionNotFound
	}
	session.Model = model
	session.UpdatedAt = time.Now()
	return nil
}

// Errors
var (
	ErrSessionNotFound      = &SessionError{Message: "session not found"}
	ErrInvalidMessageIndex  = &SessionError{Message: "invalid message index"}
)

type SessionError struct {
	Message string
}

func (e *SessionError) Error() string {
	return e.Message
}

func generateID() string {
	return time.Now().Format("20060102-150405") + "-" + randomString(4)
}

func randomString(n int) string {
	const letters = "abcdefghijklmnopqrstuvwxyz0123456789"
	b := make([]byte, n)
	for i := range b {
		b[i] = letters[time.Now().Nanosecond()%len(letters)]
	}
	return string(b)
}
