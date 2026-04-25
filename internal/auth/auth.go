package auth

import (
	"fmt"
	"net/url"
	"strings"

	"hdd/internal/model"
)

type AuthCache = model.AuthCache

type AuthConfig = model.AuthConfig

type AuthSession = model.AuthSession

type SessionCookie = model.SessionCookie

func NormalizeTokenType(tokenType string) string {
	tokenType = strings.TrimSpace(tokenType)
	if tokenType == "" || strings.EqualFold(tokenType, "bearer") {
		return "Bearer"
	}
	return tokenType
}

func NormalizeBaseURL(baseURL string) string {
	baseURL = strings.TrimSpace(baseURL)
	if baseURL == "" {
		return ""
	}
	parsed, err := url.Parse(baseURL)
	if err != nil || parsed.Scheme == "" || parsed.Host == "" {
		return strings.TrimRight(baseURL, "/")
	}
	parsed.Scheme = strings.ToLower(parsed.Scheme)
	parsed.Host = strings.ToLower(parsed.Host)
	parsed.Path = strings.TrimRight(parsed.Path, "/")
	parsed.RawQuery = ""
	parsed.Fragment = ""
	return strings.TrimRight(parsed.String(), "/")
}

func BuildAuthorization(tokenType string, accessToken string) string {
	accessToken = strings.TrimSpace(accessToken)
	if accessToken == "" {
		return ""
	}
	return NormalizeTokenType(tokenType) + " " + accessToken
}

func CacheUsable(cache model.AuthCache) bool {
	return BuildAuthorization(cache.TokenType, cache.AccessToken) != ""
}

func PasswordUsable(cache model.AuthCache) bool {
	return strings.TrimSpace(cache.Email) != "" && strings.TrimSpace(cache.Password) != ""
}

func CacheFromLogin(loginResp *model.LoginResponse, fallbackEmail string, password string, baseURL string, cookies []model.SessionCookie) model.AuthCache {
	email := strings.TrimSpace(loginResp.Data.User.Email)
	if email == "" {
		email = strings.TrimSpace(fallbackEmail)
	}
	session := model.AuthSession{
		BaseURL:     NormalizeBaseURL(baseURL),
		TokenType:   NormalizeTokenType(loginResp.Data.TokenType),
		AccessToken: strings.TrimSpace(loginResp.Data.AccessToken),
		Cookies:     normalizeCookies(cookies),
	}
	account := model.AuthCache{
		Email:       email,
		Password:    strings.TrimSpace(password),
		TokenType:   session.TokenType,
		AccessToken: session.AccessToken,
		Cookies:     session.Cookies,
	}
	if SessionUsable(session) {
		account.Sessions = []model.AuthSession{session}
	}
	return normalizeAccount(account)
}

func RequirePassword(account model.AuthCache) error {
	if PasswordUsable(account) {
		return nil
	}
	return fmt.Errorf("账号 %s 没有可用密码，没法重新登录", strings.TrimSpace(account.Email))
}
