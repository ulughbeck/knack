package registry

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
)

// Agent defines where skills are installed for a supported coding agent.
type Agent struct {
	ID            string `json:"id"`
	DisplayName   string `json:"displayName"`
	GlobalConfig  string `json:"globalConfig"`
	ProjectConfig string `json:"projectConfig"`
	SkillsDir     string `json:"skillsDir"`
}

type Config struct {
	Agents []Agent `json:"agents"`
}

func DefaultAgents() []Agent {
	return []Agent{
		{
			ID:            "codex",
			DisplayName:   "Codex",
			GlobalConfig:  ".codex",
			ProjectConfig: ".codex",
			SkillsDir:     "skills",
		},
		{
			ID:            "claude",
			DisplayName:   "Claude",
			GlobalConfig:  ".claude",
			ProjectConfig: ".claude",
			SkillsDir:     "skills",
		},
		{
			ID:            "amp",
			DisplayName:   "Amp",
			GlobalConfig:  ".config/agents",
			ProjectConfig: ".agents",
			SkillsDir:     "skills",
		},
		{
			ID:            "opencode",
			DisplayName:   "OpenCode",
			GlobalConfig:  ".config/opencode",
			ProjectConfig: ".opencode",
			SkillsDir:     "skill",
		},
		{
			ID:            "goose",
			DisplayName:   "Goose",
			GlobalConfig:  ".config/goose",
			ProjectConfig: ".goose",
			SkillsDir:     "skills",
		},
		{
			ID:            "copilot",
			DisplayName:   "GitHub Copilot",
			GlobalConfig:  "",
			ProjectConfig: ".github",
			SkillsDir:     "skills",
		},
		{
			ID:            "letta",
			DisplayName:   "Letta",
			GlobalConfig:  "",
			ProjectConfig: ".skills",
			SkillsDir:     "",
		},
		{
			ID:            "cursor",
			DisplayName:   "Cursor",
			GlobalConfig:  ".cursor",
			ProjectConfig: ".cursor",
			SkillsDir:     "skills",
		},
	}
}

// LoadRegistry returns default agents plus any overrides from config.
// If the config file is unreadable or invalid, defaults are returned with an error.
func LoadRegistry() ([]Agent, error) {
	defaults := DefaultAgents()
	cfgPaths, err := configPaths()
	if err != nil {
		return defaults, err
	}
	for _, cfgPath := range cfgPaths {
		data, err := os.ReadFile(cfgPath)
		if err != nil {
			if errors.Is(err, os.ErrNotExist) {
				continue
			}
			return defaults, err
		}
		var cfg Config
		if err := json.Unmarshal(data, &cfg); err != nil {
			return defaults, err
		}
		if len(cfg.Agents) == 0 {
			return defaults, nil
		}
		merged := mergeAgents(defaults, cfg.Agents)
		return merged, nil
	}
	return defaults, nil
}

func mergeAgents(defaults, overrides []Agent) []Agent {
	byID := map[string]Agent{}
	order := []string{}
	for _, agent := range defaults {
		if agent.ID == "" {
			continue
		}
		byID[agent.ID] = agent
		order = append(order, agent.ID)
	}
	for _, agent := range overrides {
		if agent.ID == "" {
			continue
		}
		if _, exists := byID[agent.ID]; !exists {
			order = append(order, agent.ID)
		}
		byID[agent.ID] = agent
	}
	merged := make([]Agent, 0, len(byID))
	for _, id := range order {
		if agent, ok := byID[id]; ok {
			merged = append(merged, agent)
		}
	}
	return merged
}

func configPaths() ([]string, error) {
	paths := []string{}
	if configDir, err := os.UserConfigDir(); err == nil && configDir != "" {
		paths = append(paths, filepath.Join(configDir, "knack", "config.json"))
	}
	home, err := os.UserHomeDir()
	if err != nil {
		if len(paths) == 0 {
			return nil, err
		}
		return paths, nil
	}
	legacy := filepath.Join(home, ".config", "knack", "config.json")
	if len(paths) == 0 || paths[0] != legacy {
		paths = append(paths, legacy)
	}
	return paths, nil
}
