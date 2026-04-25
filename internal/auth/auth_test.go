package auth

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"

	"hdd/internal/model"
)

func TestLoadCacheSupportsLegacySingleAccountFormat(t *testing.T) {
	tempDir := t.TempDir()
	path := filepath.Join(tempDir, "auth.json")
	content := []byte(`{
  "email": "demo@example.com",
  "password": "pw",
  "token_type": "Bearer",
  "access_token": "legacy-token"
}`)
	if err := os.WriteFile(path, content, 0600); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	cfg, err := LoadCache(path)
	if err != nil {
		t.Fatalf("LoadCache failed: %v", err)
	}
	if len(cfg.Accounts) != 1 {
		t.Fatalf("expected 1 account, got %d", len(cfg.Accounts))
	}
	if cfg.Accounts[0].AccessToken != "legacy-token" {
		t.Fatalf("expected legacy token, got %q", cfg.Accounts[0].AccessToken)
	}
}

func TestSaveAndLoadCacheUsesTopLevelBaseURLAndFlattenedSession(t *testing.T) {
	tempDir := t.TempDir()
	path := filepath.Join(tempDir, "auth.json")
	cfg := model.AuthConfig{
		BaseURL:       "HTTPS://SUB.HDD.SB/",
		SelectedEmail: "demo@example.com",
		Accounts: []model.AuthCache{{
			Email:       "demo@example.com",
			Password:    "pw",
			TokenType:   "bearer",
			AccessToken: "token-a",
			Cookies: []model.SessionCookie{{
				Name:   "session",
				Value:  "cookie-a",
				Domain: "sub.hdd.sb",
				Path:   "/",
			}},
		}},
	}
	if err := SaveCache(path, cfg); err != nil {
		t.Fatalf("SaveCache failed: %v", err)
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	if string(raw) == "" {
		t.Fatalf("expected saved auth.json content")
	}
	if bytes.Contains(raw, []byte(`"sessions"`)) {
		t.Fatalf("expected saved auth.json to omit legacy sessions: %s", raw)
	}
	if !bytes.Contains(raw, []byte(`"token_type": "Bearer"`)) {
		t.Fatalf("expected saved auth.json to keep token_type: %s", raw)
	}

	loaded, err := LoadCache(path)
	if err != nil {
		t.Fatalf("LoadCache failed: %v", err)
	}
	if loaded.BaseURL != "https://sub.hdd.sb" {
		t.Fatalf("expected normalized top-level baseURL, got %q", loaded.BaseURL)
	}
	if len(loaded.Accounts) != 1 {
		t.Fatalf("expected one account, got %+v", loaded.Accounts)
	}
	account := loaded.Accounts[0]
	if len(account.Sessions) != 0 {
		t.Fatalf("expected flattened account without sessions, got %+v", account.Sessions)
	}
	if account.TokenType != "Bearer" || account.AccessToken != "token-a" {
		t.Fatalf("unexpected flattened token: %+v", account)
	}
	if len(account.Cookies) != 1 || account.Cookies[0].Value != "cookie-a" {
		t.Fatalf("unexpected flattened cookies: %+v", account.Cookies)
	}
	storedSession, ok := GetSession(account, loaded.BaseURL)
	if !ok {
		t.Fatalf("expected session reconstructed from flattened cache")
	}
	if storedSession.BaseURL != "https://sub.hdd.sb" {
		t.Fatalf("expected reconstructed baseURL, got %q", storedSession.BaseURL)
	}
}

func TestUpsertAccountKeepsTopLevelBaseURLWhenAddingFirstAccount(t *testing.T) {
	cfg := model.AuthConfig{}
	updated := model.AuthCache{
		Email:    "demo@example.com",
		Password: "pw",
		Sessions: []model.AuthSession{{
			BaseURL:     "https://staging.example.com/",
			TokenType:   "bearer",
			AccessToken: "staging-token-new",
			Cookies: []model.SessionCookie{{
				Name:  "session",
				Value: "cookie-new",
			}},
		}},
	}

	cfg = UpsertAccount(cfg, updated)
	if cfg.BaseURL != "https://staging.example.com" {
		t.Fatalf("expected baseURL to persist at top level, got %q", cfg.BaseURL)
	}
	if len(cfg.Accounts) != 1 {
		t.Fatalf("expected one account, got %+v", cfg.Accounts)
	}
	storedSession, ok := GetSession(cfg.Accounts[0], cfg.BaseURL)
	if !ok || storedSession.AccessToken != "staging-token-new" || len(storedSession.Cookies) != 1 {
		t.Fatalf("expected flattened session data to be preserved, got %+v", cfg.Accounts[0])
	}
}

func TestLoadCacheFlattensLegacySessionFormat(t *testing.T) {
	tempDir := t.TempDir()
	path := filepath.Join(tempDir, "auth.json")
	content := []byte(`{
  "selected_email": "demo@example.com",
  "accounts": [
    {
      "email": "demo@example.com",
      "password": "pw",
      "sessions": [
        {
          "base_url": "https://prod.example.com/",
          "token_type": "bearer",
          "access_token": "prod-token",
          "cookies": [
            {"name": "session", "value": "cookie-prod", "path": "/"}
          ]
        }
      ]
    }
  ]
}`)
	if err := os.WriteFile(path, content, 0600); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	cfg, err := LoadCache(path)
	if err != nil {
		t.Fatalf("LoadCache failed: %v", err)
	}
	if cfg.BaseURL != "https://prod.example.com" {
		t.Fatalf("expected top-level baseURL from legacy session, got %q", cfg.BaseURL)
	}
	if len(cfg.Accounts) != 1 {
		t.Fatalf("expected one account, got %+v", cfg.Accounts)
	}
	account := cfg.Accounts[0]
	if len(account.Sessions) != 0 {
		t.Fatalf("expected sessions to be flattened, got %+v", account.Sessions)
	}
	if account.AccessToken != "prod-token" || len(account.Cookies) != 1 || account.Cookies[0].Value != "cookie-prod" {
		t.Fatalf("expected flattened auth data, got %+v", account)
	}
}
