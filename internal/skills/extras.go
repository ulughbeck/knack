package skills

import (
	"fmt"
	"path/filepath"
	"strings"
)

func CopyExtras(manifest Manifest, sourceRoot, destRoot string) error {
	for _, relPath := range manifest.ExtraPaths {
		clean, ok := sanitizeRelPath(relPath)
		if !ok {
			return fmt.Errorf("invalid extra path: %s", relPath)
		}
		src := filepath.Join(sourceRoot, clean)
		dest := filepath.Join(destRoot, clean)
		if err := CopyPath(src, dest); err != nil {
			return err
		}
	}
	return nil
}

func sanitizeRelPath(p string) (string, bool) {
	if p == "" {
		return "", false
	}
	if filepath.IsAbs(p) {
		return "", false
	}
	clean := filepath.Clean(p)
	if strings.HasPrefix(clean, "..") {
		return "", false
	}
	return clean, true
}
