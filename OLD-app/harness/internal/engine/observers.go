package engine

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"
	"time"
)

// Observer is an external agent registered to receive transcript catch-up events.
type Observer struct {
	Name         string    `json:"name"`
	RegisteredAt time.Time `json:"registered_at"`
	LastSeenAt   time.Time `json:"last_seen_at"`
	LastAckSeq   int64     `json:"last_ack_seq"`
	PendingCount int       `json:"pending_count"`
}

// LogEntry is an append-only transcript event in the shared log.
type LogEntry struct {
	Seq            int64           `json:"seq"`
	TranscriptHash string          `json:"transcript_hash"`
	Timestamp      time.Time       `json:"timestamp"`
	Event          TranscriptEvent `json:"event"`
}

// CatchUpResult is returned to an observer requesting missed events.
type CatchUpResult struct {
	ObserverName   string     `json:"observer_name"`
	TranscriptHash string     `json:"transcript_hash"`
	LastAckSeq     int64      `json:"last_ack_seq"`
	CurrentSeq     int64      `json:"current_seq"`
	PendingCount   int        `json:"pending_count"`
	Events         []LogEntry `json:"events"`
}

// IsObserverRegistered reports whether an observer name is already registered.
func (e *Engine) IsObserverRegistered(name string) bool {
	e.observerMu.RLock()
	defer e.observerMu.RUnlock()
	_, ok := e.observers[strings.ToLower(strings.TrimSpace(name))]
	return ok
}

// RequireObserverRegistered returns an error when the observer has not registered yet.
func (e *Engine) RequireObserverRegistered(name string) error {
	name = strings.TrimSpace(name)
	if name == "" {
		return fmt.Errorf("observer_name is required")
	}
	if !e.IsObserverRegistered(name) {
		return fmt.Errorf("observer not registered: %s (call register_observer first)", name)
	}
	return nil
}

// RegisterObserver registers an external agent. Empty name auto-generates a unique random name.
func (e *Engine) RegisterObserver(name string) (*Observer, error) {
	name = strings.TrimSpace(name)
	if name == "" {
		name = e.generateUniqueParticipantName("agent")
	} else if err := ValidateParticipantName(name); err != nil {
		return nil, err
	}

	e.observerMu.Lock()
	defer e.observerMu.Unlock()

	key := strings.ToLower(name)
	if existing, ok := e.observers[key]; ok {
		existing.LastSeenAt = time.Now()
		existing.PendingCount = e.pendingCountLocked(existing.LastAckSeq)
		return existing.public(), nil
	}

	obs := &observerState{
		Name:         name,
		RegisteredAt: time.Now(),
		LastSeenAt:   time.Now(),
		LastAckSeq:   0,
	}
	e.observers[key] = obs
	obs.PendingCount = e.pendingCountLocked(obs.LastAckSeq)
	return obs.public(), nil
}

// CatchUp returns transcript events after the observer's last ack and optionally acks them.
func (e *Engine) CatchUp(name string, autoAck bool) (*CatchUpResult, error) {
	e.observerMu.Lock()
	defer e.observerMu.Unlock()

	obs, ok := e.observers[strings.ToLower(strings.TrimSpace(name))]
	if !ok {
		return nil, fmt.Errorf("observer not registered: %s", name)
	}

	obs.LastSeenAt = time.Now()
	events := e.eventsSinceLocked(obs.LastAckSeq)
	result := &CatchUpResult{
		ObserverName:   obs.Name,
		TranscriptHash: e.transcriptHashLocked(),
		LastAckSeq:     obs.LastAckSeq,
		CurrentSeq:     e.logSeq,
		PendingCount:   len(events),
		Events:         events,
	}

	if autoAck {
		obs.LastAckSeq = e.logSeq
		result.LastAckSeq = obs.LastAckSeq
		result.PendingCount = 0
	}
	obs.PendingCount = e.pendingCountLocked(obs.LastAckSeq)
	return result, nil
}

// AckObserver marks all current log entries as seen for an observer.
func (e *Engine) AckObserver(name string) (*Observer, error) {
	e.observerMu.Lock()
	defer e.observerMu.Unlock()

	obs, ok := e.observers[strings.ToLower(strings.TrimSpace(name))]
	if !ok {
		return nil, fmt.Errorf("observer not registered: %s", name)
	}

	obs.LastAckSeq = e.logSeq
	obs.LastSeenAt = time.Now()
	obs.PendingCount = 0
	return obs.public(), nil
}

// ListObservers returns registered external agents and their queue state.
func (e *Engine) ListObservers() []*Observer {
	e.observerMu.RLock()
	defer e.observerMu.RUnlock()

	out := make([]*Observer, 0, len(e.observers))
	for _, obs := range e.observers {
		copy := *obs
		copy.PendingCount = e.pendingCountLocked(copy.LastAckSeq)
		out = append(out, copy.public())
	}
	return out
}

// TranscriptHash returns a digest of the active session transcript head.
func (e *Engine) TranscriptHash() string {
	e.observerMu.RLock()
	defer e.observerMu.RUnlock()
	return e.transcriptHashLocked()
}

func (e *Engine) appendTranscriptLog(event TranscriptEvent) {
	switch event.Type {
	case EventStreamDelta, EventStreamStart, EventStreamDone, EventReasoningDelta:
		return
	}

	e.observerMu.Lock()
	defer e.observerMu.Unlock()

	e.logSeq++
	entry := LogEntry{
		Seq:            e.logSeq,
		TranscriptHash: e.transcriptHashLocked(),
		Timestamp:      time.Now(),
		Event:          event,
	}
	e.eventLog = append(e.eventLog, entry)
}

func (e *Engine) eventsSinceLocked(afterSeq int64) []LogEntry {
	out := make([]LogEntry, 0)
	for _, entry := range e.eventLog {
		if entry.Seq > afterSeq {
			out = append(out, entry)
		}
	}
	return out
}

func (e *Engine) pendingCountLocked(lastAck int64) int {
	count := 0
	for _, entry := range e.eventLog {
		if entry.Seq > lastAck {
			count++
		}
	}
	return count
}

func (e *Engine) transcriptHashLocked() string {
	session, ok := e.sessions.GetActiveSession()
	if !ok || len(session.Messages) == 0 {
		return hashParts(sessionHeadID(session), 0, "")
	}

	last := session.Messages[len(session.Messages)-1]
	return hashParts(session.ID, len(session.Messages), last.ID)
}

func sessionHeadID(session *Session) string {
	if session == nil {
		return ""
	}
	return session.ID
}

func hashParts(sessionID string, count int, lastMessageID string) string {
	sum := sha256.Sum256([]byte(fmt.Sprintf("%s:%d:%s", sessionID, count, lastMessageID)))
	return hex.EncodeToString(sum[:8])
}

type observerState struct {
	Name         string
	RegisteredAt time.Time
	LastSeenAt   time.Time
	LastAckSeq   int64
	PendingCount int
}

func (o *observerState) public() *Observer {
	return &Observer{
		Name:         o.Name,
		RegisteredAt: o.RegisteredAt,
		LastSeenAt:   o.LastSeenAt,
		LastAckSeq:   o.LastAckSeq,
		PendingCount: o.PendingCount,
	}
}

// EmitMessageAddedForTest publishes a message_added event into the shared log (tests only).
func (e *Engine) EmitMessageAddedForTest(msg Message) {
	e.emitTranscript(e.messageAddedEvent(msg))
}
