package mining

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"
)

type Client struct {
	baseURL string
	timeout time.Duration

	mu     sync.RWMutex
	client *http.Client
}

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

func (e *statusError) Error() string { return e.err.Error() }
func (e *statusError) Unwrap() error { return e.err }

func NewClient(cfg Config) *Client {
	c := &Client{baseURL: cfg.BaseURL, timeout: cfg.HTTPTimeout}
	c.Reset()
	return c
}

func (c *Client) Reset() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.client = &http.Client{Jar: &cookieJar{}, Timeout: c.timeout}
}

func (c *Client) GetStatus() (*StatusResponse, error) {
	var resp StatusResponse
	if err := c.getJSON("/mining-api/status", &resp); err != nil {
		return nil, err
	}
	if !resp.Enabled {
		return nil, ErrPoolDisabled
	}
	if resp.CurrentRound == nil || !resp.CurrentRound.IsOpen() {
		return nil, ErrNoOpenRound
	}
	return &resp, nil
}

func (c *Client) GetStatusSnapshot() (*StatusResponse, error) {
	var resp StatusResponse
	if err := c.getJSON("/mining-api/status", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

func (c *Client) GetChallenge() (*ChallengeResponse, error) {
	var resp ChallengeResponse
	if err := c.postJSON("/mining-api/challenge", nil, &resp); err != nil {
		return nil, err
	}
	if !resp.Ok {
		return nil, ChallengeError(&resp)
	}
	return &resp, nil
}

func (c *Client) Heartbeat(req HeartbeatRequest) (*HeartbeatResponse, error) {
	var resp HeartbeatResponse
	if err := c.postJSON("/mining-api/heartbeat", req, &resp); err != nil {
		return nil, err
	}
	if resp.Result == ResultRoundClosed {
		return &resp, ErrRoundClosed
	}
	return &resp, nil
}

func (c *Client) Submit(req SubmitRequest) (*SubmitResponse, error) {
	var resp SubmitResponse
	if err := c.postJSON("/mining-api/submit", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

func (c *Client) getJSON(path string, target any) error {
	req, err := http.NewRequest(http.MethodGet, c.baseURL+path, nil)
	if err != nil {
		return err
	}
	c.setHeaders(req)
	return c.do(req, target)
}

func (c *Client) postJSON(path string, payload any, target any) error {
	var body []byte
	if payload != nil {
		var err error
		body, err = json.Marshal(payload)
		if err != nil {
			return err
		}
	}
	bodyReader := bytes.NewReader(body)
	req, err := http.NewRequest(http.MethodPost, c.baseURL+path, bodyReader)
	if err != nil {
		return err
	}
	c.setHeaders(req)
	req.Header.Set("Content-Type", "application/json")
	return c.do(req, target)
}

func (c *Client) do(req *http.Request, target any) error {
	resp, err := c.httpClient().Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode < http.StatusOK || resp.StatusCode >= http.StatusMultipleChoices {
		body, _ := io.ReadAll(resp.Body)
		return formatStatusError(resp.StatusCode, body, nil)
	}
	return json.NewDecoder(resp.Body).Decode(target)
}

func (c *Client) httpClient() *http.Client {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.client
}

func (c *Client) setHeaders(req *http.Request) {
	req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
}

func formatStatusError(statusCode int, body []byte, formatter statusErrorFormatter) error {
	if formatter == nil {
		formatter = func(statusCode int, body []byte) error {
			return fmt.Errorf("%s", localizedStatusMessage(statusCode, body))
		}
	}
	return &statusError{statusCode: statusCode, body: body, err: formatter(statusCode, body)}
}

func localizedStatusMessage(statusCode int, body []byte) string {
	var apiErr apiErrorBody
	if err := json.Unmarshal(body, &apiErr); err == nil && strings.TrimSpace(apiErr.Message) != "" {
		return fmt.Sprintf("请求失败（状态码 %d）：%s", statusCode, LocalizedMessage(apiErr.Message))
	}
	text := strings.TrimSpace(string(body))
	if text == "" {
		return fmt.Sprintf("请求失败（状态码 %d）", statusCode)
	}
	return fmt.Sprintf("请求失败（状态码 %d）：%s", statusCode, LocalizedMessage(text))
}

type cookieJar struct {
	mu      sync.Mutex
	cookies map[string][]*http.Cookie
}

func (j *cookieJar) SetCookies(u *url.URL, cookies []*http.Cookie) {
	j.mu.Lock()
	defer j.mu.Unlock()
	if j.cookies == nil {
		j.cookies = make(map[string][]*http.Cookie)
	}
	j.cookies[u.Host] = append(j.cookies[u.Host], cookies...)
}

func (j *cookieJar) Cookies(u *url.URL) []*http.Cookie {
	j.mu.Lock()
	defer j.mu.Unlock()
	return j.cookies[u.Host]
}
