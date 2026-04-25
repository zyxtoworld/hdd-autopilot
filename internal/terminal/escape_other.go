//go:build !windows

package terminal

import "context"

func StartEscapeWatcher(prompt string) (context.Context, func(), error) {
	ctx, cancel := context.WithCancel(context.Background())
	return ctx, cancel, nil
}
