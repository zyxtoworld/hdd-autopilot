package terminal

import (
	"context"
	"errors"
	"time"
)

var ErrInterrupted = errors.New("interrupted")

func Check(ctx context.Context) error {
	if ctx == nil {
		return nil
	}
	select {
	case <-ctx.Done():
		return ErrInterrupted
	default:
		return nil
	}
}

func SleepContext(ctx context.Context, wait time.Duration) error {
	if err := Check(ctx); err != nil {
		return err
	}
	if wait <= 0 {
		return nil
	}
	timer := time.NewTimer(wait)
	defer timer.Stop()
	select {
	case <-timer.C:
		return nil
	case <-ctx.Done():
		return ErrInterrupted
	}
}
