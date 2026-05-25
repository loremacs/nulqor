package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"

	"harness/internal/api"
	"harness/internal/engine"
	mcpserver "harness/internal/mcp"
	"harness/internal/lmstudio"
)

func main() {
	mode := flag.String("mode", "serve", "Run mode: serve or mcp")
	configPath := flag.String("config", "", "Path to harness.toml")
	flag.Parse()

	cfgPath := *configPath
	if cfgPath == "" {
		cfgPath = findConfigPath()
	}

	cfg, err := engine.LoadConfig(cfgPath)
	if err != nil {
		log.Fatalf("Failed to load config: %v", err)
	}

	eng := engine.NewEngine(cfg)

	switch *mode {
	case "serve":
		runServe(eng, cfg)
	case "mcp":
		runMCP(eng, cfg)
	default:
		log.Fatalf("Unknown mode: %s", *mode)
	}
}

func findConfigPath() string {
	candidates := []string{
		"harness.toml",
		filepath.Join("..", "harness.toml"),
		filepath.Join("harness", "harness.toml"),
	}
	for _, candidate := range candidates {
		if _, err := os.Stat(candidate); err == nil {
			abs, err := filepath.Abs(candidate)
			if err == nil {
				return abs
			}
			return candidate
		}
	}
	return "harness.toml"
}

func runServe(eng *engine.Engine, cfg *engine.Config) {
	ctx := context.Background()
	if err := eng.Start(ctx); err != nil {
		log.Fatal("Failed to start engine:", err)
	}
	defer eng.Stop()

	client := lmstudio.NewClient(cfg.LMStudio.BaseURL, cfg.LMStudio.APIKey)
	server := api.NewServer(eng, client)
	if err := server.Start(cfg.Server.Host, cfg.Server.Port); err != nil {
		log.Fatal(err)
	}

	log.Printf("Harness API listening on http://%s:%d", cfg.Server.Host, cfg.Server.Port)
	log.Printf("Live transcript feed: ws://%s:%d/ws/transcript", cfg.Server.Host, cfg.Server.Port)
	select {}
}

func runMCP(eng *engine.Engine, cfg *engine.Config) {
	_ = eng
	apiURL := os.Getenv("HARNESS_API_URL")
	if apiURL == "" {
		apiURL = fmt.Sprintf("http://%s:%d", cfg.Server.Host, cfg.Server.Port)
	}

	log.Printf("MCP stdio proxy -> %s", apiURL)
	if err := mcpserver.RunStdio(apiURL); err != nil {
		log.Fatal(err)
	}
}
