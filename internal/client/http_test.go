package client

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"hdd/internal/model"
)

func TestAPIClientExportsAndLoadsSessionCookies(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/api/v1/auth/login":
			http.SetCookie(w, &http.Cookie{Name: "session", Value: "cookie-123", Path: "/"})
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"code":0,"message":"ok","reason":"","data":{"access_token":"token-123","token_type":"Bearer","user":{"email":"demo@example.com"}}}`))
		case "/api/v1/auth/me":
			if !strings.Contains(r.Header.Get("Cookie"), "session=cookie-123") {
				w.WriteHeader(http.StatusUnauthorized)
				_, _ = w.Write([]byte(`{"message":"unauthorized"}`))
				return
			}
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"email":"demo@example.com","balance":0,"status":"active"}}`))
		default:
			w.WriteHeader(http.StatusNotFound)
		}
	}))
	defer server.Close()

	first := New(server.URL)
	if _, _, err := first.DoLogin("demo@example.com", "pw"); err != nil {
		t.Fatalf("DoLogin failed: %v", err)
	}
	cookies := first.ExportSessionCookies()
	if len(cookies) == 0 || cookies[0].Name != "session" {
		t.Fatalf("expected exported session cookie, got %+v", cookies)
	}

	second := New(server.URL)
	if err := second.LoadSessionCookies(cookies); err != nil {
		t.Fatalf("LoadSessionCookies failed: %v", err)
	}
	resp, err := second.ValidateAuthToken("")
	if err != nil {
		t.Fatalf("ValidateAuthToken with cookies failed: %v", err)
	}
	if resp.Data.Email != "demo@example.com" {
		t.Fatalf("expected demo@example.com, got %q", resp.Data.Email)
	}
}

func TestAPIClientClearSessionCookiesDropsCookieOnlyAuth(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/api/v1/auth/me" {
			w.WriteHeader(http.StatusNotFound)
			return
		}
		if !strings.Contains(r.Header.Get("Cookie"), "session=keep-me") {
			w.WriteHeader(http.StatusUnauthorized)
			_, _ = w.Write([]byte(`{"message":"unauthorized"}`))
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"code":0,"message":"ok","data":{"email":"demo@example.com","balance":0,"status":"active"}}`))
	}))
	defer server.Close()

	apiClient := New(server.URL)
	if err := apiClient.LoadSessionCookies([]model.SessionCookie{{Name: "session", Value: "keep-me", Path: "/"}}); err != nil {
		t.Fatalf("LoadSessionCookies failed: %v", err)
	}
	if _, err := apiClient.ValidateAuthToken(""); err != nil {
		t.Fatalf("expected cookie auth to work before clear: %v", err)
	}

	apiClient.ClearSessionCookies()
	if _, err := apiClient.ValidateAuthToken(""); err == nil {
		t.Fatal("expected cookie auth to fail after clear")
	}
}
