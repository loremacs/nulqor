package engine

import "testing"

func TestGenerateParticipantName(t *testing.T) {
	name := GenerateParticipantName("agent")
	if err := ValidateParticipantName(name); err != nil {
		t.Fatalf("expected valid generated name, got %q: %v", name, err)
	}
}

func TestRegisterObserverAutoName(t *testing.T) {
	eng := testEngine(t)
	obs, err := eng.RegisterObserver("")
	if err != nil {
		t.Fatalf("RegisterObserver: %v", err)
	}
	if obs.Name == "" {
		t.Fatal("expected generated observer name")
	}
	if err := ValidateParticipantName(obs.Name); err != nil {
		t.Fatalf("generated name invalid: %v", err)
	}
}

func TestSetHumanParticipantName(t *testing.T) {
	eng := testEngine(t)

	custom, err := eng.SetHumanParticipantName("Loren")
	if err != nil {
		t.Fatalf("SetHumanParticipantName custom: %v", err)
	}
	if custom != "Loren" {
		t.Fatalf("expected Loren, got %q", custom)
	}

	generated, err := eng.SetHumanParticipantName("")
	if err != nil {
		t.Fatalf("SetHumanParticipantName empty: %v", err)
	}
	if generated == "" {
		t.Fatal("expected generated human name")
	}
}

func TestResolveParticipantDisplayName(t *testing.T) {
	eng := testEngine(t)
	session := eng.sessions.CreateSession("NameTest")
	session.HumanParticipantName = "Loren"

	if got := eng.resolveParticipantDisplayName(ChatRequest{Driver: "human"}, session); got != "Loren" {
		t.Fatalf("expected Loren, got %q", got)
	}
	if got := eng.resolveParticipantDisplayName(ChatRequest{Driver: "ide", ObserverName: "agent-k7m2x9"}, session); got != "agent-k7m2x9" {
		t.Fatalf("expected agent-k7m2x9, got %q", got)
	}
}
