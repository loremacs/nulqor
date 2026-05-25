package engine

import (
	"encoding/json"
	"fmt"
	"strings"
)

const maxToolLoopSteps = 8

// ToolDefinition describes a built-in harness tool.
type ToolDefinition struct {
	Name        string
	Description string
	Parameters  map[string]interface{}
}

// ToolRegistry exposes built-in harness tools backed by loaders.
type ToolRegistry struct {
	cfg     *Config
	loaders *Loaders
	loaded  map[string]bool
}

func NewToolRegistry(cfg *Config, loaders *Loaders) *ToolRegistry {
	return &ToolRegistry{
		cfg:     cfg,
		loaders: loaders,
		loaded:  make(map[string]bool),
	}
}

func (r *ToolRegistry) LoadedSkills() []string {
	names := make([]string, 0, len(r.loaded))
	for name := range r.loaded {
		names = append(names, name)
	}
	return names
}

func (r *ToolRegistry) Definitions() []ToolDefinition {
	defs := make([]ToolDefinition, 0, 2)
	if r.cfg.Tools.ListSkills {
		defs = append(defs, listSkillsTool())
	}
	if r.cfg.Tools.LoadSkill {
		defs = append(defs, loadSkillTool())
	}
	return defs
}

func (r *ToolRegistry) Execute(name, argsJSON string) (string, error) {
	switch name {
	case "list_skills":
		return r.listSkills(), nil
	case "load_skill":
		var args struct {
			Name string `json:"name"`
		}
		if err := json.Unmarshal([]byte(argsJSON), &args); err != nil {
			return "", fmt.Errorf("invalid arguments: %w", err)
		}
		if args.Name == "" {
			return "", fmt.Errorf("skill name is required")
		}
		skill, ok := r.loaders.GetSkill(args.Name)
		if !ok {
			return "", fmt.Errorf("skill not found: %s", args.Name)
		}
		r.loaded[args.Name] = true
		return skill.Body, nil
	default:
		return "", fmt.Errorf("unknown tool: %s", name)
	}
}

func (r *ToolRegistry) listSkills() string {
	skills := r.loaders.ListSkills()
	if len(skills) == 0 {
		return "No skills available."
	}

	var b strings.Builder
	for _, skill := range skills {
		b.WriteString(fmt.Sprintf("- %s: %s\n", skill.Name, skill.Description))
	}
	return b.String()
}

func listSkillsTool() ToolDefinition {
	return ToolDefinition{
		Name:        "list_skills",
		Description: "List all available skills with their descriptions.",
		Parameters: map[string]interface{}{
			"type":       "object",
			"properties": map[string]interface{}{},
		},
	}
}

func loadSkillTool() ToolDefinition {
	return ToolDefinition{
		Name:        "load_skill",
		Description: "Load the full instructions for a skill by name.",
		Parameters: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"name": map[string]interface{}{
					"type":        "string",
					"description": "The skill name to load.",
				},
			},
			"required": []string{"name"},
		},
	}
}
