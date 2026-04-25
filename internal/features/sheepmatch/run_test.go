package sheepmatch

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"hdd/internal/auth"
	"hdd/internal/client"
	"hdd/internal/logging"
	"hdd/internal/model"
)

func TestGetRemainingPlaysUsesTileMeRemaining(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/tile-api/me" {
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"active_session":null,"authenticated":true,"daily_plays_remaining":{"easy":3,"normal":1},"daily_plays_used":{"easy":7,"normal":9},"server_now_ms":1777006766099,"user":{"balance":12.34,"email":"demo@example.com","id":1,"status":"active"}}`))
	}))
	defer server.Close()

	apiClient := client.New(server.URL)
	authState := newRuntimeAuthState(auth.AuthCache{Email: "demo@example.com"}, "Bearer token", nil)
	remaining, err := getRemainingPlays(apiClient, authState, model.DifficultyEasy)
	if err != nil {
		t.Fatalf("getRemainingPlays failed: %v", err)
	}
	if remaining != 3 {
		t.Fatalf("expected remaining plays 3, got %d", remaining)
	}
}

func TestLocalizedDifficultyList(t *testing.T) {
	got := localizedDifficultyList([]string{model.DifficultyEasy, model.DifficultyNormal, model.DifficultyHard, model.DifficultyHell})
	if got != "简单、普通、困难、地狱" {
		t.Fatalf("unexpected difficulty list: %s", got)
	}
}

func TestResolveBaseURLPrefersFlagThenCachedValue(t *testing.T) {
	if got := resolveBaseURL(" https://flag.example.com/ ", "https://cached.example.com/"); got != "https://flag.example.com" {
		t.Fatalf("expected flag baseURL, got %q", got)
	}
	if got := resolveBaseURL("", " https://cached.example.com/ "); got != "https://cached.example.com" {
		t.Fatalf("expected cached baseURL, got %q", got)
	}
}

func TestSummarizeRoundsByDifficultyCarriesBalance(t *testing.T) {
	balance := 12.34
	rounds := []model.RoundResultSummary{{
		Email:          "demo@example.com",
		Difficulty:     model.DifficultyEasy,
		Status:         "won",
		Reward:         1.23,
		BalanceAfter:   &balance,
		RemainingAfter: 2,
		When:           time.Now(),
	}}
	stats := summarizeRoundsByDifficulty("demo@example.com", rounds)
	summary := stats[model.DifficultyEasy]
	if summary.BalanceAfter == nil || *summary.BalanceAfter != balance {
		t.Fatalf("expected balance %.2f, got %+v", balance, summary.BalanceAfter)
	}
}

func TestRoundStatusLabelUsesChinese(t *testing.T) {
	result := model.RoundResultSummary{Status: "won"}
	if got := roundStatusLabel(result); got != "成功通关" {
		t.Fatalf("unexpected status label: %s", got)
	}
}

func TestPerAccountLogFilePathUsesSanitizedEmail(t *testing.T) {
	path := logging.LogFilePath(`E:\项目\hdd\log\sheep-match`, "demo.user@example.com")
	if !strings.HasSuffix(path, `demo.user_at_example.com.log`) {
		t.Fatalf("unexpected log path: %s", path)
	}
}

func TestAccountCacheSaverPersistsUpdatedAccount(t *testing.T) {
	tempDir := t.TempDir()
	authFile := filepath.Join(tempDir, "auth.json")
	initial := auth.AuthConfig{
		SelectedEmail: "demo@example.com",
		Accounts: []model.AuthCache{{
			Email:       "demo@example.com",
			Password:    "old-pass",
			TokenType:   "Bearer",
			AccessToken: "old-token",
		}},
	}
	saver := newAccountCacheSaver(authFile, initial)
	updated := model.AuthCache{
		Email:       "demo@example.com",
		Password:    "old-pass",
		TokenType:   "Bearer",
		AccessToken: "new-token",
	}
	if err := saver.SaveAccount(updated); err != nil {
		t.Fatalf("SaveAccount failed: %v", err)
	}
	loaded, err := auth.LoadCache(authFile)
	if err != nil {
		t.Fatalf("LoadCache failed: %v", err)
	}
	if len(loaded.Accounts) != 1 {
		t.Fatalf("expected 1 account, got %d", len(loaded.Accounts))
	}
	if loaded.Accounts[0].AccessToken != "new-token" {
		t.Fatalf("expected token to be persisted, got %q", loaded.Accounts[0].AccessToken)
	}
}

func TestSnapshotFromHistoryItemUsesExplicitSlotTiles(t *testing.T) {
	item := &model.HistoryItem{
		Difficulty: model.DifficultyHard,
		SessionID:  1567,
		SlotLimit:  7,
		MoveCount:  25,
		Status:     "pending",
		Slots:      []int{20},
		SlotTiles: []model.Tile{{
			ID: 20, GX: 3, GY: 0, Layer: 1, Pattern: "P4",
		}},
		Tiles: []model.Tile{
			{ID: 23, GX: 3, GY: 2, Layer: 1, Pattern: "P5"},
			{ID: 24, GX: 5, GY: 2, Layer: 1, Pattern: "P1"},
		},
	}

	snapshot := snapshotFromHistoryItem(item)
	if len(snapshot.SlotTiles) != 1 || snapshot.SlotTiles[0].ID != 20 {
		t.Fatalf("expected slot tiles [20], got %+v", snapshot.SlotTiles)
	}
	if len(snapshot.Tiles) != 2 || snapshot.Tiles[0].ID != 23 || snapshot.Tiles[1].ID != 24 {
		t.Fatalf("expected board [23 24], got %+v", snapshot.Tiles)
	}
}

func TestApplyLocalClickSnapshotHandlesSlotFullByLocalReplay(t *testing.T) {
	snapshot := model.SessionSnapshot{
		Difficulty: model.DifficultyEasy,
		SessionID:  123,
		SlotLimit:  1,
		Status:     "pending",
		Tiles: []model.Tile{{
			ID: 1, Pattern: "A",
		}},
		SlotTiles: []model.Tile{{
			ID: 2, Pattern: "B",
		}},
	}
	updated, err := applyLocalClickSnapshot(snapshot, 1)
	if err != nil {
		t.Fatalf("applyLocalClickSnapshot failed: %v", err)
	}
	if len(updated.SlotTiles) != 2 {
		t.Fatalf("expected slot tiles to grow locally, got %+v", updated.SlotTiles)
	}
}

func TestEnsureAuthenticatedPrefersCookieSessionAcrossRestart(t *testing.T) {
	var loginCount int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/api/v1/auth/login":
			atomic.AddInt32(&loginCount, 1)
			http.SetCookie(w, &http.Cookie{Name: "session", Value: "cookie-ok", Path: "/"})
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"access_token":"token-login","token_type":"Bearer","user":{"email":"demo@example.com"}}}`))
		case "/api/v1/auth/me":
			cookieHeader := r.Header.Get("Cookie")
			authHeader := r.Header.Get("Authorization")
			if strings.Contains(cookieHeader, "session=cookie-ok") {
				w.Header().Set("Content-Type", "application/json")
				_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"email":"demo@example.com","balance":0,"status":"active"}}`))
				return
			}
			if authHeader == "Bearer token-login" {
				w.Header().Set("Content-Type", "application/json")
				_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"email":"demo@example.com","balance":0,"status":"active"}}`))
				return
			}
			w.WriteHeader(http.StatusUnauthorized)
			_, _ = w.Write([]byte(`{"message":"unauthorized"}`))
		default:
			w.WriteHeader(http.StatusNotFound)
		}
	}))
	defer server.Close()

	first := client.New(server.URL)
	loginResp, authToken, err := first.DoLogin("demo@example.com", "pw")
	if err != nil {
		t.Fatalf("DoLogin failed: %v", err)
	}
	if authToken != "Bearer token-login" {
		t.Fatalf("unexpected authToken: %q", authToken)
	}
	account := auth.CacheFromLogin(loginResp, "demo@example.com", "pw", first.BaseURL(), first.ExportSessionCookies())

	second := client.New(server.URL)
	returnedToken, updated, err := ensureAuthenticated(second, account)
	if err != nil {
		t.Fatalf("ensureAuthenticated failed: %v", err)
	}
	if returnedToken != "" {
		t.Fatalf("expected cookie session to return empty token, got %q", returnedToken)
	}
	if atomic.LoadInt32(&loginCount) != 1 {
		t.Fatalf("expected no extra login on restart, got %d logins", loginCount)
	}
	storedSession, ok := auth.GetSession(updated, second.BaseURL())
	if !ok || len(storedSession.Cookies) == 0 {
		t.Fatalf("expected scoped cookies to remain persisted, got %+v", updated)
	}
}

func TestRunDifficultyReauthenticatesAfterUnauthorizedStep(t *testing.T) {
	tempDir := t.TempDir()
	var loginCount int32
	var stepCount int32
	var meCount int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/api/v1/auth/me":
			cookie := r.Header.Get("Cookie")
			authHeader := r.Header.Get("Authorization")
			if strings.Contains(cookie, "session=fresh-1") || authHeader == "Bearer fresh-token-1" {
				w.Header().Set("Content-Type", "application/json")
				_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"email":"demo@example.com","balance":0,"status":"active"}}`))
				return
			}
			w.WriteHeader(http.StatusUnauthorized)
			_, _ = w.Write([]byte(`{"message":"unauthorized"}`))
		case "/api/v1/auth/login":
			count := atomic.AddInt32(&loginCount, 1)
			http.SetCookie(w, &http.Cookie{Name: "session", Value: fmt.Sprintf("fresh-%d", count), Path: "/"})
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(fmt.Sprintf(`{"code":0,"message":"ok","data":{"access_token":"fresh-token-%d","token_type":"Bearer","user":{"email":"demo@example.com"}}}`, count)))
		case "/tile-api/me":
			count := atomic.AddInt32(&meCount, 1)
			remaining := 1
			if count >= 2 {
				remaining = 0
			}
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(fmt.Sprintf(`{"active_session":null,"authenticated":true,"daily_plays_remaining":{"easy":%d},"daily_plays_used":{"easy":0},"server_now_ms":1777006766099,"user":{"balance":12.34,"email":"demo@example.com","id":1,"status":"active"}}`, remaining)))
		case "/tile-api/start":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"daily_plays_remaining":{"easy":1},"difficulty":"easy","history":[],"move_count":0,"ok":true,"pattern_count":1,"powerups":{"remove":0,"shuffle":0,"undo":0},"server_now_ms":1777006766099,"server_seed_hash":"seed","session_id":123,"slot_limit":7,"slots":[],"slot_tiles":[],"started_at_ms":1777006766000,"status":"pending","tiles":[{"id":1,"gx":0,"gy":0,"layer":0,"pattern":"A"},{"id":2,"gx":2,"gy":0,"layer":0,"pattern":"A"},{"id":3,"gx":4,"gy":0,"layer":0,"pattern":"A"}],"total_tiles":3}`))
		case "/tile-api/step":
			if atomic.AddInt32(&stepCount, 1) == 1 {
				w.WriteHeader(http.StatusUnauthorized)
				_, _ = w.Write([]byte(`{"message":"unauthorized"}`))
				return
			}
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"action":"click","balance":12.34,"grant_ref":"","history":[],"move_count":3,"ok":true,"powerups":{"remove":0,"shuffle":0,"undo":0},"removed":[1,2,3],"reward_amount":0.5,"schema_version":1,"server_now_ms":1777006767000,"session_id":123,"slot_limit":7,"slots":[],"started_at_ms":1777006766000,"status":"won","tiles":[],"total_tiles":0}`))
		default:
			w.WriteHeader(http.StatusNotFound)
		}
	}))
	defer server.Close()

	apiClient := client.New(server.URL)
	defer apiClient.CloseIdleConnections()
	saver := newAccountCacheSaver(filepath.Join(tempDir, "auth.json"), auth.AuthConfig{})
	authState := newRuntimeAuthState(auth.AuthCache{Email: "demo@example.com", Password: "pw"}, "Bearer stale-token", saver)
	output := captureStdout(t, func() {
		summary, rounds := runDifficultyWithContext(context.Background(), apiClient, authState, model.DifficultyEasy, false, model.AccountRunSummary{}, tempDir, nil, 1, 1)
		if len(rounds) != 1 || summary.Won != 1 {
			t.Fatalf("expected run to recover after re-login, got summary=%+v rounds=%+v", summary, rounds)
		}
	})
	if atomic.LoadInt32(&loginCount) != 1 {
		t.Fatalf("expected one re-login after unauthorized step, got %d", loginCount)
	}
	if !strings.Contains(output, "登录状态中途失效了，正在重新登录后继续") {
		t.Fatalf("expected re-login message, got %q", output)
	}
	if authState.authToken != "Bearer fresh-token-1" {
		t.Fatalf("expected refreshed auth token, got %q", authState.authToken)
	}
	session, ok := auth.GetSession(authState.account, apiClient.BaseURL())
	if !ok || len(session.Cookies) == 0 || session.Cookies[0].Value != "fresh-1" {
		t.Fatalf("expected refreshed cookies saved to auth state, got %+v", authState.account)
	}
}

func captureStdout(t *testing.T, fn func()) string {
	t.Helper()
	original := os.Stdout
	reader, writer, err := os.Pipe()
	if err != nil {
		t.Fatalf("Pipe failed: %v", err)
	}
	os.Stdout = writer
	defer func() {
		os.Stdout = original
	}()

	outputCh := make(chan string, 1)
	go func() {
		var buffer bytes.Buffer
		_, _ = io.Copy(&buffer, reader)
		outputCh <- buffer.String()
	}()

	fn()
	_ = writer.Close()
	return <-outputCh
}
