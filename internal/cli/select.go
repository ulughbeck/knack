package cli

import (
	"fmt"
)

func SelectOne(prompt string, options []string, defaultIndex int) (int, error) {
	defaults := []int{}
	if defaultIndex >= 0 && defaultIndex < len(options) {
		defaults = []int{defaultIndex}
	}
	indexes, err := SelectMultiple(prompt, options, defaults)
	if err != nil {
		return -1, err
	}
	if len(indexes) == 0 {
		return -1, fmt.Errorf("no selection made")
	}
	if len(indexes) > 1 {
		return -1, fmt.Errorf("please select a single option")
	}
	return indexes[0], nil
}
