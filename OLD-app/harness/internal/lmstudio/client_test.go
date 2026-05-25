package lmstudio

import (
	"encoding/json"
	"testing"
)

func TestChatMessageAlwaysIncludesContent(t *testing.T) {
	msg := ChatMessage{Role: "user"}
	data, err := json.Marshal(msg)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var decoded map[string]interface{}
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if _, ok := decoded["content"]; !ok {
		t.Fatalf("expected content field in JSON, got %s", string(data))
	}
}
