package terminal

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestCheckReturnsNilForBackgroundContext(t *testing.T) {
	if err := Check(context.Background()); err != nil {
		t.Fatalf("Check returned error: %v", err)
	}
}

func TestCheckReturnsInterruptedForCanceledContext(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if err := Check(ctx); !errors.Is(err, ErrInterrupted) {
		t.Fatalf("expected ErrInterrupted, got %v", err)
	}
}

func TestSleepContextReturnsInterruptedWhenCanceled(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	started := time.Now()
	if err := SleepContext(ctx, time.Second); !errors.Is(err, ErrInterrupted) {
		t.Fatalf("expected ErrInterrupted, got %v", err)
	}
	if time.Since(started) > 100*time.Millisecond {
		t.Fatal("SleepContext should return quickly after cancellation")
	}
}

func TestSleepContextWaitsWithoutCancellation(t *testing.T) {
	started := time.Now()
	if err := SleepContext(context.Background(), 20*time.Millisecond); err != nil {
		t.Fatalf("SleepContext returned error: %v", err)
	}
	if time.Since(started) < 15*time.Millisecond {
		t.Fatal("SleepContext returned too early")
	}
}
