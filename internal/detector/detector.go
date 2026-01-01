package detector

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"knack/internal/registry"
)

type Target struct {
	Agent     registry.Agent
	Scope     string // "local" or "global"
	Root      string
	SkillsDir string
}

// DetectTargets checks for existing agent roots and returns detected targets.
func DetectTargets(agents []registry.Agent, cwd string) ([]Target, []string) {
	warnings := []string{}
	var targets []Target
	for _, agent := range agents {
		if agent.ID == "" {
			continue
		}
		if projectRoot := resolveProjectConfig(agent, cwd); projectRoot != "" {
			if exists(projectRoot) {
				targets = append(targets, Target{
					Agent:     agent,
					Scope:     "local",
					Root:      projectRoot,
					SkillsDir: filepath.Join(projectRoot, agent.SkillsDir),
				})
			}
		}
		if globalRoot := resolveGlobalConfig(agent); globalRoot != "" {
			if exists(globalRoot) {
				targets = append(targets, Target{
					Agent:     agent,
					Scope:     "global",
					Root:      globalRoot,
					SkillsDir: filepath.Join(globalRoot, agent.SkillsDir),
				})
			}
		}
	}
	return targets, warnings
}

// DefaultTargets prefers local targets when both scopes are available for an agent.
func DefaultTargets(targets []Target) []Target {
	byAgent := map[string]Target{}
	order := []string{}
	for _, t := range targets {
		if t.Agent.ID == "" {
			continue
		}
		if _, exists := byAgent[t.Agent.ID]; !exists {
			order = append(order, t.Agent.ID)
		}
		if t.Scope == "local" {
			byAgent[t.Agent.ID] = t
			continue
		}
		if _, exists := byAgent[t.Agent.ID]; !exists {
			byAgent[t.Agent.ID] = t
		}
	}
	defaults := make([]Target, 0, len(byAgent))
	for _, id := range order {
		if t, ok := byAgent[id]; ok {
			defaults = append(defaults, t)
		}
	}
	return defaults
}

func FilterScope(targets []Target, scope string) []Target {
	var filtered []Target
	for _, t := range targets {
		if t.Scope == scope {
			filtered = append(filtered, t)
		}
	}
	return filtered
}

func resolveProjectConfig(agent registry.Agent, cwd string) string {
	if agent.ProjectConfig == "" {
		return ""
	}
	if filepath.IsAbs(agent.ProjectConfig) {
		return agent.ProjectConfig
	}
	return filepath.Join(cwd, agent.ProjectConfig)
}

func resolveGlobalConfig(agent registry.Agent) string {
	if agent.GlobalConfig == "" {
		return ""
	}
	if filepath.IsAbs(agent.GlobalConfig) {
		return agent.GlobalConfig
	}
	clean := filepath.Clean(agent.GlobalConfig)
	if isDotConfigPath(clean) && runtime.GOOS == "windows" {
		if configDir, err := os.UserConfigDir(); err == nil && configDir != "" {
			trimmed := strings.TrimPrefix(clean, ".config")
			trimmed = strings.TrimPrefix(trimmed, string(os.PathSeparator))
			if trimmed == "" {
				return configDir
			}
			return filepath.Join(configDir, trimmed)
		}
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, clean)
}

func isDotConfigPath(clean string) bool {
	if clean == ".config" {
		return true
	}
	prefix := ".config" + string(os.PathSeparator)
	return strings.HasPrefix(clean, prefix)
}

func exists(path string) bool {
	info, err := os.Stat(path)
	if err != nil {
		return false
	}
	return info.IsDir()
}
