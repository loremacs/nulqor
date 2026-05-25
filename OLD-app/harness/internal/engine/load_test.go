package engine

import (
	"path/filepath"
	"testing"
)

func TestLoadAgentsWithIndexFile(t *testing.T) {
	cfgPath := filepath.Join("..", "..", "harness.toml")
	cfg, err := LoadConfig(cfgPath)
	if err != nil {
		t.Fatalf("LoadConfig: %v", err)
	}

	loaders := NewLoaders(cfg)
	if err := loaders.LoadAgents(); err != nil {
		t.Fatalf("LoadAgents: %v", err)
	}

	if _, ok := loaders.GetAgent("default"); !ok {
		t.Errorf("expected default agent from AGENTS.md, got agents: %v", agentNames(loaders))
	}
}

func TestLoadSkillsYAMLFrontmatter(t *testing.T) {
	cfgPath := filepath.Join("..", "..", "harness.toml")
	cfg, err := LoadConfig(cfgPath)
	if err != nil {
		t.Fatalf("LoadConfig: %v", err)
	}

	loaders := NewLoaders(cfg)
	if err := loaders.LoadSkills(); err != nil {
		t.Fatalf("LoadSkills: %v", err)
	}

	skill, ok := loaders.GetSkill("build-harness")
	if !ok {
		t.Fatalf("expected build-harness skill, got: %v", agentNamesFromSkills(loaders))
	}
	if skill.Description == "" {
		t.Fatalf("expected skill description from YAML frontmatter")
	}
}

func agentNames(l *Loaders) []string {
	names := make([]string, 0, len(l.agents))
	for name := range l.agents {
		names = append(names, name)
	}
	return names
}

func agentNamesFromSkills(l *Loaders) []string {
	names := make([]string, 0, len(l.skills))
	for name := range l.skills {
		names = append(names, name)
	}
	return names
}
