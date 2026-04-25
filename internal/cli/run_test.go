package cli

import (
	"bufio"
	"context"
	"errors"
	"strings"
	"testing"

	"hdd/internal/mining"
	"hdd/internal/terminal"
)

func TestPromptMainMenuChoiceAcceptsValidChoice(t *testing.T) {
	choice, err := promptMainMenuChoice(bufio.NewReader(strings.NewReader("2\n")))
	if err != nil {
		t.Fatalf("promptMainMenuChoice returned error: %v", err)
	}
	if choice != "2" {
		t.Fatalf("expected choice 2, got %q", choice)
	}
}

func TestPromptMainMenuChoiceRetriesAfterInvalidInput(t *testing.T) {
	choice, err := promptMainMenuChoice(bufio.NewReader(strings.NewReader("9\n1\n")))
	if err != nil {
		t.Fatalf("promptMainMenuChoice returned error: %v", err)
	}
	if choice != "1" {
		t.Fatalf("expected choice 1 after retry, got %q", choice)
	}
}

func TestPromptMiningMenuChoiceAcceptsValidChoice(t *testing.T) {
	choice, err := promptMiningMenuChoice(bufio.NewReader(strings.NewReader("4\n")))
	if err != nil {
		t.Fatalf("promptMiningMenuChoice returned error: %v", err)
	}
	if choice != "4" {
		t.Fatalf("expected choice 4, got %q", choice)
	}
}

func TestPromptMiningMenuChoiceRetriesAfterInvalidInput(t *testing.T) {
	choice, err := promptMiningMenuChoice(bufio.NewReader(strings.NewReader("9\n2\n")))
	if err != nil {
		t.Fatalf("promptMiningMenuChoice returned error: %v", err)
	}
	if choice != "2" {
		t.Fatalf("expected choice 2 after retry, got %q", choice)
	}
}

func TestPromptBatchMenuChoiceAcceptsValidChoice(t *testing.T) {
	choice, err := promptBatchMenuChoice(bufio.NewReader(strings.NewReader("4\n")))
	if err != nil {
		t.Fatalf("promptBatchMenuChoice returned error: %v", err)
	}
	if choice != "4" {
		t.Fatalf("expected choice 4, got %q", choice)
	}
}

func TestPromptFeatureMenuChoiceRetriesAfterInvalidInput(t *testing.T) {
	choice, err := promptFeatureMenuChoice(bufio.NewReader(strings.NewReader("8\n3\n")))
	if err != nil {
		t.Fatalf("promptFeatureMenuChoice returned error: %v", err)
	}
	if choice != "3" {
		t.Fatalf("expected choice 3 after retry, got %q", choice)
	}
}

func TestPromptFeatureMenuChoiceAcceptsSheepMatchChoice(t *testing.T) {
	choice, err := promptFeatureMenuChoice(bufio.NewReader(strings.NewReader("4\n")))
	if err != nil {
		t.Fatalf("promptFeatureMenuChoice returned error: %v", err)
	}
	if choice != "4" {
		t.Fatalf("expected choice 4, got %q", choice)
	}
}

func TestPromptMiningRuntimeChoiceAcceptsValidChoice(t *testing.T) {
	choice, err := promptMiningRuntimeChoice(bufio.NewReader(strings.NewReader("2\n")))
	if err != nil {
		t.Fatalf("promptMiningRuntimeChoice returned error: %v", err)
	}
	if choice != "2" {
		t.Fatalf("expected choice 2, got %q", choice)
	}
}

func TestPromptMiningRuntimeChoiceRetriesAfterInvalidInput(t *testing.T) {
	choice, err := promptMiningRuntimeChoice(bufio.NewReader(strings.NewReader("9\n1\n")))
	if err != nil {
		t.Fatalf("promptMiningRuntimeChoice returned error: %v", err)
	}
	if choice != "1" {
		t.Fatalf("expected choice 1 after retry, got %q", choice)
	}
}

func TestHandleInterruptedReturnsNilForInterruptedError(t *testing.T) {
	if err := handleInterrupted(terminal.ErrInterrupted); err != nil {
		t.Fatalf("expected nil for interrupted error, got %v", err)
	}
}

func TestHandleInterruptedPassesThroughOtherErrors(t *testing.T) {
	expected := errors.New("boom")
	if err := handleInterrupted(expected); !errors.Is(err, expected) {
		t.Fatalf("expected original error, got %v", err)
	}
}

func TestRunMiningModeStartsWatcherOnlyAfterRuntimeSelection(t *testing.T) {
	originalDetect := detectGPUAvailability
	originalWatcher := startEscapeWatcherHook
	originalRunGPU := runAutoTunedGPUWithContext
	defer func() {
		detectGPUAvailability = originalDetect
		startEscapeWatcherHook = originalWatcher
		runAutoTunedGPUWithContext = originalRunGPU
	}()

	detectGPUAvailability = func(mode mining.Mode) mining.GPUAvailability {
		return mining.GPUAvailability{Available: true}
	}
	watcherStarted := false
	startEscapeWatcherHook = func() (context.Context, func(), error) {
		watcherStarted = true
		return context.Background(), func() {}, nil
	}
	runAutoTunedGPUWithContext = func(ctx context.Context, mode mining.Mode) error {
		if !watcherStarted {
			t.Fatal("expected watcher to start before GPU mining begins")
		}
		return nil
	}

	exit, err := runMiningMode(bufio.NewReader(strings.NewReader("1\n")), mining.ModeInviteThenBalance)
	if err != nil {
		t.Fatalf("runMiningMode returned error: %v", err)
	}
	if exit {
		t.Fatal("expected to stay in script after runtime selection")
	}
	if !watcherStarted {
		t.Fatal("expected watcher to start after runtime selection")
	}
}

func TestRunMiningModeDoesNotStartWatcherWhenReturningFromRuntimeMenu(t *testing.T) {
	originalDetect := detectGPUAvailability
	originalWatcher := startEscapeWatcherHook
	defer func() {
		detectGPUAvailability = originalDetect
		startEscapeWatcherHook = originalWatcher
	}()

	detectGPUAvailability = func(mode mining.Mode) mining.GPUAvailability {
		return mining.GPUAvailability{Available: true}
	}
	watcherStarted := false
	startEscapeWatcherHook = func() (context.Context, func(), error) {
		watcherStarted = true
		return context.Background(), func() {}, nil
	}

	exit, err := runMiningMode(bufio.NewReader(strings.NewReader("3\n")), mining.ModeInviteThenBalance)
	if err != nil {
		t.Fatalf("runMiningMode returned error: %v", err)
	}
	if exit {
		t.Fatal("expected return to previous menu, not exit script")
	}
	if watcherStarted {
		t.Fatal("expected watcher not to start before runtime selection returns")
	}
}

func TestQueryAllBalancesStartsWatcherAndHandlesEmptyAccounts(t *testing.T) {
	originalWatcher := startEscapeWatcherHook
	defer func() {
		startEscapeWatcherHook = originalWatcher
	}()

	watcherStarted := false
	startEscapeWatcherHook = func() (context.Context, func(), error) {
		watcherStarted = true
		return context.Background(), func() {}, nil
	}

	err := queryAllBalances(&appState{})
	if err != nil {
		t.Fatalf("queryAllBalances returned error: %v", err)
	}
	if !watcherStarted {
		t.Fatal("expected watcher to start for balance query")
	}
}

func TestWaitForBatchReturnHandlesInterruptedContext(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if err := waitForBatchReturn(ctx, "全部账号签到完成。"); err != nil {
		t.Fatalf("waitForBatchReturn returned error: %v", err)
	}
}
