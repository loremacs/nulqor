package lmstudio

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

// Client represents an LM Studio OpenAI-compatible client
type Client struct {
	baseURL    string
	apiKey     string
	httpClient *http.Client
	model      string
}

// NewClient creates a new LM Studio client
func NewClient(baseURL, apiKey string) *Client {
	return &Client{
		baseURL: NormalizeBaseURL(baseURL),
		apiKey:  apiKey,
		httpClient: &http.Client{
			Timeout: 120 * time.Second,
		},
	}
}

// SetModel sets the model to use
func (c *Client) SetModel(model string) {
	c.model = model
}

// GetModel returns the current model
func (c *Client) GetModel() string {
	return c.model
}

// ListModels fetches available models from LM Studio
func (c *Client) ListModels(ctx context.Context) ([]string, error) {
	url := fmt.Sprintf("%s/models", c.baseURL)

	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	if c.apiKey != "" {
		req.Header.Set("Authorization", "Bearer "+c.apiKey)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to list models: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("LM Studio returned status %d", resp.StatusCode)
	}

	var result struct {
		Data []struct {
			ID string `json:"id"`
		} `json:"data"`
	}

	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}

	models := make([]string, len(result.Data))
	for i, m := range result.Data {
		models[i] = m.ID
	}

	return models, nil
}

// ChatMessage represents a chat message
type ChatMessage struct {
	Role       string     `json:"role"`
	Content           string     `json:"content"`
	ReasoningContent  string     `json:"reasoning_content,omitempty"`
	ToolCalls         []ToolCall `json:"tool_calls,omitempty"`
	ToolCallID string     `json:"tool_call_id,omitempty"`
	Name       string     `json:"name,omitempty"`
}

// ToolCall represents a model tool invocation.
type ToolCall struct {
	ID       string           `json:"id"`
	Type     string           `json:"type"`
	Function ToolCallFunction `json:"function"`
}

// ToolCallFunction holds the function payload for a tool call.
type ToolCallFunction struct {
	Name      string `json:"name"`
	Arguments string `json:"arguments"`
}

// ChatCompletionRequest represents a chat completion request
type ChatCompletionRequest struct {
	Model       string        `json:"model"`
	Messages    []ChatMessage `json:"messages"`
	Temperature float64       `json:"temperature,omitempty"`
	MaxTokens   int           `json:"max_tokens,omitempty"`
	TopP        float64       `json:"top_p,omitempty"`
	TopK        int           `json:"top_k,omitempty"`
	Stream      bool          `json:"stream"`
	Tools       []Tool        `json:"tools,omitempty"`
}

// Tool represents a tool definition
type Tool struct {
	Type     string       `json:"type"`
	Function ToolFunction `json:"function"`
}

// ToolFunction represents a tool function
type ToolFunction struct {
	Name        string                 `json:"name"`
	Description string                 `json:"description"`
	Parameters  map[string]interface{} `json:"parameters"`
}

// ChatCompletionResponse represents a chat completion response
type ChatCompletionResponse struct {
	ID      string   `json:"id"`
	Choices []Choice `json:"choices"`
	Usage   Usage    `json:"usage"`
}

// Choice represents a completion choice
type Choice struct {
	Message      ChatMessage `json:"message"`
	FinishReason string      `json:"finish_reason"`
}

// Usage represents token usage
type Usage struct {
	PromptTokens     int `json:"prompt_tokens"`
	CompletionTokens int `json:"completion_tokens"`
	TotalTokens      int `json:"total_tokens"`
}

// ChatCompletion sends a chat completion request
func (c *Client) ChatCompletion(ctx context.Context, req ChatCompletionRequest) (*ChatCompletionResponse, error) {
	if req.Model == "" && c.model != "" {
		req.Model = c.model
	}

	url := fmt.Sprintf("%s/chat/completions", c.baseURL)

	body, err := json.Marshal(req)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	httpReq.Header.Set("Content-Type", "application/json")
	if c.apiKey != "" {
		httpReq.Header.Set("Authorization", "Bearer "+c.apiKey)
	}

	resp, err := c.httpClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("failed to send request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("LM Studio returned status %d: %s", resp.StatusCode, string(body))
	}

	var result ChatCompletionResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}

	return &result, nil
}

// StreamChunk represents a streaming chunk
type StreamChunk struct {
	ID      string `json:"id"`
	Choices []struct {
		Delta struct {
			Content          string `json:"content"`
			ReasoningContent string `json:"reasoning_content"`
		} `json:"delta"`
		FinishReason *string `json:"finish_reason"`
	} `json:"choices"`
}

// ChatCompletionStream sends a streaming chat completion request
func (c *Client) ChatCompletionStream(ctx context.Context, req ChatCompletionRequest) (<-chan StreamChunk, <-chan error) {
	chunkChan := make(chan StreamChunk, 10)
	errChan := make(chan error, 1)

	if req.Model == "" && c.model != "" {
		req.Model = c.model
	}
	req.Stream = true

	go func() {
		defer close(chunkChan)
		defer close(errChan)

		url := fmt.Sprintf("%s/chat/completions", c.baseURL)

		body, err := json.Marshal(req)
		if err != nil {
			errChan <- fmt.Errorf("failed to marshal request: %w", err)
			return
		}

		httpReq, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(body))
		if err != nil {
			errChan <- fmt.Errorf("failed to create request: %w", err)
			return
		}

		httpReq.Header.Set("Content-Type", "application/json")
		httpReq.Header.Set("Accept", "text/event-stream")
		if c.apiKey != "" {
			httpReq.Header.Set("Authorization", "Bearer "+c.apiKey)
		}

		resp, err := c.httpClient.Do(httpReq)
		if err != nil {
			errChan <- fmt.Errorf("failed to send request: %w", err)
			return
		}
		defer resp.Body.Close()

		if resp.StatusCode != http.StatusOK {
			body, _ := io.ReadAll(resp.Body)
			errChan <- fmt.Errorf("LM Studio returned status %d: %s", resp.StatusCode, string(body))
			return
		}

		for {
			scanner := bufio.NewScanner(resp.Body)
			for scanner.Scan() {
				line := strings.TrimSpace(scanner.Text())
				if line == "" {
					continue
				}
				if !strings.HasPrefix(line, "data: ") {
					continue
				}
				payload := strings.TrimPrefix(line, "data: ")
				if payload == "[DONE]" {
					return
				}

				var chunk StreamChunk
				if err := json.Unmarshal([]byte(payload), &chunk); err != nil {
					continue
				}

				select {
				case chunkChan <- chunk:
				case <-ctx.Done():
					return
				}
			}
			if err := scanner.Err(); err != nil {
				if err != io.EOF {
					errChan <- fmt.Errorf("failed to read stream: %w", err)
				}
			}
			return
		}
	}()

	return chunkChan, errChan
}

// TestConnection tests the connection to LM Studio
func (c *Client) TestConnection(ctx context.Context) error {
	_, err := c.ListModels(ctx)
	return err
}
