# Knack

Knack is a cross-platform CLI for adding agent skills from GitHub into any supported coding agent on your machine.

## Features
- Detects local and global agent roots automatically
- Defaults to local installs when a project has a local agent folder
- Supports GitHub shorthand (`owner/repo[/path]`), GitHub URLs (including tree URLs), and direct .zip URLs
- Optional `skill.json` manifest to copy extra files

## Install (local dev)
```sh
go test ./...

go run ./cmd/knack --help
```

## Usage
```sh
knack add <source> [--skill <name>] [--agent <id>] [--global] [--force]
knack remove <skill> [--global]
knack doctor
```

### Add examples
```sh
knack add anthropics/skills
knack add anthropics/skills/pdf
knack add anthropics/skills/pdf/SKILL.md
knack add anthropics/skills --agent claude
knack add https://github.com/org/repo
knack add https://example.com/skill.zip

# Force global add
knack add https://github.com/org/repo --global
```

### Remove examples
```sh
knack remove my-skill
knack remove my-skill --global
```

## Agent registry
Knack ships with a built-in agent registry (Codex + Claude). You can override or extend it using:
```txt
<OS config dir>/knack/config.json
```
On Linux/macOS, `~/.config/knack/config.json` is also supported for compatibility.
On Windows, the OS config dir typically maps to `%AppData%` (for example, `%AppData%\\knack\\config.json`).
See an example at:
```txt
docs/examples/config.json
```

### Config format
Each agent entry requires:
- `id`: stable identifier (string)
- `displayName`: label shown in prompts
- `globalConfig`: path to global agent config root (relative to home or absolute)
- `projectConfig`: path to project agent config root (relative to project or absolute)
- `skillsDir`: directory under the agent root where skills are installed

## Skill discovery
Knack considers a directory a skill if it contains `SKILL.md`. It will search:
1) the repo root (or subdir from a GitHub tree URL)
2) immediate subdirectories of that root that contain `SKILL.md`

If the URL points directly to a `SKILL.md` file on GitHub, Knack treats the parent folder as the skill.
If multiple skills are found and `--skill` is not provided, Knack will prompt you to choose.

## Optional skill manifest (`skill.json`)
If a `skill.json` file exists in the skill folder, Knack will copy additional files declared in `extraPaths`.
Paths are relative to the repo root and must not be absolute or contain `..` segments.

Example:
```json
{
  "extraPaths": [
    "references",
    "assets/logo.png"
  ]
}
```

## Notes
- Use `--global` to force adds into global agent roots; otherwise local roots are preferred.
- `--force` overwrites an existing skill folder.
