package detector

import (
	"testing"

	"knack/internal/registry"
)

func TestDefaultTargetsPrefersLocal(t *testing.T) {
	agent := registry.Agent{ID: "codex", DisplayName: "Codex"}
	local := Target{Agent: agent, Scope: "local", Root: "/tmp/local", SkillsDir: "/tmp/local/skills"}
	global := Target{Agent: agent, Scope: "global", Root: "/tmp/global", SkillsDir: "/tmp/global/skills"}
	targets := []Target{global, local}
	defaults := DefaultTargets(targets)
	if len(defaults) != 1 {
		t.Fatalf("expected 1 default target, got %d", len(defaults))
	}
	if defaults[0].Scope != "local" {
		t.Fatalf("expected local target, got %s", defaults[0].Scope)
	}
}
