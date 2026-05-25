# Agents Index

This directory contains agent persona files. Each agent defines a specific personality and behavior for the AI model.

## Available Agents

- **default** - The default agent persona (see AGENTS.md in project root)

## Adding New Agents

To add a new agent:
1. Create a new `.md` file in this directory
2. Include YAML frontmatter with `name` and `description`
3. Write the persona body in Markdown

Example:
```markdown
---
name: coder
description: An expert programmer agent
---

You are an expert programmer with deep knowledge of multiple languages...
```

## See Also

- [../skills/INDEX.md](../skills/INDEX.md) - Skills directory index
- [../rules/INDEX.md](../rules/INDEX.md) - Rules directory index
