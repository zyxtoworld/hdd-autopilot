package scratch

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"hdd/internal/auth"
	"hdd/internal/client"
	"hdd/internal/model"
	"hdd/internal/terminal"
)

func newScratchTestServer(t *testing.T, handler func(http.ResponseWriter, *http.Request)) *httptest.Server {
	t.Helper()
	return httptest.NewServer(http.HandlerFunc(handler))
}

func TestEnsureAuthenticatedUsesCachedToken(t *testing.T) {
	state := &BatchState{}
	server := newScratchTestServer(t, func(w http.ResponseWriter, req *http.Request) {
		if req.URL.String() != "/api/v1/auth/me?timezone=Asia%2FShanghai" {
			t.Fatalf("unexpected url: %s", req.URL.String())
		}
		if got := req.Header.Get("Authorization"); got != "Bearer token" {
			t.Fatalf("authorization = %q, want Bearer token", got)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"email":"demo@example.com","balance":0,"status":"active"}}`))
	})
	defer server.Close()

	runtime := &AccountRuntime{
		APIClient: client.New(server.URL),
		Account: auth.AuthCache{
			Email:       "demo@example.com",
			TokenType:   "Bearer",
			AccessToken: "token",
		},
	}
	if err := EnsureAuthenticated(state, runtime); err != nil {
		t.Fatalf("EnsureAuthenticated failed: %v", err)
	}
	if runtime.AuthToken != "Bearer token" {
		t.Fatalf("authToken = %q, want Bearer token", runtime.AuthToken)
	}
}

func TestEnsureAuthenticatedFallsBackToPasswordLogin(t *testing.T) {
	tempDir := t.TempDir()
	authFile := filepath.Join(tempDir, "auth.json")
	state := &BatchState{AuthCacheFile: authFile}

	server := newScratchTestServer(t, func(w http.ResponseWriter, req *http.Request) {
		switch req.URL.String() {
		case "/api/v1/auth/me?timezone=Asia%2FShanghai":
			w.WriteHeader(http.StatusUnauthorized)
			_, _ = w.Write([]byte(`{"message":"unauthorized"}`))
		case "/api/v1/auth/login":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"access_token":"fresh-token","token_type":"Bearer","user":{"email":"demo@example.com"}}}`))
		default:
			t.Fatalf("unexpected url: %s", req.URL.String())
		}
	})
	defer server.Close()

	runtime := &AccountRuntime{
		APIClient: client.New(server.URL),
		Account: auth.AuthCache{
			Email:       "demo@example.com",
			Password:    "secret",
			TokenType:   "Bearer",
			AccessToken: "stale-token",
		},
	}
	if err := EnsureAuthenticated(state, runtime); err != nil {
		t.Fatalf("EnsureAuthenticated failed: %v", err)
	}
	if runtime.AuthToken != "Bearer fresh-token" {
		t.Fatalf("authToken = %q, want Bearer fresh-token", runtime.AuthToken)
	}
	if len(state.Config.Accounts) != 1 || state.Config.Accounts[0].AccessToken != "fresh-token" {
		t.Fatalf("updated state = %+v, want persisted refreshed token", state.Config)
	}
}

func TestAddRoundTotalsUsesRevealHistoryWhenPresent(t *testing.T) {
	cost, reward := AddRoundTotals(model.ScratchRoundResult{
		PlayResp:          &model.ScratchPlayResponse{CostAmount: 1.5},
		RevealResp:        &model.ScratchRevealResponse{RewardAmount: 2.0},
		RevealHistoryItem: &model.ScratchHistoryItem{RewardAmount: 3.0},
	}, 0, 0)
	if cost != 1.5 {
		t.Fatalf("cost = %v, want 1.5", cost)
	}
	if reward != 3.0 {
		t.Fatalf("reward = %v, want 3.0", reward)
	}
}

func TestNewAccountRuntimesCreatesOnePerAccount(t *testing.T) {
	runtimes := NewAccountRuntimes([]auth.AuthCache{{Email: "a@example.com"}, {Email: "b@example.com"}}, "https://sub.hdd.sb")
	if len(runtimes) != 2 {
		t.Fatalf("len = %d, want 2", len(runtimes))
	}
	if runtimes[0].Email() != "a@example.com" || runtimes[1].Email() != "b@example.com" {
		t.Fatalf("unexpected emails: %s %s", runtimes[0].Email(), runtimes[1].Email())
	}
}

func TestRunAccountRoundPrintsPerAccountOutput(t *testing.T) {
	state := &BatchState{}
	var lines []string
	server := newScratchTestServer(t, func(w http.ResponseWriter, req *http.Request) {
		switch req.URL.String() {
		case "/api/v1/auth/me?timezone=Asia%2FShanghai":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"email":"demo@example.com","balance":0,"status":"active"}}`))
		case "/scratch-api/play":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"balance":10.0,"cost_amount":1.0,"earliest_reveal_at_ms":0,"game_type":"lucky-numbers","issued_at_ms":0,"min_scratch_ms":0,"play_id":1,"reveal_token":"token","status":"pending","ticket_payload":{"layout":"","title":"","subtitle":"","lucky_numbers":[1],"numbers":[{"matched":true,"prize_label":"1元","value":1}],"reward_amount":1.0,"reward_label":"1元"}}`))
		case "/scratch-api/history":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"items":[{"id":1,"cost_amount":1.0,"reward_amount":1.0,"net_amount":0.0,"status":"done"}]}`))
		case "/scratch-api/reveal":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"balance":11.0,"game_type":"lucky-numbers","net_amount":0.0,"play_id":1,"reward_amount":1.0,"status":"revealed","ticket_payload":{"layout":"","title":"","subtitle":"","lucky_numbers":[1],"numbers":[{"matched":true,"prize_label":"1元","value":1}],"reward_amount":1.0,"reward_label":"1元"}}`))
		default:
			t.Fatalf("unexpected url: %s", req.URL.String())
		}
	})
	defer server.Close()

	runtime := &AccountRuntime{
		APIClient: client.New(server.URL),
		Account:       auth.AuthCache{Email: "demo@example.com", TokenType: "Bearer", AccessToken: "token"},
		RevealLimiter: NewRevealLimiter(0),
	}
	RunAccountRound(state, runtime, RunOptions{HistoryRetries: 1}, func(format string, args ...any) {
		lines = append(lines, fmt.Sprintf(format, args...))
	})
	output := strings.Join(lines, "")
	if !strings.Contains(output, "当前账号：demo@example.com") {
		t.Fatalf("expected per-account header, got %q", output)
	}
	if !strings.Contains(output, "这一轮已经开局") {
		t.Fatalf("expected round output, got %q", output)
	}
}

func TestSaveAccountPersistsConfig(t *testing.T) {
	tempDir := t.TempDir()
	authFile := filepath.Join(tempDir, "auth.json")
	state := &BatchState{AuthCacheFile: authFile}
	if err := state.SaveAccount(auth.AuthCache{Email: "demo@example.com", Password: "pw", TokenType: "Bearer", AccessToken: "token"}); err != nil {
		t.Fatalf("SaveAccount failed: %v", err)
	}
	content, err := os.ReadFile(authFile)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	var cfg auth.AuthConfig
	if err := json.Unmarshal(content, &cfg); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if len(cfg.Accounts) != 1 || cfg.Accounts[0].Email != "demo@example.com" {
		t.Fatalf("unexpected config: %+v", cfg)
	}
}

func TestWaitForNextRoundDoesNotSleepFirstRound(t *testing.T) {
	started := time.Now()
	waitForNextRound(1, 20*time.Millisecond)
	if time.Since(started) > 10*time.Millisecond {
		t.Fatalf("first round should not sleep")
	}
}

func TestRunAccountRoundWithContextReturnsInterruptedBeforeLogin(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	state := &BatchState{}
	runtime := &AccountRuntime{Account: auth.AuthCache{Email: "demo@example.com"}}
	var lines []string
	err := RunAccountRoundWithContext(ctx, state, runtime, RunOptions{}, "", func(format string, args ...any) {
		lines = append(lines, fmt.Sprintf(format, args...))
	})
	if !errors.Is(err, terminal.ErrInterrupted) {
		t.Fatalf("expected ErrInterrupted, got %v", err)
	}
	if len(lines) != 0 {
		t.Fatalf("expected no output after interruption, got %q", strings.Join(lines, ""))
	}
}

func TestRunAccountRoundWritesPerAccountLog(t *testing.T) {
	state := &BatchState{}
	logDir := t.TempDir()
	server := newScratchTestServer(t, func(w http.ResponseWriter, req *http.Request) {
		switch req.URL.String() {
		case "/api/v1/auth/me?timezone=Asia%2FShanghai":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"email":"demo@example.com","balance":0,"status":"active"}}`))
		case "/scratch-api/play":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"balance":10.0,"cost_amount":1.0,"earliest_reveal_at_ms":0,"game_type":"lucky-numbers","issued_at_ms":0,"min_scratch_ms":0,"play_id":1,"reveal_token":"token","status":"pending","ticket_payload":{"layout":"","title":"","subtitle":"","lucky_numbers":[1],"numbers":[{"matched":true,"prize_label":"1元","value":1}],"reward_amount":1.0,"reward_label":"1元"}}`))
		case "/scratch-api/history":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"items":[{"id":1,"cost_amount":1.0,"reward_amount":1.0,"net_amount":0.0,"status":"done"}]}`))
		case "/scratch-api/reveal":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"balance":11.0,"game_type":"lucky-numbers","net_amount":0.0,"play_id":1,"reward_amount":1.0,"status":"revealed","ticket_payload":{"layout":"","title":"","subtitle":"","lucky_numbers":[1],"numbers":[{"matched":true,"prize_label":"1元","value":1}],"reward_amount":1.0,"reward_label":"1元"}}`))
		default:
			t.Fatalf("unexpected url: %s", req.URL.String())
		}
	})
	defer server.Close()

	runtime := &AccountRuntime{
		APIClient:      client.New(server.URL),
		Account:        auth.AuthCache{Email: "demo@example.com", TokenType: "Bearer", AccessToken: "token"},
		RevealLimiter:  NewRevealLimiter(0),
	}
	defer runtime.APIClient.CloseIdleConnections()

	err := RunAccountRoundWithContext(context.Background(), state, runtime, RunOptions{HistoryRetries: 1}, logDir, func(format string, args ...any) {})
	if err != nil {
		t.Fatalf("RunAccountRoundWithContext failed: %v", err)
	}
	content, err := os.ReadFile(filepath.Join(logDir, "demo_at_example.com.log"))
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	text := string(content)
	if !strings.Contains(text, "第 1 轮结束") || !strings.Contains(text, "累计情况") {
		t.Fatalf("unexpected log content: %q", text)
	}
}

func TestFetchScratchHistoryItemWithRetryWithContextReturnsInterruptedWhileWaiting(t *testing.T) {
	server := newScratchTestServer(t, func(w http.ResponseWriter, req *http.Request) {
		if req.URL.String() != "/scratch-api/history" {
			t.Fatalf("unexpected url: %s", req.URL.String())
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"items":[]}`))
	})
	defer server.Close()

	apiClient := client.New(server.URL)
	defer apiClient.CloseIdleConnections()
	ctx, cancel := context.WithCancel(context.Background())
	go func() {
		time.Sleep(10 * time.Millisecond)
		cancel()
	}()

	_, attempts, err := FetchScratchHistoryItemWithRetryWithContext(ctx, apiClient, "", 1, 3, 200*time.Millisecond, nil)
	if !errors.Is(err, terminal.ErrInterrupted) {
		t.Fatalf("expected ErrInterrupted, got %v", err)
	}
	if attempts != 1 {
		t.Fatalf("expected interruption on first retry wait, got attempts=%d", attempts)
	}
}
