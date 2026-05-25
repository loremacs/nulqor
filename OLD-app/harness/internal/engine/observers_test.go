package engine

import (
	"testing"
)

func TestRegisterObserverUniqueName(t *testing.T) {
	eng := testEngine(t)

	first, err := eng.RegisterObserver("Cursor-Agent-1")
	if err != nil {
		t.Fatalf("RegisterObserver: %v", err)
	}
	if first.Name != "Cursor-Agent-1" {
		t.Fatalf("unexpected name: %s", first.Name)
	}

	second, err := eng.RegisterObserver("cursor-agent-1")
	if err != nil {
		t.Fatalf("re-register: %v", err)
	}
	if second.LastAckSeq != first.LastAckSeq {
		t.Fatalf("expected reconnect to preserve ack cursor")
	}

	if _, err := eng.RegisterObserver("!bad"); err == nil {
		t.Fatalf("expected invalid name error")
	}

	auto, err := eng.RegisterObserver("")
	if err != nil {
		t.Fatalf("auto RegisterObserver: %v", err)
	}
	if auto.Name == "" {
		t.Fatal("expected auto-generated observer name")
	}
}

func TestCatchUpAndAck(t *testing.T) {
	eng := testEngine(t)

	obs, err := eng.RegisterObserver("Windsurf-2")
	if err != nil {
		t.Fatalf("RegisterObserver: %v", err)
	}

	session := eng.sessions.CreateSession("Test")
	user := Message{ID: "u1", Role: "user", Content: "hello"}
	_ = eng.sessions.AddMessage(session.ID, user)
	eng.emitTranscript(eng.messageAddedEvent(user))

	catchUp, err := eng.CatchUp(obs.Name, false)
	if err != nil {
		t.Fatalf("CatchUp: %v", err)
	}
	if len(catchUp.Events) != 1 {
		t.Fatalf("expected 1 pending event, got %d", len(catchUp.Events))
	}

	if _, err := eng.AckObserver(obs.Name); err != nil {
		t.Fatalf("AckObserver: %v", err)
	}

	catchUp, err = eng.CatchUp(obs.Name, true)
	if err != nil {
		t.Fatalf("CatchUp after ack: %v", err)
	}
	if len(catchUp.Events) != 0 {
		t.Fatalf("expected flushed queue, got %d", len(catchUp.Events))
	}
}

func testEngine(t *testing.T) *Engine {
	t.Helper()
	cfg, err := LoadConfig("../../harness.toml")
	if err != nil {
		t.Fatalf("LoadConfig: %v", err)
	}
	return NewEngine(cfg)
}
