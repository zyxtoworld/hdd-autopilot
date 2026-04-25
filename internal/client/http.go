package client

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/cookiejar"
	"net/url"
	"strings"
	"time"

	"hdd/internal/auth"
	"hdd/internal/model"
)

const (
	defaultBaseURL = "https://sub.hdd.sb"
	authMePath     = "/api/v1/auth/me?timezone=Asia%2FShanghai"
	loginPath      = "/api/v1/auth/login"
)

var ErrUnauthorized = errors.New("unauthorized")

type statusErrorFormatter func(statusCode int, body []byte) error

type statusError struct {
	statusCode int
	body       []byte
	err        error
}

type apiErrorBody struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
	Reason  string `json:"reason"`
}

type unauthorizedError struct {
	cause error
}

type APIClient struct {
	baseURL    string
	baseURLURL *url.URL
	httpClient *http.Client
}

func (e *statusError) Error() string {
	return e.err.Error()
}

func (e *statusError) Unwrap() error {
	return e.err
}

func (e *unauthorizedError) Error() string {
	return e.cause.Error()
}

func (e *unauthorizedError) Unwrap() error {
	return ErrUnauthorized
}

func NewHTTPClient() *http.Client {
	transport := &http.Transport{
		Proxy: http.ProxyFromEnvironment,
		DialContext: (&net.Dialer{
			Timeout:   10 * time.Second,
			KeepAlive: 30 * time.Second,
		}).DialContext,
		ForceAttemptHTTP2:     true,
		MaxIdleConns:          8,
		MaxIdleConnsPerHost:   8,
		IdleConnTimeout:       90 * time.Second,
		TLSHandshakeTimeout:   10 * time.Second,
		ExpectContinueTimeout: time.Second,
	}
	jar, _ := cookiejar.New(nil)
	return &http.Client{
		Timeout:   30 * time.Second,
		Transport: transport,
		Jar:       jar,
	}
}

func New(baseURL string) *APIClient {
	baseURL = strings.TrimSpace(baseURL)
	if baseURL == "" {
		baseURL = defaultBaseURL
	}
	normalized := strings.TrimRight(baseURL, "/")
	parsed, _ := url.Parse(normalized)
	return &APIClient{
		baseURL:    normalized,
		baseURLURL: parsed,
		httpClient: NewHTTPClient(),
	}
}

func (c *APIClient) CloseIdleConnections() {
	if c == nil || c.httpClient == nil {
		return
	}
	if transport, ok := c.httpClient.Transport.(*http.Transport); ok {
		transport.CloseIdleConnections()
	}
}

func (c *APIClient) BaseURL() string {
	return c.baseURL
}

func (c *APIClient) LoadSessionCookies(cookies []model.SessionCookie) error {
	if c == nil || c.httpClient == nil || c.httpClient.Jar == nil || c.baseURLURL == nil {
		return nil
	}
	jarCookies, err := cookiesToHTTPCookies(cookies)
	if err != nil {
		return err
	}
	c.httpClient.Jar.SetCookies(c.baseURLURL, jarCookies)
	return nil
}

func (c *APIClient) ExportSessionCookies() []model.SessionCookie {
	if c == nil || c.httpClient == nil || c.httpClient.Jar == nil || c.baseURLURL == nil {
		return nil
	}
	return httpCookiesToSession(c.httpClient.Jar.Cookies(c.baseURLURL))
}

func (c *APIClient) ClearSessionCookies() {
	if c == nil || c.httpClient == nil {
		return
	}
	jar, _ := cookiejar.New(nil)
	c.httpClient.Jar = jar
}

func cookiesToHTTPCookies(cookies []model.SessionCookie) ([]*http.Cookie, error) {
	result := make([]*http.Cookie, 0, len(cookies))
	for _, cookie := range cookies {
		httpCookie := &http.Cookie{
			Name:     strings.TrimSpace(cookie.Name),
			Value:    cookie.Value,
			Domain:   strings.TrimSpace(cookie.Domain),
			Path:     strings.TrimSpace(cookie.Path),
			Secure:   cookie.Secure,
			HttpOnly: cookie.HttpOnly,
		}
		if httpCookie.Name == "" || httpCookie.Value == "" {
			continue
		}
		if cookie.ExpiresAt != "" {
			expiresAt, err := time.Parse(time.RFC3339, cookie.ExpiresAt)
			if err != nil {
				return nil, err
			}
			httpCookie.Expires = expiresAt
		}
		result = append(result, httpCookie)
	}
	return result, nil
}

func httpCookiesToSession(cookies []*http.Cookie) []model.SessionCookie {
	result := make([]model.SessionCookie, 0, len(cookies))
	for _, cookie := range cookies {
		if cookie == nil || strings.TrimSpace(cookie.Name) == "" || cookie.Value == "" {
			continue
		}
		sessionCookie := model.SessionCookie{
			Name:     strings.TrimSpace(cookie.Name),
			Value:    cookie.Value,
			Domain:   strings.TrimSpace(cookie.Domain),
			Path:     strings.TrimSpace(cookie.Path),
			Secure:   cookie.Secure,
			HttpOnly: cookie.HttpOnly,
		}
		if !cookie.Expires.IsZero() {
			sessionCookie.ExpiresAt = cookie.Expires.UTC().Format(time.RFC3339)
		}
		result = append(result, sessionCookie)
	}
	return result
}

func setCommonHeaders(req *http.Request, authToken string, origin string, referer string) {
	if authToken != "" {
		req.Header.Set("Authorization", authToken)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json, text/plain, */*")
	req.Header.Set("Accept-Language", "zh")
	if origin != "" {
		req.Header.Set("Origin", origin)
	}
	if referer != "" {
		req.Header.Set("Referer", referer)
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 Edg/147.0.0.0")
}

func (c *APIClient) newJSONRequest(method string, path string, authToken string, referer string, payload any) (*http.Request, error) {
	var body io.Reader
	if payload != nil {
		content, err := json.Marshal(payload)
		if err != nil {
			return nil, err
		}
		body = bytes.NewReader(content)
	}

	req, err := http.NewRequest(method, c.baseURL+path, body)
	if err != nil {
		return nil, err
	}
	setCommonHeaders(req, authToken, c.baseURL, referer)
	return req, nil
}

func defaultStatusError(statusCode int, body []byte) error {
	return fmt.Errorf("%s", localizedStatusMessage(statusCode, body))
}

func localizedStatusMessage(statusCode int, body []byte) string {
	var apiErr apiErrorBody
	if err := json.Unmarshal(body, &apiErr); err == nil {
		switch {
		case statusCode == http.StatusUnauthorized && (apiErr.Reason == "INVALID_CREDENTIALS" || strings.EqualFold(apiErr.Message, "invalid email or password")):
			return "邮箱或密码错误"
		case statusCode == http.StatusUnauthorized:
			return "登录状态已失效，请重新登录"
		case strings.TrimSpace(apiErr.Message) != "":
			return fmt.Sprintf("请求失败了（状态码 %d）：%s", statusCode, localizedVisibleText(apiErr.Message, "服务端返回了错误"))
		}
	}

	if statusCode == http.StatusUnauthorized {
		return "登录状态已失效，请重新登录"
	}

	text := strings.TrimSpace(string(body))
	if text == "" {
		return fmt.Sprintf("请求失败了（状态码 %d）", statusCode)
	}
	return fmt.Sprintf("请求失败了（状态码 %d）：%s", statusCode, localizedVisibleText(text, "服务端返回了错误"))
}

func localizedVisibleText(text string, fallback string) string {
	trimmed := strings.TrimSpace(text)
	if trimmed == "" {
		return fallback
	}
	lower := strings.ToLower(trimmed)
	switch {
	case lower == "invalid email or password":
		return "邮箱或密码错误"
	case strings.Contains(lower, "tile is covered"):
		return "这个方块被挡住了，现在还不能点"
	case strings.Contains(lower, "tile not on board"):
		return "这个方块已经不在棋盘上了"
	case strings.Contains(lower, "daily limit reached"):
		return "今天这个难度的次数已经用完了"
	case strings.Contains(lower, "slot full"):
		return "槽位已经满了"
	case strings.Contains(lower, "session not found"):
		return "这局已经找不到了，可能已经结束"
	case strings.Contains(lower, "invalid action"):
		return "这个操作不对"
	case strings.Contains(lower, "unauthorized") || strings.Contains(lower, "invalid token"):
		return "登录状态已失效，请重新登录"
	default:
		if containsASCIIAlpha(trimmed) {
			return fallback
		}
		return trimmed
	}
}

func localizedDifficultyLabel(difficulty string) string {
	switch strings.TrimSpace(strings.ToLower(difficulty)) {
	case model.DifficultyEasy:
		return "简单"
	case model.DifficultyNormal:
		return "普通"
	case model.DifficultyHard:
		return "困难"
	case model.DifficultyHell:
		return "地狱"
	default:
		return difficulty
	}
}

func localizedActionLabel(action string) string {
	switch strings.TrimSpace(strings.ToLower(action)) {
	case "click":
		return "点击"
	case "undo":
		return "撤回"
	case "remove":
		return "移除"
	case "shuffle":
		return "洗牌"
	case "abandon":
		return "放弃"
	default:
		return action
	}
}

func containsASCIIAlpha(text string) bool {
	for _, ch := range text {
		if (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') {
			return true
		}
	}
	return false
}

func formatStatusError(statusCode int, body []byte, formatter statusErrorFormatter) error {
	if formatter == nil {
		formatter = defaultStatusError
	}
	baseErr := formatter(statusCode, body)
	if statusCode == http.StatusUnauthorized {
		return &statusError{statusCode: statusCode, body: body, err: &unauthorizedError{cause: baseErr}}
	}
	return &statusError{statusCode: statusCode, body: body, err: baseErr}
}

func IsUnauthorized(err error) bool {
	return errors.Is(err, ErrUnauthorized)
}

func (c *APIClient) doRequestBody(req *http.Request, formatStatusErrorFn statusErrorFormatter) ([]byte, error) {
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode != http.StatusOK {
		return nil, formatStatusError(resp.StatusCode, respBody, formatStatusErrorFn)
	}
	return respBody, nil
}

func doJSONRequest[T any](client *APIClient, req *http.Request, formatStatusErrorFn statusErrorFormatter) (*T, error) {
	respBody, err := client.doRequestBody(req, formatStatusErrorFn)
	if err != nil {
		return nil, err
	}

	var value T
	if err := json.Unmarshal(respBody, &value); err != nil {
		return nil, err
	}
	return &value, nil
}

func (c *APIClient) ValidateAuthToken(authToken string) (*model.AuthMeResponse, error) {
	req, err := c.newJSONRequest(http.MethodGet, authMePath, authToken, c.baseURL+"/dashboard", nil)
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.AuthMeResponse](c, req, nil)
}

func (c *APIClient) DoLogin(email string, password string) (*model.LoginResponse, string, error) {
	req, err := c.newJSONRequest(http.MethodPost, loginPath, "", c.baseURL+"/dashboard", model.LoginRequest{Email: email, Password: password})
	if err != nil {
		return nil, "", err
	}

	loginResp, err := doJSONRequest[model.LoginResponse](c, req, nil)
	if err != nil {
		return nil, "", err
	}
	if loginResp.Code != 0 {
		if loginResp.Reason == "INVALID_CREDENTIALS" || strings.EqualFold(loginResp.Message, "invalid email or password") {
			return loginResp, "", &unauthorizedError{cause: fmt.Errorf("邮箱或密码错误")}
		}
		if strings.TrimSpace(loginResp.Message) != "" {
			return loginResp, "", fmt.Errorf("登录失败：%s", localizedVisibleText(loginResp.Message, "服务端返回错误"))
		}
		return loginResp, "", fmt.Errorf("登录失败，服务端返回的错误码是 %d", loginResp.Code)
	}

	accessToken := strings.TrimSpace(loginResp.Data.AccessToken)
	if accessToken == "" {
		return loginResp, "", fmt.Errorf("登录返回的令牌为空")
	}

	return loginResp, auth.BuildAuthorization(loginResp.Data.TokenType, accessToken), nil
}
