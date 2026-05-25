# Rules Index

This directory contains rule files that are always applied to the system prompt. Rules define constraints, guidelines, and behaviors that the model must follow.

## Rule Files

Rules can be in any of the following formats:
- `.md` - Markdown files
- `.mdc` - Markdown Common (used by Cursor)
- `.txt` - Plain text files

All rule files in this directory are concatenated and included in the system prompt.

## Adding New Rules

To add a new rule:
1. Create a new file in this directory (`.md`, `.mdc`, or `.txt`)
2. Write the rule content

Example rule file (`formatting.md`):
```markdown
# Formatting Rules

- Always use Markdown for code blocks
- Use proper indentation
- Follow the project's style guide
```

## Available Rules

*(Rules will be listed here as they are added)*

## See Also

- [../agents/INDEX.md](../agents/INDEX.md) - Agents directory index
- [../skills/INDEX.md](../skills/INDEX.md) - Skills directory index
