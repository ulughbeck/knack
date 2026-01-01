package cli

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"
)

func SelectMultiple(prompt string, options []string, defaultIndexes []int) ([]int, error) {
	reader := bufio.NewReader(os.Stdin)
	fmt.Fprintln(os.Stdout, prompt)
	for i, option := range options {
		fmt.Fprintf(os.Stdout, "  %d) %s\n", i+1, option)
	}
	if len(defaultIndexes) > 0 {
		fmt.Fprint(os.Stdout, "Enter numbers separated by comma (or press Enter for default): ")
	} else {
		fmt.Fprint(os.Stdout, "Enter numbers separated by comma: ")
	}
	line, err := reader.ReadString('\n')
	if err != nil && len(line) == 0 {
		return nil, err
	}
	line = strings.TrimSpace(line)
	if line == "" {
		return defaultIndexes, nil
	}
	if strings.EqualFold(line, "all") {
		indexes := make([]int, len(options))
		for i := range options {
			indexes[i] = i
		}
		return indexes, nil
	}
	parts := strings.Split(line, ",")
	var indexes []int
	seen := map[int]bool{}
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		idx, err := strconv.Atoi(part)
		if err != nil || idx < 1 || idx > len(options) {
			return nil, fmt.Errorf("invalid selection: %s", part)
		}
		if !seen[idx-1] {
			indexes = append(indexes, idx-1)
			seen[idx-1] = true
		}
	}
	return indexes, nil
}

func IsInteractive() bool {
	info, err := os.Stdin.Stat()
	if err != nil {
		return false
	}
	return (info.Mode() & os.ModeCharDevice) != 0
}
