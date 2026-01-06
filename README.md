# Knack

Knack is a cross-platform CLI for adding agent skills from GitHub into any supported coding agent on your machine.

## Install:

### Install the latest release (macOS + Linux)
```sh
curl -fsSL https://raw.githubusercontent.com/ulughbeck/knack/main/scripts/install.sh | sh
```

### Install the latest release (Windows)
```powershell
iwr -useb https://raw.githubusercontent.com/ulughbeck/knack/main/scripts/install.ps1 | iex
```

### Install a specific version
```sh
curl -fsSL https://raw.githubusercontent.com/ulughbeck/knack/main/scripts/install.sh | sh -s v0.0.1
```

### Install a specific version (Windows)
```powershell
$env:VERSION = "v0.0.1"
iwr -useb https://raw.githubusercontent.com/ulughbeck/knack/main/scripts/install.ps1 | iex
```

### Manual install from GitHub Releases
```sh
# macOS:
tar -xzf knack-macos-arm64.tar.gz
# or: tar -xzf knack-macos-x86_64.tar.gz
#
# Linux:
# tar -xzf knack-linux-arm64.tar.gz
# or: tar -xzf knack-linux-x86_64.tar.gz
#
# Windows (PowerShell):
# Expand-Archive -Path knack-windows-x86_64.zip -DestinationPath .
mkdir -p /usr/local/bin
cp knack /usr/local/bin/knack
```

### Manual install from source code
```sh
cargo test
cargo install --path .
```

## What does Knack do?

Knack installs and removes skills for supported coding agents on your machine. It pulls skills from GitHub or zip URLs, caches global skills, and copies them into each agent’s skills directory. The CLI is interactive: you can select multiple skills and multiple agents from lists.

### Features
- Detects project and global agent roots automatically
- Defaults to global installs; use `--project` to install into the current repo
- Interactive multi-select for both skills and target agents
- Supports GitHub shorthand (`owner/repo[/path]`), GitHub URLs (including tree URLs), and direct .zip URLs
- Optional `skill.json` manifest to copy extra files

### Usage
```sh
knack add <source> [--skill <name>] [--agent <id>] [--project] [--force]
knack rm <skill> [--project]
knack doctor
```

### Add (install)
Knack adds skills globally by default and lets you select the agents to install into. Use `--project` to install into the current repo’s agent config instead.

#### Examples
```sh
# Install all skills from a GitHub repository
knack add anthropics/skills

# Install a skill for a specific agent
knack add anthropics/skills --agent claude

# Install a specific skill from GitHub
knack add anthropics/skills/pdf
knack add anthropics/skills/pdf/SKILL.md

# Install from a git URL
knack add https://github.com/org/repo

# Install from a zip download URL
knack add https://example.com/skill.zip

# Install into project instead of global
knack add https://github.com/org/repo --project
```

#### How add works
- Global by default; `--project` installs into the current repo instead
- Prompts to select multiple skills if the source contains more than one
- Prompts to select target agents (global first, project selectable)
- `--force` overwrites an existing skill folder
- Global installs are cached under the skills root and then copied into each agent’s global skills directory
- Project installs are copied directly into the project

### Skill discovery
Knack considers a directory a skill if it contains `SKILL.md`. It will search:
1) the repo root (or subdir from a GitHub tree URL)
2) immediate subdirectories of that root that contain `SKILL.md`

If the URL points directly to a `SKILL.md` file on GitHub, Knack treats the parent folder as the skill.
If multiple skills are found and `--skill` is not provided, Knack will prompt you to choose.

### Optional skill manifest (`skill.json`)
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

### Remove
Removes are global by default; use `--project` to remove from the current repo.

#### Remove examples
```sh
# Remove skill from global agents 
knack rm my-skill

# Remove skill from project 
knack rm my-skill --project
```

## Configuration

Knack uses a JSONC config file (comments + trailing commas) as the source of truth for global installs:
```txt
~/.config/knack/config.jsonc
```
On Windows, the config dir typically maps to `%AppData%` (for example, `%AppData%\\knack\\config.jsonc`).

Global skills are cached in the Knack skills root (default `~/.config/knack/skills`) and then
copied into each agent’s global skills directory.

### Settings
- `skillsRoot`: optional path to the global cache (defaults to `~/.config/knack/skills` on macOS/Linux, `%AppData%\\knack\\skills` on Windows)

### Supported agents (built-in defaults)
Blank fields indicate no default path is required for that tool.

| id | display name | globalConfig | projectConfig | skillsDir |
| --- | --- | --- | --- | --- |
| codex | Codex | .codex | .codex | skills |
| claude | Claude | .claude | .claude | skills |
| amp | Amp | .config/agents | .agents | skills |
| opencode | OpenCode | .config/opencode | .opencode | skill |
| goose | Goose | .config/goose | .goose | skills |
| copilot | GitHub Copilot | - | .github | skills |
| cursor-agent | Cursor | .cursor | .cursor | skills |

### Config format
To add any custom agents just edit the config file. Each agent entry requires:
- `id`: stable identifier (string)
- `displayName`: label shown in prompts
- `globalConfig`: path to global agent config root (relative to home or absolute)
- `projectConfig`: path to project agent config root (relative to project or absolute)
- `skillsDir`: directory under the agent root where skills are installed

Each skill entry requires:
- `name`: skill folder name
- `source`: skill source (GitHub shorthand/URL/zip). Optional if already cached.
- `agents`: explicit list of agent ids to install globally

### Example config
```json
{
  "settings": {
    "skillsRoot": "~/.config/knack/skills"
  },
  "agents": [
    {
      "id": "codex",
      "displayName": "Codex",
      "globalConfig": ".codex",
      "projectConfig": ".codex",
      "skillsDir": "skills"
    },
    {
      "id": "claude",
      "displayName": "Claude",
      "globalConfig": ".claude",
      "projectConfig": ".claude",
      "skillsDir": "skills"
    },
    {
      "id": "custom-agent",
      "displayName": "Custom Agent",
      "globalConfig": ".customagent",
      "projectConfig": ".customagent",
      "skillsDir": "skills"
    }
  ],
  "skills": [
    {
      "name": "pdf",
      "source": "anthropics/skills/pdf",
      "agents": ["claude", "codex"]
    }
  ]
}
```

## Contributing
Contributions are welcome! Please:
- Fork the repository
- Create a feature branch
- Make your changes with tests
- Submit a pull request
