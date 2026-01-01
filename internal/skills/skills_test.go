package skills

import (
	"os"
	"path/filepath"
	"testing"
)

func TestFindSkillDirsRoot(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "SKILL.md"), []byte("test"), 0o644); err != nil {
		t.Fatal(err)
	}
	source := Source{Root: root}
	skillsList, err := FindSkillDirs(source, "")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(skillsList) != 1 {
		t.Fatalf("expected 1 skill, got %d", len(skillsList))
	}
}

func TestFindSkillDirsChildFolder(t *testing.T) {
	root := t.TempDir()
	child := filepath.Join(root, "alpha")
	if err := os.MkdirAll(child, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(child, "SKILL.md"), []byte("test"), 0o644); err != nil {
		t.Fatal(err)
	}
	source := Source{Root: root}
	skillsList, err := FindSkillDirs(source, "")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(skillsList) != 1 || skillsList[0].Name != "alpha" {
		t.Fatalf("expected skill alpha, got %+v", skillsList)
	}
}

func TestFindSkillDirsWithName(t *testing.T) {
	root := t.TempDir()
	beta := filepath.Join(root, "beta")
	if err := os.MkdirAll(beta, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(beta, "SKILL.md"), []byte("test"), 0o644); err != nil {
		t.Fatal(err)
	}
	source := Source{Root: root}
	skillsList, err := FindSkillDirs(source, "beta")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(skillsList) != 1 || skillsList[0].Name != "beta" {
		t.Fatalf("expected skill beta, got %+v", skillsList)
	}
}

func TestParseGitHubShorthand(t *testing.T) {
	cases := []struct {
		input  string
		owner  string
		repo   string
		subdir string
		ok     bool
	}{
		{input: "anthropics/skills", owner: "anthropics", repo: "skills", subdir: "", ok: true},
		{input: "anthropics/skills/pdf", owner: "anthropics", repo: "skills", subdir: "pdf", ok: true},
		{input: "anthropics/skills/pdf/SKILL.md", owner: "anthropics", repo: "skills", subdir: "pdf", ok: true},
		{input: "github.com/anthropics/skills/pdf", owner: "anthropics", repo: "skills", subdir: "pdf", ok: true},
		{input: "./skills", ok: false},
	}
	for _, tc := range cases {
		owner, repo, subdir, ok := parseGitHubShorthand(tc.input)
		if ok != tc.ok {
			t.Fatalf("input %q ok=%v, want %v", tc.input, ok, tc.ok)
		}
		if !ok {
			continue
		}
		if owner != tc.owner || repo != tc.repo || subdir != tc.subdir {
			t.Fatalf("input %q got owner=%q repo=%q subdir=%q", tc.input, owner, repo, subdir)
		}
	}
}
