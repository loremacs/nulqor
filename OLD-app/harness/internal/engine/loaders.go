package engine

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// Skill represents a loaded skill
type Skill struct {
	Name        string
	Description string
	Triggers    []string
	Body        string
	Path        string
	Scripts     []string
	References  []string
	LoadedAt    time.Time
}

// Agent represents a loaded agent persona
type Agent struct {
	Name        string
	Description string
	Body        string
	Path        string
	LoadedAt    time.Time
}

// Rule represents a loaded rule
type Rule struct {
	Name     string    `toml:"-"`
	Body     string    `toml:"-"`
	Path     string    `toml:"-"`
	LoadedAt time.Time `toml:"-"`
}

// Loaders manages loading skills, agents, and rules
type Loaders struct {
	cfg *Config

	skills map[string]*Skill
	agents map[string]*Agent
	rules  []*Rule

	// Callbacks for hot-reload notifications
	onSkillsReloaded func([]*Skill)
	onAgentsReloaded func(map[string]*Agent)
	onRulesReloaded  func([]*Rule)
}

// NewLoaders creates a new loaders instance
func NewLoaders(cfg *Config) *Loaders {
	return &Loaders{
		cfg:    cfg,
		skills: make(map[string]*Skill),
		agents: make(map[string]*Agent),
		rules:  make([]*Rule, 0),
	}
}

// LoadAll loads all skills, agents, and rules
func (l *Loaders) LoadAll() error {
	if err := l.LoadSkills(); err != nil {
		return fmt.Errorf("failed to load skills: %w", err)
	}
	if err := l.LoadAgents(); err != nil {
		return fmt.Errorf("failed to load agents: %w", err)
	}
	if err := l.LoadRules(); err != nil {
		return fmt.Errorf("failed to load rules: %w", err)
	}
	return nil
}

// LoadSkills loads all skills from the skills directory
func (l *Loaders) LoadSkills() error {
	l.skills = make(map[string]*Skill)

	dir := l.cfg.Paths.SkillsDir
	entries, err := os.ReadDir(dir)
	if err != nil {
		if os.IsNotExist(err) {
			// Directory doesn't exist, that's ok
			return nil
		}
		return err
	}

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		skillPath := filepath.Join(dir, entry.Name())
		skillFile := filepath.Join(skillPath, "SKILL.md")

		skill, err := l.loadSkill(skillFile)
		if err != nil {
			// Log but don't fail on individual skill errors
			fmt.Printf("Warning: failed to load skill %s: %v\n", entry.Name(), err)
			continue
		}

		l.skills[skill.Name] = skill
	}

	if l.onSkillsReloaded != nil {
		skillList := make([]*Skill, 0, len(l.skills))
		for _, s := range l.skills {
			skillList = append(skillList, s)
		}
		l.onSkillsReloaded(skillList)
	}

	return nil
}

// loadSkill loads a single skill from its SKILL.md file
func (l *Loaders) loadSkill(path string) (*Skill, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	meta, body, err := parseFrontmatterRequired(string(data))
	if err != nil {
		return nil, err
	}

	skill := &Skill{
		Name:        meta.Name,
		Description: meta.Description,
		Triggers:    meta.Triggers,
		Body:        body,
		Path:        path,
		LoadedAt:    time.Now(),
	}

	if skill.Name == "" {
		skill.Name = filepath.Base(filepath.Dir(path))
	}

	// Scan for bundled assets
	skillDir := filepath.Dir(path)
	if scriptsDir := filepath.Join(skillDir, "scripts"); dirExists(scriptsDir) {
		files, _ := filepath.Glob(filepath.Join(scriptsDir, "*"))
		for _, f := range files {
			skill.Scripts = append(skill.Scripts, f)
		}
	}
	if refsDir := filepath.Join(skillDir, "references"); dirExists(refsDir) {
		files, _ := filepath.Glob(filepath.Join(refsDir, "*"))
		for _, f := range files {
			skill.References = append(skill.References, f)
		}
	}

	return skill, nil
}

// LoadAgents loads all agents from the agents directory
func (l *Loaders) LoadAgents() error {
	l.agents = make(map[string]*Agent)

	dir := l.cfg.Paths.AgentsDir
	entries, err := os.ReadDir(dir)
	if err != nil {
		if os.IsNotExist(err) {
			// Check for top-level AGENTS.md as fallback
			return l.loadTopLevelAgents()
		}
		return err
	}

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		name := entry.Name()
		if !strings.HasSuffix(strings.ToLower(name), ".md") {
			continue
		}
		if strings.EqualFold(name, "INDEX.md") {
			continue
		}

		agentPath := filepath.Join(dir, entry.Name())
		agent, err := l.loadAgent(agentPath)
		if err != nil {
			fmt.Printf("Warning: failed to load agent %s: %v\n", entry.Name(), err)
			continue
		}

		l.agents[agent.Name] = agent
	}

	// Always load top-level AGENTS.md as the default persona when present.
	if err := l.loadTopLevelAgents(); err != nil {
		if !os.IsNotExist(err) {
			fmt.Printf("Warning: failed to load AGENTS.md: %v\n", err)
		}
	}

	if l.onAgentsReloaded != nil {
		l.onAgentsReloaded(l.agents)
	}

	return nil
}

// loadTopLevelAgents loads AGENTS.md from the workspace root.
func (l *Loaders) loadTopLevelAgents() error {
	agentsPath := filepath.Join(filepath.Dir(l.cfg.Paths.AgentsDir), "AGENTS.md")
	if _, err := os.Stat(agentsPath); os.IsNotExist(err) {
		return err
	}

	agent, err := l.loadAgent(agentsPath)
	if err != nil {
		return err
	}

	if agent.Name == "" || strings.EqualFold(agent.Name, "AGENTS") {
		agent.Name = "default"
	}

	l.agents[agent.Name] = agent
	return nil
}

// loadAgent loads a single agent from its .md file
func (l *Loaders) loadAgent(path string) (*Agent, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	meta, body, ok := parseFrontmatter(string(data))
	agent := &Agent{
		Body:     body,
		Path:     path,
		LoadedAt: time.Now(),
	}

	if ok {
		agent.Name = meta.Name
		agent.Description = meta.Description
	} else {
		agent.Name = strings.TrimSuffix(filepath.Base(path), ".md")
		agent.Body = string(data)
	}

	if agent.Name == "" {
		agent.Name = strings.TrimSuffix(filepath.Base(path), ".md")
	}

	return agent, nil
}

// LoadRules loads all rules from the rules directory
func (l *Loaders) LoadRules() error {
	l.rules = make([]*Rule, 0)

	dir := l.cfg.Paths.RulesDir
	entries, err := os.ReadDir(dir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		name := entry.Name()
		if strings.EqualFold(name, "INDEX.md") {
			continue
		}
		if !strings.HasSuffix(strings.ToLower(name), ".md") &&
			!strings.HasSuffix(strings.ToLower(name), ".mdc") &&
			!strings.HasSuffix(strings.ToLower(name), ".txt") {
			continue
		}

		rulePath := filepath.Join(dir, name)
		rule, err := l.loadRule(rulePath)
		if err != nil {
			fmt.Printf("Warning: failed to load rule %s: %v\n", name, err)
			continue
		}

		l.rules = append(l.rules, rule)
	}

	if l.onRulesReloaded != nil {
		l.onRulesReloaded(l.rules)
	}

	return nil
}

// loadRule loads a single rule file
func (l *Loaders) loadRule(path string) (*Rule, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	return &Rule{
		Name:     filepath.Base(path),
		Body:     string(data),
		Path:     path,
		LoadedAt: time.Now(),
	}, nil
}

// GetSkill returns a skill by name
func (l *Loaders) GetSkill(name string) (*Skill, bool) {
	skill, ok := l.skills[name]
	return skill, ok
}

// ListSkills returns all skills
func (l *Loaders) ListSkills() []*Skill {
	skills := make([]*Skill, 0, len(l.skills))
	for _, s := range l.skills {
		skills = append(skills, s)
	}
	return skills
}

// GetAgent returns an agent by name
func (l *Loaders) GetAgent(name string) (*Agent, bool) {
	agent, ok := l.agents[name]
	return agent, ok
}

// ListAgents returns all agents
func (l *Loaders) ListAgents() map[string]*Agent {
	return l.agents
}

// GetRules returns all rules
func (l *Loaders) GetRules() []*Rule {
	return l.rules
}

// SetReloadCallbacks sets callbacks for hot-reload notifications
func (l *Loaders) SetReloadCallbacks(
	onSkillsReloaded func([]*Skill),
	onAgentsReloaded func(map[string]*Agent),
	onRulesReloaded func([]*Rule),
) {
	l.onSkillsReloaded = onSkillsReloaded
	l.onAgentsReloaded = onAgentsReloaded
	l.onRulesReloaded = onRulesReloaded
}

// ReadFile reads a file's content (sandboxed to project dirs)
func (l *Loaders) ReadFile(path string) ([]byte, error) {
	// Sandbox check: ensure path is within one of the allowed directories
	allowedDirs := []string{
		l.cfg.Paths.SkillsDir,
		l.cfg.Paths.AgentsDir,
		l.cfg.Paths.RulesDir,
	}

	absPath, err := filepath.Abs(path)
	if err != nil {
		return nil, err
	}

	allowed := false
	for _, dir := range allowedDirs {
		absDir, _ := filepath.Abs(dir)
		if strings.HasPrefix(absPath, absDir) {
			allowed = true
			break
		}
	}

	if !allowed {
		return nil, fmt.Errorf("path not in allowed directory")
	}

	return os.ReadFile(path)
}

// WriteFile writes content to a file (sandboxed to project dirs)
func (l *Loaders) WriteFile(path string, content []byte) error {
	// Sandbox check
	allowedDirs := []string{
		l.cfg.Paths.SkillsDir,
		l.cfg.Paths.AgentsDir,
		l.cfg.Paths.RulesDir,
	}

	absPath, err := filepath.Abs(path)
	if err != nil {
		return err
	}

	allowed := false
	for _, dir := range allowedDirs {
		absDir, _ := filepath.Abs(dir)
		if strings.HasPrefix(absPath, absDir) {
			allowed = true
			break
		}
	}

	if !allowed {
		return fmt.Errorf("path not in allowed directory")
	}

	return os.WriteFile(path, content, 0644)
}

func dirExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}
