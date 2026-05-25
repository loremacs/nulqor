package engine

import "testing"

func TestFirstRegisterGetsBacklog(t *testing.T) {
	eng := testEngine(t)
	session := eng.sessions.CreateSession("Backlog")

	user := Message{ID: "u1", Role: "user", Content: "before register"}
	_ = eng.sessions.AddMessage(session.ID, user)
	eng.EmitMessageAddedForTest(user)

	assistant := Message{ID: "a1", Role: "assistant", Content: "reply before register"}
	_ = eng.sessions.AddMessage(session.ID, assistant)
	eng.EmitMessageAddedForTest(assistant)

	obs, err := eng.RegisterObserver("Cursor-backlog")
	if err != nil {
		t.Fatalf("RegisterObserver: %v", err)
	}
	if obs.LastAckSeq != 0 {
		t.Fatalf("expected new observer last_ack_seq=0, got %d", obs.LastAckSeq)
	}
	if obs.PendingCount != 2 {
		t.Fatalf("expected 2 pending on register, got %d", obs.PendingCount)
	}

	catchUp, err := eng.CatchUp(obs.Name, true)
	if err != nil {
		t.Fatalf("CatchUp: %v", err)
	}
	if len(catchUp.Events) != 2 {
		t.Fatalf("expected 2 backlog events, got %d", len(catchUp.Events))
	}
	for _, entry := range catchUp.Events {
		if entry.Event.Type != EventMessageAdded {
			t.Fatalf("expected message_added only, got %s", entry.Event.Type)
		}
	}

	catchUp, err = eng.CatchUp(obs.Name, true)
	if err != nil {
		t.Fatalf("CatchUp after ack: %v", err)
	}
	if len(catchUp.Events) != 0 {
		t.Fatalf("expected empty queue after ack, got %d", len(catchUp.Events))
	}
}

func TestCatchUpDedupesAssistantTurn(t *testing.T) {
	eng := testEngine(t)
	obs, err := eng.RegisterObserver("Dedup-test")
	if err != nil {
		t.Fatalf("RegisterObserver: %v", err)
	}

	user := Message{ID: "u1", Role: "user", Content: "hi"}
	eng.emitTranscript(eng.messageAddedEvent(user))
	eng.emitTranscript(TranscriptEvent{Type: EventStreamStart, StreamID: "s1"})
	assistant := Message{ID: "a1", Role: "assistant", Content: "hello"}
	eng.emitTranscript(eng.messageAddedEvent(assistant))
	eng.emitTranscript(TranscriptEvent{
		Type:     EventStreamDone,
		StreamID: "s1",
		Message:  &assistant,
	})

	catchUp, err := eng.CatchUp(obs.Name, false)
	if err != nil {
		t.Fatalf("CatchUp: %v", err)
	}
	if len(catchUp.Events) != 2 {
		t.Fatalf("expected 2 log events (user+assistant), got %d", len(catchUp.Events))
	}
}
