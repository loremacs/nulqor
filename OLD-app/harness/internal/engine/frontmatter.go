package engine

import (
	"fmt"
	"strings"

	"gopkg.in/yaml.v3"
)

type frontmatterMeta struct {
	Name        string   `yaml:"name"`
	Description string   `yaml:"description"`
	Triggers    []string `yaml:"triggers"`
}

func parseFrontmatter(content string) (meta frontmatterMeta, body string, ok bool) {
	parts := strings.SplitN(content, "---", 3)
	if len(parts) < 3 {
		return meta, strings.TrimSpace(content), false
	}

	frontmatter := strings.TrimSpace(parts[1])
	body = strings.TrimSpace(parts[2])

	if frontmatter == "" {
		return meta, body, false
	}

	if err := yaml.Unmarshal([]byte(frontmatter), &meta); err != nil {
		return meta, body, false
	}

	return meta, body, true
}

func parseFrontmatterRequired(content string) (frontmatterMeta, string, error) {
	meta, body, ok := parseFrontmatter(content)
	if !ok {
		return meta, "", fmt.Errorf("invalid format: missing frontmatter")
	}
	return meta, body, nil
}
