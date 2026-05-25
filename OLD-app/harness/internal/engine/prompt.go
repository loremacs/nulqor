package engine

import (
	"fmt"
	"strings"
)

// PromptAssembler builds system prompts from agent, rules, and skills
type PromptAssembler struct {
	loaders *Loaders
}

// NewPromptAssembler creates a new prompt assembler
func NewPromptAssembler(loaders *Loaders) *PromptAssembler {
	return &PromptAssembler{
		loaders: loaders,
	}
}

// AssembleSystemPrompt builds a system prompt for the given agent
func (p *PromptAssembler) AssembleSystemPrompt(agentName string) (string, error) {
	var builder strings.Builder

	// 1. Agent persona
	agent, ok := p.loaders.GetAgent(agentName)
	if !ok {
		return "", fmt.Errorf("agent not found: %s", agentName)
	}
	builder.WriteString(agent.Body)
	builder.WriteString("\n\n")

	// 2. Rules
	rules := p.loaders.GetRules()
	if len(rules) > 0 {
		builder.WriteString("# Rules\n\n")
		for _, rule := range rules {
			builder.WriteString(rule.Body)
			builder.WriteString("\n\n")
		}
	}

	// 3. Skills index (compact, with descriptions)
	skills := p.loaders.ListSkills()
	if len(skills) > 0 {
		builder.WriteString("# Available Skills\n\n")
		builder.WriteString("You have access to the following skills. Use the `load_skill` tool to load a skill's full instructions when needed.\n\n")
		
		for _, skill := range skills {
			builder.WriteString(fmt.Sprintf("- **%s**: %s\n", skill.Name, skill.Description))
		}
		
		builder.WriteString("\n")
	}

	return builder.String(), nil
}

// AssembleSystemPromptWithSkill injects a specific skill's body into the system prompt
func (p *PromptAssembler) AssembleSystemPromptWithSkill(agentName, skillName string) (string, error) {
	basePrompt, err := p.AssembleSystemPrompt(agentName)
	if err != nil {
		return "", err
	}

	skill, ok := p.loaders.GetSkill(skillName)
	if !ok {
		return "", fmt.Errorf("skill not found: %s", skillName)
	}

	var builder strings.Builder
	builder.WriteString(basePrompt)
	builder.WriteString(fmt.Sprintf("\n# Loaded Skill: %s\n\n", skill.Name))
	builder.WriteString(skill.Body)
	builder.WriteString("\n")

	return builder.String(), nil
}

// GetSkillIndex returns a compact index of available skills
func (p *PromptAssembler) GetSkillIndex() string {
	skills := p.loaders.ListSkills()
	if len(skills) == 0 {
		return "No skills available."
	}

	var builder strings.Builder
	for _, skill := range skills {
		builder.WriteString(fmt.Sprintf("- %s: %s\n", skill.Name, skill.Description))
	}
	return builder.String()
}
