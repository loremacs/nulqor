package engine

import (
	"testing"

	"harness/internal/lmstudio"
)

func TestSendChatRequiresRegisteredObserver(t *testing.T) {
	eng := testEngine(t)
	client := lmstudio.NewClient("http://localhost:1234/v1", "")

	_, err := eng.SendChat(t.Context(), client, ChatRequest{
		Message:      "hello",
		Driver:       "ide",
		ObserverName: "agent-unregistered",
	})
	if err == nil {
		t.Fatal("expected error for unregistered observer")
	}

	_, err = eng.RegisterObserver("agent-registered")
	if err != nil {
		t.Fatalf("RegisterObserver: %v", err)
	}
}
