package skills

import (
	"archive/zip"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"strings"
	"time"
)

type Source struct {
	Root   string
	Subdir string
	URL    string
}

type SkillDir struct {
	Name string
	Path string
}

type Manifest struct {
	ExtraPaths []string `json:"extraPaths"`
}

// FetchSource downloads or resolves a source location into a local directory.
func FetchSource(input string) (Source, func(), error) {
	cleanup := func() {}
	if input == "" {
		return Source{}, cleanup, errors.New("empty source")
	}
	if info, err := os.Stat(input); err == nil && info.IsDir() {
		return Source{Root: input, URL: input}, cleanup, nil
	}
	if info, err := os.Stat(input); err == nil && !info.IsDir() {
		if filepath.Base(input) == "SKILL.md" {
			return Source{Root: filepath.Dir(input), URL: input}, cleanup, nil
		}
		return Source{}, cleanup, fmt.Errorf("unsupported file path: %s", input)
	}
	if owner, repo, subdir, ok := parseGitHubShorthand(input); ok {
		root, clean, err := downloadGitHubRepo(owner, repo, "")
		if err != nil {
			return Source{}, cleanup, err
		}
		return Source{Root: root, Subdir: subdir, URL: input}, clean, nil
	}
	parsed, err := url.Parse(input)
	if err != nil {
		return Source{}, cleanup, err
	}
	if parsed.Scheme == "" {
		return Source{}, cleanup, fmt.Errorf("unsupported source: %s", input)
	}
	if parsed.Host == "github.com" {
		owner, repo, ref, subdir, ok := parseGitHubURL(parsed)
		if !ok {
			return Source{}, cleanup, fmt.Errorf("unsupported GitHub URL: %s", input)
		}
		root, clean, err := downloadGitHubRepo(owner, repo, ref)
		if err != nil {
			return Source{}, cleanup, err
		}
		return Source{Root: root, Subdir: subdir, URL: input}, clean, nil
	}
	if strings.HasSuffix(parsed.Path, ".zip") {
		root, clean, err := downloadZip(input)
		if err != nil {
			return Source{}, cleanup, err
		}
		return Source{Root: root, URL: input}, clean, nil
	}
	return Source{}, cleanup, fmt.Errorf("unsupported source URL: %s", input)
}

func parseGitHubShorthand(input string) (owner, repo, subdir string, ok bool) {
	if strings.Contains(input, "://") {
		return "", "", "", false
	}
	if strings.HasPrefix(input, "/") || strings.HasPrefix(input, "./") || strings.HasPrefix(input, "../") {
		return "", "", "", false
	}
	trimmed := strings.TrimPrefix(input, "github.com/")
	parts := strings.Split(strings.Trim(trimmed, "/"), "/")
	if len(parts) < 2 {
		return "", "", "", false
	}
	owner = parts[0]
	repo = strings.TrimSuffix(parts[1], ".git")
	if owner == "" || repo == "" {
		return "", "", "", false
	}
	if len(parts) > 2 {
		subdir = path.Clean(strings.Join(parts[2:], "/"))
		if subdir == "." {
			subdir = ""
		}
		if strings.HasSuffix(subdir, "/SKILL.md") || subdir == "SKILL.md" {
			subdir = path.Dir(subdir)
			if subdir == "." {
				subdir = ""
			}
		}
		if strings.HasPrefix(subdir, "..") {
			return "", "", "", false
		}
	}
	return owner, repo, subdir, true
}

func FindSkillDirs(source Source, skillName string) ([]SkillDir, error) {
	base := source.Root
	if source.Subdir != "" {
		base = filepath.Join(base, source.Subdir)
	}
	if skillName != "" {
		if isSkillDir(base) && filepath.Base(base) == skillName {
			return []SkillDir{{Name: filepath.Base(base), Path: base}}, nil
		}
		candidate := filepath.Join(base, skillName)
		if isSkillDir(candidate) {
			return []SkillDir{{Name: filepath.Base(candidate), Path: candidate}}, nil
		}
		return nil, fmt.Errorf("skill %q not found", skillName)
	}
	if isSkillDir(base) {
		return []SkillDir{{Name: filepath.Base(base), Path: base}}, nil
	}
	list := []SkillDir{}
	entries, err := os.ReadDir(base)
	if err != nil {
		return nil, fmt.Errorf("no skill directories found")
	}
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		candidate := filepath.Join(base, entry.Name())
		if isSkillDir(candidate) {
			list = append(list, SkillDir{Name: entry.Name(), Path: candidate})
		}
	}
	if len(list) == 0 {
		return nil, fmt.Errorf("no skill directories found")
	}
	return list, nil
}

func LoadManifest(skillPath string) (Manifest, error) {
	manifestPath := filepath.Join(skillPath, "skill.json")
	data, err := os.ReadFile(manifestPath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return Manifest{}, nil
		}
		return Manifest{}, err
	}
	var manifest Manifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		return Manifest{}, err
	}
	return manifest, nil
}

func CopyDir(src, dest string) error {
	info, err := os.Stat(src)
	if err != nil {
		return err
	}
	if !info.IsDir() {
		return fmt.Errorf("source is not a directory: %s", src)
	}
	return filepath.Walk(src, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		target := filepath.Join(dest, rel)
		if info.IsDir() {
			return os.MkdirAll(target, info.Mode())
		}
		return copyFile(path, target, info.Mode())
	})
}

func CopyPath(src, dest string) error {
	info, err := os.Stat(src)
	if err != nil {
		return err
	}
	if info.IsDir() {
		return CopyDir(src, dest)
	}
	return copyFile(src, dest, info.Mode())
}

func copyFile(src, dest string, mode os.FileMode) error {
	if err := os.MkdirAll(filepath.Dir(dest), 0o755); err != nil {
		return err
	}
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.Create(dest)
	if err != nil {
		return err
	}
	defer func() {
		_ = out.Close()
	}()
	if _, err := io.Copy(out, in); err != nil {
		return err
	}
	return os.Chmod(dest, mode)
}

func isSkillDir(dir string) bool {
	info, err := os.Stat(dir)
	if err != nil || !info.IsDir() {
		return false
	}
	_, err = os.Stat(filepath.Join(dir, "SKILL.md"))
	return err == nil
}

func parseGitHubURL(u *url.URL) (owner, repo, ref, subdir string, ok bool) {
	parts := strings.Split(strings.Trim(u.Path, "/"), "/")
	if len(parts) < 2 {
		return "", "", "", "", false
	}
	owner = parts[0]
	repo = strings.TrimSuffix(parts[1], ".git")
	if owner == "" || repo == "" {
		return "", "", "", "", false
	}
	if len(parts) >= 4 && parts[2] == "tree" {
		ref = parts[3]
		if len(parts) > 4 {
			subdir = path.Clean(strings.Join(parts[4:], "/"))
			if strings.HasSuffix(subdir, "/SKILL.md") || subdir == "SKILL.md" {
				subdir = path.Dir(subdir)
				if subdir == "." {
					subdir = ""
				}
			}
			if strings.HasPrefix(subdir, "..") {
				return "", "", "", "", false
			}
		}
	}
	if len(parts) >= 4 && parts[2] == "blob" {
		if parts[len(parts)-1] != "SKILL.md" {
			return "", "", "", "", false
		}
		ref = parts[3]
		if len(parts) > 4 {
			subdir = path.Clean(strings.Join(parts[4:len(parts)-1], "/"))
			if subdir == "." {
				subdir = ""
			}
			if strings.HasPrefix(subdir, "..") {
				return "", "", "", "", false
			}
		}
	}
	return owner, repo, ref, subdir, true
}

func downloadGitHubRepo(owner, repo, ref string) (string, func(), error) {
	candidates := []string{}
	if ref != "" {
		candidates = append(candidates, ref)
	} else {
		candidates = append(candidates, "main", "master")
	}
	var lastErr error
	for _, candidate := range candidates {
		zipURL := fmt.Sprintf("https://codeload.github.com/%s/%s/zip/refs/heads/%s", owner, repo, candidate)
		root, cleanup, err := downloadZip(zipURL)
		if err == nil {
			return root, cleanup, nil
		}
		lastErr = err
	}
	if lastErr == nil {
		lastErr = fmt.Errorf("unable to download repository")
	}
	return "", func() {}, lastErr
}

func downloadZip(zipURL string) (string, func(), error) {
	client := &http.Client{Timeout: 60 * time.Second}
	req, err := http.NewRequest(http.MethodGet, zipURL, nil)
	if err != nil {
		return "", func() {}, err
	}
	req.Header.Set("User-Agent", "knack")
	resp, err := client.Do(req)
	if err != nil {
		return "", func() {}, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return "", func() {}, fmt.Errorf("failed to download (%s): %s", zipURL, resp.Status)
	}
	zipFile, err := os.CreateTemp("", "knack-src-*.zip")
	if err != nil {
		return "", func() {}, err
	}
	defer zipFile.Close()
	zipPath := zipFile.Name()
	if _, err := io.Copy(zipFile, resp.Body); err != nil {
		return "", func() {}, err
	}
	if err := zipFile.Sync(); err != nil {
		return "", func() {}, err
	}
	zipReader, err := zip.OpenReader(zipPath)
	if err != nil {
		_ = os.Remove(zipPath)
		return "", func() {}, err
	}
	defer zipReader.Close()
	parent, err := os.MkdirTemp("", "knack-src-*")
	if err != nil {
		_ = os.Remove(zipPath)
		return "", func() {}, err
	}
	cleanup := func() {
		_ = os.RemoveAll(parent)
		_ = os.Remove(zipPath)
	}
	if err := extractZip(&zipReader.Reader, parent); err != nil {
		cleanup()
		return "", func() {}, err
	}
	root := resolveExtractedRoot(parent)
	return root, cleanup, nil
}

func extractZip(reader *zip.Reader, dest string) error {
	for _, file := range reader.File {
		clean := filepath.Clean(file.Name)
		if strings.HasPrefix(clean, "..") || filepath.IsAbs(clean) {
			return fmt.Errorf("invalid zip entry: %s", file.Name)
		}
		target := filepath.Join(dest, clean)
		if file.FileInfo().IsDir() {
			if err := os.MkdirAll(target, file.Mode()); err != nil {
				return err
			}
			continue
		}
		if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
			return err
		}
		src, err := file.Open()
		if err != nil {
			return err
		}
		out, err := os.OpenFile(target, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, file.Mode())
		if err != nil {
			src.Close()
			return err
		}
		if _, err := io.Copy(out, src); err != nil {
			out.Close()
			src.Close()
			return err
		}
		out.Close()
		src.Close()
	}
	return nil
}

func resolveExtractedRoot(parent string) string {
	entries, err := os.ReadDir(parent)
	if err != nil || len(entries) != 1 {
		return parent
	}
	entry := entries[0]
	if entry.IsDir() {
		return filepath.Join(parent, entry.Name())
	}
	return parent
}
