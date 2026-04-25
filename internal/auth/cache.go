package auth

import (
	"bytes"
	"encoding/json"
	"os"
	"strings"

	"hdd/internal/model"
)

func normalizeCookie(cookie model.SessionCookie) model.SessionCookie {
	cookie.Name = strings.TrimSpace(cookie.Name)
	cookie.Value = strings.TrimSpace(cookie.Value)
	cookie.Domain = strings.TrimSpace(cookie.Domain)
	cookie.Path = strings.TrimSpace(cookie.Path)
	cookie.ExpiresAt = strings.TrimSpace(cookie.ExpiresAt)
	return cookie
}

func normalizeCookies(cookies []model.SessionCookie) []model.SessionCookie {
	result := make([]model.SessionCookie, 0, len(cookies))
	seen := make(map[string]struct{}, len(cookies))
	for _, cookie := range cookies {
		cookie = normalizeCookie(cookie)
		if cookie.Name == "" || cookie.Value == "" {
			continue
		}
		key := cookie.Name + "\x00" + cookie.Domain + "\x00" + cookie.Path
		if _, ok := seen[key]; ok {
			continue
		}
		seen[key] = struct{}{}
		result = append(result, cookie)
	}
	return result
}

func SessionUsable(session model.AuthSession) bool {
	session = normalizeSession(session)
	return BuildAuthorization(session.TokenType, session.AccessToken) != "" || len(session.Cookies) > 0
}

func normalizeSession(session model.AuthSession) model.AuthSession {
	session.BaseURL = NormalizeBaseURL(session.BaseURL)
	session.TokenType = NormalizeTokenType(session.TokenType)
	session.AccessToken = strings.TrimSpace(session.AccessToken)
	session.Cookies = normalizeCookies(session.Cookies)
	return session
}

func normalizeSessions(sessions []model.AuthSession) []model.AuthSession {
	result := make([]model.AuthSession, 0, len(sessions))
	indexByBaseURL := make(map[string]int, len(sessions))
	for _, session := range sessions {
		session = normalizeSession(session)
		if session.BaseURL == "" || !SessionUsable(session) {
			continue
		}
		if idx, ok := indexByBaseURL[session.BaseURL]; ok {
			result[idx] = session
			continue
		}
		indexByBaseURL[session.BaseURL] = len(result)
		result = append(result, session)
	}
	return result
}

func normalizeAccount(account model.AuthCache) model.AuthCache {
	account.Email = strings.TrimSpace(account.Email)
	account.Password = strings.TrimSpace(account.Password)
	account.TokenType = NormalizeTokenType(account.TokenType)
	account.AccessToken = strings.TrimSpace(account.AccessToken)
	account.Cookies = normalizeCookies(account.Cookies)
	account.Sessions = normalizeSessions(account.Sessions)
	return account
}

func findLegacySession(account model.AuthCache, baseURL string) (model.AuthSession, bool) {
	account = normalizeAccount(account)
	if idx := FindSession(account.Sessions, baseURL); idx >= 0 {
		return account.Sessions[idx], true
	}
	if len(account.Sessions) > 0 {
		return account.Sessions[0], true
	}
	return model.AuthSession{}, false
}

func normalizeAccountForBaseURL(account model.AuthCache, baseURL string) model.AuthCache {
	account = normalizeAccount(account)
	if len(account.Cookies) == 0 || !CacheUsable(account) {
		if session, ok := findLegacySession(account, baseURL); ok {
			if account.TokenType == "" {
				account.TokenType = session.TokenType
			}
			if account.AccessToken == "" {
				account.AccessToken = session.AccessToken
			}
			if len(account.Cookies) == 0 {
				account.Cookies = session.Cookies
			}
		}
	}
	account.Cookies = normalizeCookies(account.Cookies)
	account.Sessions = nil
	return normalizeAccount(account)
}

func normalizeAccounts(accounts []model.AuthCache) []model.AuthCache {
	return normalizeAccountsForBaseURL(accounts, "")
}

func normalizeAccountsForBaseURL(accounts []model.AuthCache, baseURL string) []model.AuthCache {
	result := make([]model.AuthCache, 0, len(accounts))
	for _, account := range accounts {
		account = normalizeAccountForBaseURL(account, baseURL)
		if account.Email == "" {
			continue
		}
		result = append(result, account)
	}
	return result
}

func normalizeConfig(config model.AuthConfig) model.AuthConfig {
	config.BaseURL = NormalizeBaseURL(config.BaseURL)
	if config.BaseURL == "" {
		for _, account := range config.Accounts {
			if session, ok := findLegacySession(account, ""); ok {
				config.BaseURL = session.BaseURL
				break
			}
		}
	}
	config.Accounts = normalizeAccountsForBaseURL(config.Accounts, config.BaseURL)
	if config.SelectedEmail == "" && len(config.Accounts) > 0 {
		config.SelectedEmail = config.Accounts[0].Email
	}
	return config
}

func FindAccount(accounts []model.AuthCache, email string) int {
	email = strings.TrimSpace(email)
	for i, account := range accounts {
		if strings.EqualFold(strings.TrimSpace(account.Email), email) {
			return i
		}
	}
	return -1
}

func FindSession(sessions []model.AuthSession, baseURL string) int {
	baseURL = NormalizeBaseURL(baseURL)
	for i, session := range sessions {
		if NormalizeBaseURL(session.BaseURL) == baseURL {
			return i
		}
	}
	return -1
}

func GetSession(account model.AuthCache, baseURL string) (model.AuthSession, bool) {
	account = normalizeAccountForBaseURL(account, baseURL)
	if !CacheUsable(account) && len(account.Cookies) == 0 {
		return model.AuthSession{}, false
	}
	return model.AuthSession{
		BaseURL:     NormalizeBaseURL(baseURL),
		TokenType:   account.TokenType,
		AccessToken: account.AccessToken,
		Cookies:     account.Cookies,
	}, true
}

func UpsertSession(account model.AuthCache, session model.AuthSession) model.AuthCache {
	account = normalizeAccount(account)
	session = normalizeSession(session)
	if !SessionUsable(session) {
		account.TokenType = ""
		account.AccessToken = ""
		account.Cookies = nil
		account.Sessions = nil
		return normalizeAccount(account)
	}
	account.TokenType = session.TokenType
	account.AccessToken = session.AccessToken
	account.Cookies = session.Cookies
	account.Sessions = nil
	return normalizeAccount(account)
}

func UpsertAccount(config model.AuthConfig, account model.AuthCache) model.AuthConfig {
	if config.BaseURL == "" {
		if session, ok := findLegacySession(account, ""); ok {
			config.BaseURL = session.BaseURL
		}
	}
	config = normalizeConfig(config)
	account = normalizeAccountForBaseURL(account, config.BaseURL)
	if account.Email == "" {
		return config
	}
	idx := FindAccount(config.Accounts, account.Email)
	if idx >= 0 {
		merged := config.Accounts[idx]
		merged.Email = account.Email
		if account.Password != "" {
			merged.Password = account.Password
		}
		if CacheUsable(account) {
			merged.TokenType = account.TokenType
			merged.AccessToken = account.AccessToken
		}
		if len(account.Cookies) > 0 {
			merged.Cookies = account.Cookies
		}
		config.Accounts[idx] = normalizeAccountForBaseURL(merged, config.BaseURL)
	} else {
		config.Accounts = append(config.Accounts, account)
	}
	config.SelectedEmail = account.Email
	return normalizeConfig(config)
}

func LoadCache(path string) (model.AuthConfig, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return model.AuthConfig{}, nil
		}
		return model.AuthConfig{}, err
	}
	if len(bytes.TrimSpace(content)) == 0 {
		return model.AuthConfig{}, nil
	}

	var config model.AuthConfig
	if err := json.Unmarshal(content, &config); err == nil {
		config = normalizeConfig(config)
		if len(config.Accounts) > 0 {
			return config, nil
		}
	}

	var cache model.AuthCache
	if err := json.Unmarshal(content, &cache); err != nil {
		return model.AuthConfig{}, err
	}
	cache = normalizeAccount(cache)
	baseURL := ""
	if session, ok := findLegacySession(cache, ""); ok {
		baseURL = session.BaseURL
	}
	cache = normalizeAccountForBaseURL(cache, baseURL)
	if cache.Email == "" {
		return model.AuthConfig{}, nil
	}
	return normalizeConfig(model.AuthConfig{BaseURL: baseURL, SelectedEmail: cache.Email, Accounts: []model.AuthCache{cache}}), nil
}

func compactTokenTypeForSave(tokenType string) string {
	return NormalizeTokenType(tokenType)
}

func compactAccountForSave(account model.AuthCache, baseURL string) model.AuthCache {
	account = normalizeAccountForBaseURL(account, baseURL)
	account.TokenType = compactTokenTypeForSave(account.TokenType)
	account.Sessions = nil
	if len(account.Cookies) == 0 {
		account.Cookies = nil
	}
	return account
}

func compactConfigForSave(config model.AuthConfig) model.AuthConfig {
	config = normalizeConfig(config)
	config.Accounts = normalizeAccountsForBaseURL(config.Accounts, config.BaseURL)
	for i, account := range config.Accounts {
		config.Accounts[i] = compactAccountForSave(account, config.BaseURL)
	}
	return config
}

func SaveCache(path string, config model.AuthConfig) error {
	config = compactConfigForSave(config)
	content, err := json.MarshalIndent(config, "", "  ")
	if err != nil {
		return err
	}
	content = append(content, '\n')
	return os.WriteFile(path, content, 0600)
}
