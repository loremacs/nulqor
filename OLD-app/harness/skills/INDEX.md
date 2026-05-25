# Skills Index

This directory contains skill modules. Each skill is a self-contained unit of knowledge or capability that can be loaded on demand.

## Skill Structure

Each skill is a folder containing:
- `SKILL.md` - The main skill file with YAML frontmatter and Markdown body
- `scripts/` - Optional scripts related to the skill
- `references/` - Optional reference documents
- `assets/` - Optional assets (images, etc.)

## Adding New Skills

To add a new skill:
1. Create a new folder in this directory
2. Create a `SKILL.md` file with the following structure:

```markdown
---
name: skill-name
description: A brief description of what this skill does
triggers:
  - keyword1
  - keyword2
---

# Skill Name

Detailed instructions and knowledge for this skill...
```

## Available Skills

*(Skills will be listed here as they are added)*

## See Also

- [../agents/INDEX.md](../agents/INDEX.md) - Agents directory index
- [../rules/INDEX.md](../rules/INDEX.md) - Rules directory index
