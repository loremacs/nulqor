package engine

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"harness/internal/lmstudio"
)

// ChatRequest captures a single turn request.
type ChatRequest struct {
	Message         string
	Model           string
	Agent           string
	Driver          string
	ObserverName    string
	ParticipantName string
	StreamID        string
	Stream          func(chunk string) error
	ReasoningStream func(chunk string) error
}

// ChatResult is returned after a completed turn.
type ChatResult struct {
	Content          string `json:"content"`
	Reasoning        string `json:"reasoning,omitempty"`
	SystemPrompt     string `json:"system_prompt"`
	LatencyMs    int64  `json:"latency_ms"`
	Tokens       int    `json:"tokens"`
}

// SendChat runs one user turn through the active session, including tool loops.
func (e *Engine) SendChat(ctx context.Context, client *lmstudio.Client, req ChatRequest) (*ChatResult, error) {
	req.Message = strings.TrimSpace(req.Message)
	if req.Message == "" {
		return nil, fmt.Errorf("message cannot be empty")
	}
	if req.Driver == "ide" {
		if strings.TrimSpace(req.ObserverName) == "" {
			return nil, fmt.Errorf("observer_name is required for external agents; call register_observer first")
		}
		if err := e.RequireObserverRegistered(req.ObserverName); err != nil {
			return nil, err
		}
	}

	session, ok := e.sessions.GetActiveSession()
	if !ok {
		session = e.sessions.CreateSession("Default")
	}

	participantDisplay := e.resolveParticipantDisplayName(req, session)
	driverID := reqDriver(req.Driver, req.ObserverName)

	agentName := req.Agent
	if agentName == "" {
		agentName = session.Agent
	}
	if agentName == "" {
		agentName = e.cfg.Defaults.Agent
	}

	model := req.Model
	if model == "" {
		model = session.Model
	}
	if model == "" {
		model = client.GetModel()
	}
	if model == "" {
		models, err := client.ListModels(ctx)
		if err != nil {
			return nil, fmt.Errorf("no model selected and autodetect failed: %w", err)
		}
		if len(models) == 0 {
			return nil, fmt.Errorf("no model loaded in LM Studio")
		}
		model = models[0]
	}

	client.SetModel(model)
	_ = e.sessions.SetAgent(session.ID, agentName)
	_ = e.sessions.SetModel(session.ID, model)

	registry := NewToolRegistry(e.cfg, e.loaders)
	systemPrompt, err := e.prompt.AssembleSystemPrompt(agentName)
	if err != nil {
		return nil, err
	}

	userMsg := Message{
		ID:        generateID(),
		Role:      "user",
		Content:   req.Message,
		Timestamp: time.Now(),
		Metadata:  buildMessageMetadata(driverID, participantDisplay, model, 0, 0),
	}
	if err := e.sessions.AddMessage(session.ID, userMsg); err != nil {
		return nil, err
	}
	e.emitTranscript(e.messageAddedEvent(userMsg))

	streamID := req.StreamID
	if streamID == "" {
		streamID = generateID()
	}
	emitStreams := req.Stream != nil || req.ReasoningStream != nil || e.hasTranscriptSubscribers()
	var streamContent, streamReasoning func(string) error
	if emitStreams {
		e.emitTranscript(TranscriptEvent{
			Type:        EventStreamStart,
			StreamID:    streamID,
			Session:     e.snapshotEvent().Session,
			Participant: participantDisplay,
		})
		streamContent = func(chunk string) error {
			if chunk == "" {
				return nil
			}
			e.emitTranscript(TranscriptEvent{
				Type:     EventStreamDelta,
				StreamID: streamID,
				Delta:    chunk,
			})
			if req.Stream != nil {
				return req.Stream(chunk)
			}
			return nil
		}
		streamReasoning = func(chunk string) error {
			if chunk == "" {
				return nil
			}
			e.emitTranscript(TranscriptEvent{
				Type:     EventReasoningDelta,
				StreamID: streamID,
				Delta:    chunk,
			})
			if req.ReasoningStream != nil {
				return req.ReasoningStream(chunk)
			}
			return nil
		}
	}

	messages := e.buildMessages(session, systemPrompt, registry)
	toolDefs := registry.Definitions()
	start := time.Now()

	var finalContent string
	var finalReasoning string
	var totalTokens int

	for step := 0; step < maxToolLoopSteps; step++ {
		lmReq := lmstudio.ChatCompletionRequest{
			Messages:    messages,
			Temperature: e.cfg.Generation.Temperature,
			MaxTokens:   e.cfg.Generation.MaxTokens,
			TopP:        e.cfg.Generation.TopP,
			TopK:        e.cfg.Generation.TopK,
			Tools:       toLMTools(toolDefs),
		}

		if streamContent != nil && len(toolDefs) == 0 {
			content, reasoning, tokens, err := e.streamCompletion(ctx, client, lmReq, streamReasoning, streamContent)
			if err != nil {
				return nil, err
			}
			finalContent = content
			finalReasoning = reasoning
			totalTokens = tokens
			break
		}

		resp, err := client.ChatCompletion(ctx, lmReq)
		if err != nil {
			return nil, err
		}
		if len(resp.Choices) == 0 {
			return nil, fmt.Errorf("no response from model")
		}

		choice := resp.Choices[0]
		totalTokens = resp.Usage.TotalTokens

		if len(choice.Message.ToolCalls) > 0 {
			messages = append(messages, choice.Message)
			for _, call := range choice.Message.ToolCalls {
				result, execErr := registry.Execute(call.Function.Name, call.Function.Arguments)
				if execErr != nil {
					result = execErr.Error()
				}
				if call.Function.Name == "load_skill" {
					systemPrompt = e.promptAssembledWithLoaded(agentName, registry)
					if len(messages) > 0 && messages[0].Role == "system" {
						messages[0].Content = systemPrompt
					}
				}
				messages = append(messages, lmstudio.ChatMessage{
					Role:       "tool",
					Content:    result,
					ToolCallID: call.ID,
					Name:       call.Function.Name,
				})
			}
			continue
		}

		finalContent = choice.Message.Content
		finalReasoning = strings.TrimSpace(choice.Message.ReasoningContent)
		if streamReasoning != nil && finalReasoning != "" {
			if err := streamReasoning(finalReasoning); err != nil {
				return nil, err
			}
		}
		if streamContent != nil && finalContent != "" {
			if err := streamContent(finalContent); err != nil {
				return nil, err
			}
		}
		break
	}

	if finalContent == "" {
		return nil, fmt.Errorf("model returned an empty response")
	}

	latency := time.Since(start).Milliseconds()
	assistantMeta := buildMessageMetadata(driverID, participantDisplay, model, int(latency), totalTokens)
	assistantMeta.ReasoningContent = finalReasoning
	assistantMsg := Message{
		ID:        generateID(),
		Role:      "assistant",
		Content:   finalContent,
		Timestamp: time.Now(),
		Metadata:  assistantMeta,
	}
	if err := e.sessions.AddMessage(session.ID, assistantMsg); err != nil {
		return nil, err
	}
	e.emitTranscript(e.messageAddedEvent(assistantMsg))
	e.emitTranscript(TranscriptEvent{
		Type:     EventStreamDone,
		StreamID: streamID,
		Session:  e.snapshotEvent().Session,
		Message:  &assistantMsg,
	})

	result := &ChatResult{
		Content:      finalContent,
		Reasoning:    finalReasoning,
		SystemPrompt: systemPrompt,
		LatencyMs:    latency,
		Tokens:       totalTokens,
	}
	_ = e.logRun(result, req, agentName, model)
	return result, nil
}

func (e *Engine) buildMessages(session *Session, systemPrompt string, registry *ToolRegistry) []lmstudio.ChatMessage {
	messages := []lmstudio.ChatMessage{{Role: "system", Content: systemPrompt}}
	for _, msg := range session.Messages {
		if msg.Role == "system" {
			continue
		}
		messages = append(messages, lmstudio.ChatMessage{
			Role:    msg.Role,
			Content: msg.Content,
		})
	}
	return messages
}

func (e *Engine) promptAssembledWithLoaded(agentName string, registry *ToolRegistry) string {
	prompt, err := e.prompt.AssembleSystemPrompt(agentName)
	if err != nil {
		return ""
	}
	var b strings.Builder
	b.WriteString(prompt)
	for _, name := range registry.LoadedSkills() {
		skill, ok := e.loaders.GetSkill(name)
		if !ok {
			continue
		}
		b.WriteString(fmt.Sprintf("\n# Loaded Skill: %s\n\n", skill.Name))
		b.WriteString(skill.Body)
		b.WriteString("\n")
	}
	return b.String()
}

func (e *Engine) streamCompletion(ctx context.Context, client *lmstudio.Client, req lmstudio.ChatCompletionRequest, emitReasoning, emitContent func(string) error) (string, string, int, error) {
	chunks, errCh := client.ChatCompletionStream(ctx, req)
	var content strings.Builder
	var reasoning strings.Builder
	for chunk := range chunks {
		if len(chunk.Choices) == 0 {
			continue
		}
		delta := chunk.Choices[0].Delta
		if r := delta.ReasoningContent; r != "" {
			reasoning.WriteString(r)
			if emitReasoning != nil {
				if err := emitReasoning(r); err != nil {
					return "", "", 0, err
				}
			}
		}
		if c := delta.Content; c != "" {
			content.WriteString(c)
			if emitContent != nil {
				if err := emitContent(c); err != nil {
					return "", "", 0, err
				}
			}
		}
	}
	if err := <-errCh; err != nil {
		return "", "", 0, err
	}
	return content.String(), reasoning.String(), 0, nil
}

func (e *Engine) logRun(result *ChatResult, req ChatRequest, agentName, model string) error {
	path := filepath.Join(e.cfg.Paths.RunsDir, time.Now().Format("2006-01-02")+".jsonl")
	entry := map[string]interface{}{
		"timestamp":     time.Now().Format(time.RFC3339),
		"driver":        reqDriver(req.Driver, req.ObserverName),
		"agent":         agentName,
		"model":         model,
		"user_message":  req.Message,
		"system_prompt": result.SystemPrompt,
		"reply":         result.Content,
		"reasoning":     result.Reasoning,
		"latency_ms":    result.LatencyMs,
		"tokens":        result.Tokens,
	}
	data, err := json.Marshal(entry)
	if err != nil {
		return err
	}
	f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return err
	}
	defer f.Close()
	_, err = f.Write(append(data, '\n'))
	return err
}

func toLMTools(defs []ToolDefinition) []lmstudio.Tool {
	if len(defs) == 0 {
		return nil
	}
	out := make([]lmstudio.Tool, len(defs))
	for i, def := range defs {
		out[i] = lmstudio.Tool{
			Type: "function",
			Function: lmstudio.ToolFunction{
				Name:        def.Name,
				Description: def.Description,
				Parameters:  def.Parameters,
			},
		}
	}
	return out
}

func reqDriver(driver, observerName string) string {
	if observerName != "" {
		return observerName
	}
	if driver == "" {
		return "human"
	}
	return driver
}

func (e *Engine) resolveParticipantDisplayName(req ChatRequest, session *Session) string {
	if req.ObserverName != "" {
		return req.ObserverName
	}
	if name := strings.TrimSpace(req.ParticipantName); name != "" {
		if sanitized, err := SanitizeDisplayName(name); err == nil {
			return sanitized
		}
	}
	if session != nil && session.HumanParticipantName != "" {
		return session.HumanParticipantName
	}
	if req.Driver == "human" || req.Driver == "" {
		return e.HumanParticipantName()
	}
	return "Agent"
}

func buildMessageMetadata(driverID, displayName, model string, latencyMs, tokenCount int) MessageMetadata {
	return MessageMetadata{
		Driver:          driverID,
		ParticipantName: displayName,
		Model:           model,
		LatencyMs:       latencyMs,
		TokenCount:      tokenCount,
	}
}

// PreviewSystemPrompt returns the assembled system prompt for an agent.
func (e *Engine) PreviewSystemPrompt(agentName string) (string, error) {
	if agentName == "" {
		agentName = e.cfg.Defaults.Agent
	}
	return e.prompt.AssembleSystemPrompt(agentName)
}
