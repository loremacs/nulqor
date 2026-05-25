package engine

import "testing"

func TestMultipleObserversIndependentQueues(t *testing.T) {
	eng := testEngine(t)

	a, err := eng.RegisterObserver("Cursor-multi")
	if err != nil {
		t.Fatalf("register A: %v", err)
	}
	b, err := eng.RegisterObserver("Windsurf-multi")
	if err != nil {
		t.Fatalf("register B: %v", err)
	}

	session := eng.sessions.CreateSession("Multi")
	msg := Message{ID: "m-multi", Role: "user", Content: "shared event"}
	_ = eng.sessions.AddMessage(session.ID, msg)
	eng.EmitMessageAddedForTest(msg)

	catchA, err := eng.CatchUp(a.Name, true)
	if err != nil {
		t.Fatalf("catch up A: %v", err)
	}
	if len(catchA.Events) != 1 {
		t.Fatalf("A expected 1 event, got %d", len(catchA.Events))
	}

	catchB, err := eng.CatchUp(b.Name, false)
	if err != nil {
		t.Fatalf("catch up B: %v", err)
	}
	if len(catchB.Events) != 1 {
		t.Fatalf("B expected 1 pending event, got %d", len(catchB.Events))
	}

	catchA2, err := eng.CatchUp(a.Name, false)
	if err != nil {
		t.Fatalf("catch up A again: %v", err)
	}
	if len(catchA2.Events) != 0 {
		t.Fatalf("A queue should be flushed, got %d", len(catchA2.Events))
	}
}

func TestTranscriptHashChangesWithMessages(t *testing.T) {
	eng := testEngine(t)
	session := eng.sessions.CreateSession("Hash")

	before := eng.TranscriptHash()
	msg := Message{ID: "hash-1", Role: "user", Content: "hash test"}
	_ = eng.sessions.AddMessage(session.ID, msg)
	eng.EmitMessageAddedForTest(msg)
	after := eng.TranscriptHash()

	if before == after {
		t.Fatalf("expected transcript hash to change after message")
	}
}
