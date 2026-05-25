package mcp

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// RunStdio serves MCP tools over stdio, proxying to a running harness HTTP API.
func RunStdio(apiURL string) error {
	remote := NewRemoteHarness(apiURL)

	server := mcp.NewServer(&mcp.Implementation{
		Name:    "harness",
		Version: "1.0.0",
	}, nil)

	mcp.AddTool(server, &mcp.Tool{
		Name:        "register_observer",
		Description: "Register this MCP client as an external agent. Provide a custom name or omit name for a unique auto-generated name. Required once per agent session before catch_up or send_message.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, args registerObserverArgs) (*mcp.CallToolResult, any, error) {
		result, err := remote.RegisterObserver(args.Name)
		return toolResult(result, err)
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "catch_up",
		Description: "Fetch transcript events missed since your last ack for this observer. Set auto_ack=true to flush the queue after reading.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, args catchUpArgs) (*mcp.CallToolResult, any, error) {
		result, err := remote.CatchUp(args.ObserverName, args.AutoAck)
		return toolResult(result, err)
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "ack_observer",
		Description: "Mark all current transcript events as seen for an observer without returning them.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, args ackObserverArgs) (*mcp.CallToolResult, any, error) {
		result, err := remote.AckObserver(args.ObserverName)
		return toolResult(result, err)
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "send_message",
		Description: "Send a message to the harness local model as a registered observer. Requires observer_name from register_observer.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, args sendMessageArgs) (*mcp.CallToolResult, any, error) {
		if strings.TrimSpace(args.ObserverName) == "" {
			return toolResult(nil, fmt.Errorf("observer_name is required; call register_observer first with a unique name"))
		}
		result, err := remote.SendMessage(args.ObserverName, args.Message, args.Model, args.Agent)
		return toolResult(result, err)
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "list_observers",
		Description: "List registered external agents and pending catch-up counts.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, _ struct{}) (*mcp.CallToolResult, any, error) {
		result, err := remote.ListObservers()
		return toolResult(result, err)
	})

	return server.Run(context.Background(), &mcp.StdioTransport{})
}

type registerObserverArgs struct {
	Name string `json:"name" jsonschema:"Optional custom agent name (3-32 chars). Omit or leave empty for a unique auto-generated name such as agent-k7m2x9."`
}

type catchUpArgs struct {
	ObserverName string `json:"observer_name" jsonschema:"Registered observer name"`
	AutoAck      bool   `json:"auto_ack" jsonschema:"When true, flush the queue after returning events"`
}

type ackObserverArgs struct {
	ObserverName string `json:"observer_name" jsonschema:"Registered observer name"`
}

type sendMessageArgs struct {
	ObserverName string `json:"observer_name" jsonschema:"Registered observer name"`
	Message      string `json:"message" jsonschema:"User message to send to the local model"`
	Model        string `json:"model" jsonschema:"Optional LM Studio model id"`
	Agent        string `json:"agent" jsonschema:"Optional harness agent persona name"`
}

func toolResult(payload map[string]interface{}, err error) (*mcp.CallToolResult, any, error) {
	if err != nil {
		return &mcp.CallToolResult{
			IsError: true,
			Content: []mcp.Content{&mcp.TextContent{Text: err.Error()}},
		}, nil, nil
	}
	text, _ := json.MarshalIndent(payload, "", "  ")
	return &mcp.CallToolResult{
		Content: []mcp.Content{&mcp.TextContent{Text: string(text)}},
	}, payload, nil
}
