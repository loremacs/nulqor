package engine

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
)

// Config holds the harness configuration
type Config struct {
	RootDir    string           `toml:"-"`
	ConfigPath string           `toml:"-"`
	Server     ServerConfig     `toml:"server"`
	LMStudio   LMStudioConfig   `toml:"lmstudio"`
	Paths      PathsConfig      `toml:"paths"`
	Defaults   DefaultsConfig   `toml:"defaults"`
	Generation GenerationConfig `toml:"generation"`
	Tools      ToolsConfig      `toml:"tools"`
}

type ServerConfig struct {
	Host string `toml:"host"`
	Port int    `toml:"port"`
}

type LMStudioConfig struct {
	BaseURL string `toml:"base_url"`
	APIKey  string `toml:"api_key"`
}

type PathsConfig struct {
	SkillsDir string `toml:"skills_dir"`
	AgentsDir string `toml:"agents_dir"`
	RulesDir  string `toml:"rules_dir"`
	RunsDir   string `toml:"runs_dir"`
}

type DefaultsConfig struct {
	Agent string `toml:"agent"`
	Model string `toml:"model"`
}

type GenerationConfig struct {
	Temperature float64 `toml:"temperature"`
	MaxTokens   int     `toml:"max_tokens"`
	TopP        float64 `toml:"top_p"`
	TopK        int     `toml:"top_k"`
}

type ToolsConfig struct {
	ListSkills bool `toml:"list_skills"`
	LoadSkill  bool `toml:"load_skill"`
	ReadFile   bool `toml:"read_file"`
	WriteFile  bool `toml:"write_file"`
	RunShell   bool `toml:"run_shell"`
}

// LoadConfig loads configuration from harness.toml with sensible defaults
func LoadConfig(path string) (*Config, error) {
	cfg := defaultConfig()

	absConfigPath, err := filepath.Abs(path)
	if err != nil {
		return nil, fmt.Errorf("failed to resolve config path: %w", err)
	}
	cfg.ConfigPath = absConfigPath
	cfg.RootDir = filepath.Dir(absConfigPath)

	// Try to load from file
	data, err := os.ReadFile(absConfigPath)
	if err != nil {
		if os.IsNotExist(err) {
			return finalizeConfig(cfg)
		}
		return nil, fmt.Errorf("failed to read config: %w", err)
	}

	if err := toml.Unmarshal(data, cfg); err != nil {
		return nil, fmt.Errorf("failed to parse config: %w", err)
	}

	// Apply environment variable overrides
	if v := os.Getenv("HARNESS_LMSTUDIO_URL"); v != "" {
		cfg.LMStudio.BaseURL = v
	}
	if v := os.Getenv("HARNESS_LMSTUDIO_KEY"); v != "" {
		cfg.LMStudio.APIKey = v
	}
	if v := os.Getenv("HARNESS_HOST"); v != "" {
		cfg.Server.Host = v
	}
	if v := os.Getenv("HARNESS_PORT"); v != "" {
		fmt.Sscanf(v, "%d", &cfg.Server.Port)
	}

	return finalizeConfig(cfg)
}

func defaultConfig() *Config {
	return &Config{
		Server: ServerConfig{
			Host: "localhost",
			Port: 8080,
		},
		LMStudio: LMStudioConfig{
			BaseURL: "http://localhost:1234/v1",
			APIKey:  "",
		},
		Paths: PathsConfig{
			SkillsDir: "./skills",
			AgentsDir: "./agents",
			RulesDir:  "./rules",
			RunsDir:   "./runs",
		},
		Defaults: DefaultsConfig{
			Agent: "default",
			Model: "",
		},
		Generation: GenerationConfig{
			Temperature: 0.7,
			MaxTokens:   2048,
			TopP:        0.9,
			TopK:        40,
		},
		Tools: ToolsConfig{
			ListSkills: true,
			LoadSkill:  true,
			ReadFile:   false,
			WriteFile:  false,
			RunShell:   false,
		},
	}
}

func finalizeConfig(cfg *Config) (*Config, error) {
	cfg.Paths.SkillsDir = resolvePath(cfg.RootDir, cfg.Paths.SkillsDir)
	cfg.Paths.AgentsDir = resolvePath(cfg.RootDir, cfg.Paths.AgentsDir)
	cfg.Paths.RulesDir = resolvePath(cfg.RootDir, cfg.Paths.RulesDir)
	cfg.Paths.RunsDir = resolvePath(cfg.RootDir, cfg.Paths.RunsDir)

	if err := os.MkdirAll(cfg.Paths.RunsDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create runs dir: %w", err)
	}

	return cfg, nil
}

func resolvePath(root, p string) string {
	if filepath.IsAbs(p) {
		return p
	}
	return filepath.Clean(filepath.Join(root, p))
}
