package main

import (
	"errors"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"knack/internal/cli"
	"knack/internal/detector"
	"knack/internal/registry"
	"knack/internal/skills"
)

func main() {
	if len(os.Args) < 2 {
		usage()
		os.Exit(1)
	}

	switch os.Args[1] {
	case "add":
		os.Exit(runAdd(os.Args[2:]))
	case "remove":
		os.Exit(runRemove(os.Args[2:]))
	case "doctor":
		os.Exit(runDoctor(os.Args[2:]))
	case "help", "-h", "--help":
		usage()
		os.Exit(0)
	default:
		fmt.Fprintf(os.Stderr, "Unknown command: %s\n", os.Args[1])
		usage()
		os.Exit(1)
	}
}

func usage() {
	fmt.Fprintln(os.Stdout, "Knack - manage agent skills for all your agents")
	fmt.Fprintln(os.Stdout, "")
	fmt.Fprintln(os.Stdout, "Usage:")
	fmt.Fprintln(os.Stdout, "  knack add <source> [--skill <name>] [--agent <id>] [--global] [--force]")
	fmt.Fprintln(os.Stdout, "  knack remove <skill> [--global]")
	fmt.Fprintln(os.Stdout, "  knack doctor")
}

func runAdd(args []string) int {
	fs := flag.NewFlagSet("add", flag.ContinueOnError)
	skillName := fs.String("skill", "", "skill name")
	agentID := fs.String("agent", "", "agent id")
	global := fs.Bool("global", false, "add to global agent roots")
	forceFlag := fs.Bool("force", false, "overwrite existing skill")
	fs.SetOutput(os.Stderr)
	if err := fs.Parse(args); err != nil {
		return 1
	}
	if fs.NArg() < 1 {
		fmt.Fprintln(os.Stderr, "add requires a source")
		return 1
	}
	sourceInput := fs.Arg(0)

	agents, err := registry.LoadRegistry()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to load config: %v\n", err)
	}
	targets, err := resolveTargets(agents, *global, *agentID)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}

	source, cleanup, err := skills.FetchSource(sourceInput)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	defer cleanup()

	skillDirs, err := skills.FindSkillDirs(source, *skillName)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	selectedSkill, err := selectSkillDir(skillDirs)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}

	force := *forceFlag
	existingTargets, err := findExistingSkillTargets(targets, selectedSkill.Name)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	if len(existingTargets) > 0 && !force {
		if !cli.IsInteractive() {
			fmt.Fprintln(os.Stderr, "skill already exists; use --force to overwrite")
			return 1
		}
		choices := []string{"Overwrite existing skills", "Do nothing"}
		prompt := fmt.Sprintf("Skill %q already installed for: %s", selectedSkill.Name, formatTargetList(existingTargets))
		choice, err := cli.SelectOne(prompt, choices, 1)
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			return 1
		}
		if choice == 1 {
			return 0
		}
		force = true
	}

	for _, target := range targets {
		if err := installSkill(target, selectedSkill, source, force); err != nil {
			fmt.Fprintf(os.Stderr, "Add failed for %s (%s): %v\n", target.Agent.DisplayName, target.Scope, err)
			return 1
		}
		fmt.Fprintf(os.Stdout, "Added %s to %s (%s)\n", selectedSkill.Name, target.Agent.DisplayName, target.Scope)
	}
	return 0
}

func runRemove(args []string) int {
	fs := flag.NewFlagSet("remove", flag.ContinueOnError)
	global := fs.Bool("global", false, "remove from global agent roots")
	fs.SetOutput(os.Stderr)
	if err := fs.Parse(args); err != nil {
		return 1
	}
	if fs.NArg() < 1 {
		fmt.Fprintln(os.Stderr, "remove requires a skill name")
		return 1
	}
	skillName := fs.Arg(0)

	agents, err := registry.LoadRegistry()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to load config: %v\n", err)
	}
	targets, err := resolveTargets(agents, *global, "")
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}

	for _, target := range targets {
		path := filepath.Join(target.SkillsDir, skillName)
		if _, err := os.Stat(path); err != nil {
			if errors.Is(err, os.ErrNotExist) {
				fmt.Fprintf(os.Stdout, "Skill %s not found in %s (%s)\n", skillName, target.Agent.DisplayName, target.Scope)
				continue
			}
			fmt.Fprintf(os.Stderr, "Failed to check skill in %s (%s): %v\n", target.Agent.DisplayName, target.Scope, err)
			return 1
		}
		if err := os.RemoveAll(path); err != nil {
			fmt.Fprintf(os.Stderr, "Failed to remove from %s (%s): %v\n", target.Agent.DisplayName, target.Scope, err)
			return 1
		}
		fmt.Fprintf(os.Stdout, "Removed %s from %s (%s)\n", skillName, target.Agent.DisplayName, target.Scope)
	}
	return 0
}

func runDoctor(args []string) int {
	fs := flag.NewFlagSet("doctor", flag.ContinueOnError)
	fs.SetOutput(os.Stderr)
	if err := fs.Parse(args); err != nil {
		return 1
	}
	agents, err := registry.LoadRegistry()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to load config: %v\n", err)
	}
	cwd, err := os.Getwd()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	targets, _ := detector.DetectTargets(agents, cwd)
	globalTargets := detector.FilterScope(targets, "global")
	localTargets := detector.FilterScope(targets, "local")

	printDoctorSection("Agents found:", globalTargets)
	if len(localTargets) > 0 {
		fmt.Fprintln(os.Stdout, "")
		printDoctorSection("Project settings:", localTargets)
	}
	return 0
}

func printDoctorSection(title string, targets []detector.Target) {
	fmt.Fprintln(os.Stdout, title)
	if len(targets) == 0 {
		fmt.Fprintln(os.Stdout, "- None detected.")
		return
	}
	for _, target := range targets {
		count, warn := countSkills(target.SkillsDir)
		if warn != "" {
			fmt.Fprintf(os.Stdout, "- %s (skills=%d, warning=%s)\n", target.Agent.DisplayName, count, warn)
			continue
		}
		fmt.Fprintf(os.Stdout, "- %s (skills=%d)\n", target.Agent.DisplayName, count)
	}
}

func resolveTargets(agents []registry.Agent, forceGlobal bool, agentID string) ([]detector.Target, error) {
	cwd, err := os.Getwd()
	if err != nil {
		return nil, err
	}
	if agentID != "" && !agentExists(agents, agentID) {
		return nil, fmt.Errorf("unknown agent id: %s", agentID)
	}
	targets, _ := detector.DetectTargets(agents, cwd)
	if agentID != "" {
		targets = filterTargetsByAgent(targets, agentID)
	}
	if forceGlobal {
		targets = detector.FilterScope(targets, "global")
	}
	if len(targets) == 0 {
		return nil, fmt.Errorf("no agent targets found")
	}
	if len(targets) == 1 {
		return targets, nil
	}
	if !cli.IsInteractive() {
		return detector.DefaultTargets(targets), nil
	}
	return promptTargets(targets)
}

func agentExists(agents []registry.Agent, id string) bool {
	for _, agent := range agents {
		if agent.ID == id {
			return true
		}
	}
	return false
}

func filterTargetsByAgent(targets []detector.Target, id string) []detector.Target {
	filtered := make([]detector.Target, 0, len(targets))
	for _, target := range targets {
		if target.Agent.ID == id {
			filtered = append(filtered, target)
		}
	}
	return filtered
}

func promptTargets(targets []detector.Target) ([]detector.Target, error) {
	options := make([]string, len(targets))
	defaultTargets := detector.DefaultTargets(targets)
	defaultIndexes := []int{}
	for i, target := range targets {
		scope := target.Scope
		if scope == "local" {
			scope = "project"
		}
		options[i] = fmt.Sprintf("%s (%s) - %s", target.Agent.DisplayName, scope, target.Root)
		for _, def := range defaultTargets {
			if def.Agent.ID == target.Agent.ID && def.Scope == target.Scope {
				defaultIndexes = append(defaultIndexes, i)
				break
			}
		}
	}
	indexes, err := cli.SelectMultiple("Select agent targets:", options, defaultIndexes)
	if err != nil {
		return nil, err
	}
	if len(indexes) == 0 {
		return nil, fmt.Errorf("no targets selected")
	}
	selection := make([]detector.Target, 0, len(indexes))
	for _, idx := range indexes {
		selection = append(selection, targets[idx])
	}
	return selection, nil
}

func selectSkillDir(skillsList []skills.SkillDir) (skills.SkillDir, error) {
	if len(skillsList) == 1 {
		return skillsList[0], nil
	}
	if !cli.IsInteractive() {
		return skills.SkillDir{}, fmt.Errorf("multiple skills found; rerun with --skill")
	}
	options := make([]string, len(skillsList))
	for i, skill := range skillsList {
		options[i] = skill.Name
	}
	index, err := cli.SelectOne("Select a skill:", options, 0)
	if err != nil {
		return skills.SkillDir{}, err
	}
	return skillsList[index], nil
}

func findExistingSkillTargets(targets []detector.Target, skillName string) ([]detector.Target, error) {
	existing := []detector.Target{}
	for _, target := range targets {
		path := filepath.Join(target.SkillsDir, skillName)
		if _, err := os.Stat(path); err != nil {
			if errors.Is(err, os.ErrNotExist) {
				continue
			}
			return nil, err
		}
		existing = append(existing, target)
	}
	return existing, nil
}

func formatTargetList(targets []detector.Target) string {
	parts := make([]string, 0, len(targets))
	for _, target := range targets {
		scope := "global"
		if target.Scope == "local" {
			scope = "project"
		}
		parts = append(parts, fmt.Sprintf("%s (%s)", target.Agent.DisplayName, scope))
	}
	return strings.Join(parts, ", ")
}

func installSkill(target detector.Target, skill skills.SkillDir, source skills.Source, force bool) error {
	if err := os.MkdirAll(target.SkillsDir, 0o755); err != nil {
		return err
	}
	dest := filepath.Join(target.SkillsDir, skill.Name)
	if _, err := os.Stat(dest); err == nil {
		if !force {
			return fmt.Errorf("skill already exists; use --force to overwrite")
		}
		if err := os.RemoveAll(dest); err != nil {
			return err
		}
	}
	if err := skills.CopyDir(skill.Path, dest); err != nil {
		return err
	}
	manifest, err := skills.LoadManifest(skill.Path)
	if err != nil {
		return err
	}
	if len(manifest.ExtraPaths) > 0 {
		if err := skills.CopyExtras(manifest, source.Root, target.Root); err != nil {
			return err
		}
	}
	return nil
}

func countSkills(dir string) (int, string) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			if err := os.MkdirAll(dir, 0o755); err != nil {
				return 0, err.Error()
			}
			return 0, ""
		}
		return 0, err.Error()
	}
	count := 0
	for _, entry := range entries {
		if entry.IsDir() {
			count++
		}
	}
	return count, ""
}
