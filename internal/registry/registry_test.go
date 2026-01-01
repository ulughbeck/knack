package registry

import "testing"

func TestMergeAgentsOverride(t *testing.T) {
	defaults := []Agent{
		{ID: "codex", DisplayName: "Codex", GlobalConfig: ".codex", ProjectConfig: ".codex", SkillsDir: "skills"},
		{ID: "claude", DisplayName: "Claude", GlobalConfig: ".claude", ProjectConfig: ".claude", SkillsDir: "skills"},
	}
	overrides := []Agent{
		{ID: "codex", DisplayName: "CodexX", GlobalConfig: ".codexx", ProjectConfig: ".codexx", SkillsDir: "skills"},
		{ID: "new", DisplayName: "New", GlobalConfig: ".new", ProjectConfig: ".new", SkillsDir: "skills"},
	}
	merged := mergeAgents(defaults, overrides)
	if len(merged) != 3 {
		t.Fatalf("expected 3 agents, got %d", len(merged))
	}
	if merged[0].DisplayName != "CodexX" {
		t.Fatalf("expected override applied, got %s", merged[0].DisplayName)
	}
	if merged[2].ID != "new" {
		t.Fatalf("expected new agent appended")
	}
}
