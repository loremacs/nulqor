package main

import (
	"time"

	"harness/internal/engine"
)

func transcriptEventPayload(event engine.TranscriptEvent) map[string]interface{} {
	payload := map[string]interface{}{
		"type": string(event.Type),
	}
	if event.StreamID != "" {
		payload["stream_id"] = event.StreamID
	}
	if event.Delta != "" {
		payload["delta"] = event.Delta
	}
	if event.Participant != "" {
		payload["participant"] = event.Participant
	}
	if event.Session != nil {
		payload["session"] = sessionPayload(event.Session)
	}
	if event.Message != nil {
		payload["message"] = messagePayload(*event.Message)
	}
	return payload
}

func sessionPayload(session *engine.Session) map[string]interface{} {
	messages := make([]map[string]interface{}, len(session.Messages))
	for i, msg := range session.Messages {
		messages[i] = messagePayload(msg)
	}
	return map[string]interface{}{
		"id":                     session.ID,
		"name":                   session.Name,
		"messages":               messages,
		"agent":                  session.Agent,
		"model":                  session.Model,
		"human_participant_name": session.HumanParticipantName,
		"created_at":             session.CreatedAt.Format(time.RFC3339),
		"updated_at":             session.UpdatedAt.Format(time.RFC3339),
	}
}

func messagePayload(msg engine.Message) map[string]interface{} {
	return map[string]interface{}{
		"id":        msg.ID,
		"role":      msg.Role,
		"content":   msg.Content,
		"timestamp": msg.Timestamp.Format(time.RFC3339),
		"metadata": map[string]interface{}{
			"driver":            msg.Metadata.Driver,
			"participant_name":  msg.Metadata.ParticipantName,
			"reasoning_content": msg.Metadata.ReasoningContent,
			"model":             msg.Metadata.Model,
			"latency_ms":       msg.Metadata.LatencyMs,
			"token_count":      msg.Metadata.TokenCount,
		},
	}
}
