package engine

// TranscriptEventType identifies live transcript updates.
type TranscriptEventType string

const (
	EventTranscriptSnapshot TranscriptEventType = "transcript_snapshot"
	EventMessageAdded     TranscriptEventType = "message_added"
	EventStreamStart      TranscriptEventType = "stream_start"
	EventStreamDelta      TranscriptEventType = "stream_delta"
	EventReasoningDelta   TranscriptEventType = "reasoning_delta"
	EventStreamDone       TranscriptEventType = "stream_done"
)

// TranscriptEvent is broadcast to live transcript subscribers.
type TranscriptEvent struct {
	Type        TranscriptEventType `json:"type"`
	Session     *Session            `json:"session,omitempty"`
	Message     *Message            `json:"message,omitempty"`
	StreamID    string              `json:"stream_id,omitempty"`
	Delta       string              `json:"delta,omitempty"`
	Participant string              `json:"participant,omitempty"`
}

// SubscribeTranscript registers a listener for live transcript events.
func (e *Engine) SubscribeTranscript() chan TranscriptEvent {
	e.eventMu.Lock()
	defer e.eventMu.Unlock()

	ch := make(chan TranscriptEvent, 32)
	e.eventSubs = append(e.eventSubs, ch)
	return ch
}

// UnsubscribeTranscript removes a transcript listener.
func (e *Engine) UnsubscribeTranscript(ch chan TranscriptEvent) {
	e.eventMu.Lock()
	defer e.eventMu.Unlock()

	for i, sub := range e.eventSubs {
		if sub == ch {
			e.eventSubs = append(e.eventSubs[:i], e.eventSubs[i+1:]...)
			close(ch)
			return
		}
	}
}

func (e *Engine) emitTranscript(event TranscriptEvent) {
	e.appendTranscriptLog(event)

	e.eventMu.RLock()
	subs := append([]chan TranscriptEvent(nil), e.eventSubs...)
	e.eventMu.RUnlock()

	for _, sub := range subs {
		select {
		case sub <- event:
		default:
		}
	}
}

func (e *Engine) snapshotEvent() TranscriptEvent {
	session, ok := e.sessions.GetActiveSession()
	if !ok {
		return TranscriptEvent{Type: EventTranscriptSnapshot}
	}
	copySession := *session
	return TranscriptEvent{
		Type:    EventTranscriptSnapshot,
		Session: &copySession,
	}
}

func (e *Engine) hasTranscriptSubscribers() bool {
	e.eventMu.RLock()
	defer e.eventMu.RUnlock()
	return len(e.eventSubs) > 0
}

func (e *Engine) messageAddedEvent(msg Message) TranscriptEvent {
	session, _ := e.sessions.GetActiveSession()
	var sessionCopy *Session
	if session != nil {
		copySession := *session
		sessionCopy = &copySession
	}
	msgCopy := msg
	return TranscriptEvent{
		Type:    EventMessageAdded,
		Session: sessionCopy,
		Message: &msgCopy,
	}
}
