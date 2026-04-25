package scratch

import (
	"context"
	"fmt"
	"math/rand"
	"strings"
	"sync"
	"time"

	"hdd/internal/auth"
	"hdd/internal/client"
	"hdd/internal/model"
	"hdd/internal/terminal"
)

const (
	playErrorBackoff  = 3 * time.Second
	revealMinInterval = 15 * time.Second
)

var scratchGameTypes = []string{
	model.ScratchGameTypeLuckyNumbers,
	model.ScratchGameTypeThreeKind,
	model.ScratchGameTypeIconMatch,
	model.ScratchGameTypeTreasureChest,
	model.ScratchGameTypeProgressRun,
}

func init() {
	rand.Seed(time.Now().UnixNano())
}

type RunOptions struct {
	Interval       time.Duration
	MaxRounds      int
	HistoryRetries int
	HistoryWait    time.Duration
}

type BatchState struct {
	mu            sync.Mutex
	Config        auth.AuthConfig
	AuthCacheFile string
}

type AccountRuntime struct {
	APIClient      *client.APIClient
	Account        auth.AuthCache
	AuthToken      string
	TotalCost      float64
	TotalReward    float64
	RoundsPlayed   int
	RevealLimiter  *RevealLimiter
}

func (r *AccountRuntime) Email() string {
	return strings.TrimSpace(r.Account.Email)
}

type RevealLimiter struct {
	minInterval time.Duration
	nextAllowed time.Time
}

func NewRevealLimiter(minInterval time.Duration) *RevealLimiter {
	return &RevealLimiter{minInterval: minInterval}
}

func (r *RevealLimiter) WaitUntil(target time.Time) {
	_ = r.WaitUntilWithContext(context.Background(), target)
}

func (r *RevealLimiter) WaitUntilWithContext(ctx context.Context, target time.Time) error {
	if err := terminal.Check(ctx); err != nil {
		return err
	}
	now := time.Now()
	readyAt := now
	if target.After(readyAt) {
		readyAt = target
	}
	if r.nextAllowed.After(readyAt) {
		readyAt = r.nextAllowed
	}
	if wait := time.Until(readyAt); wait > 0 {
		if err := terminal.SleepContext(ctx, wait); err != nil {
			return err
		}
	}
	r.nextAllowed = readyAt.Add(r.minInterval)
	return nil
}

func LoadState(authCacheFile string) (BatchState, error) {
	cfg, err := auth.LoadCache(authCacheFile)
	if err != nil {
		return BatchState{}, fmt.Errorf("读取登录缓存失败: %w", err)
	}
	return BatchState{Config: cfg, AuthCacheFile: authCacheFile}, nil
}

func (s *BatchState) SaveAccount(account auth.AuthCache) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.Config = auth.UpsertAccount(s.Config, account)
	if strings.TrimSpace(s.AuthCacheFile) == "" {
		return nil
	}
	return auth.SaveCache(s.AuthCacheFile, s.Config)
}

func NewAccountRuntimes(accounts []auth.AuthCache, baseURL string) []*AccountRuntime {
	runtimes := make([]*AccountRuntime, 0, len(accounts))
	for _, account := range accounts {
		runtimes = append(runtimes, &AccountRuntime{
			APIClient:     client.New(baseURL),
			Account:       account,
			RevealLimiter: NewRevealLimiter(revealMinInterval),
		})
	}
	return runtimes
}

func EnsureAuthenticated(state *BatchState, runtime *AccountRuntime) error {
	return EnsureAuthenticatedWithContext(context.Background(), state, runtime)
}

func EnsureAuthenticatedWithContext(ctx context.Context, state *BatchState, runtime *AccountRuntime) error {
	if err := terminal.Check(ctx); err != nil {
		return err
	}
	baseURL := runtime.APIClient.BaseURL()
	if session, ok := auth.GetSession(runtime.Account, baseURL); ok {
		if len(session.Cookies) > 0 {
			if err := runtime.APIClient.LoadSessionCookies(session.Cookies); err != nil {
				return err
			}
			authMeResp, err := runtime.APIClient.ValidateAuthToken("")
			if err == nil {
				email := strings.TrimSpace(authMeResp.Data.Email)
				if email != "" && !strings.EqualFold(email, strings.TrimSpace(runtime.Account.Email)) {
					runtime.APIClient.ClearSessionCookies()
					return fmt.Errorf("账号 %s 读取到的登录状态属于另一个账号 %s，请重新登录或检查 auth.json", strings.TrimSpace(runtime.Account.Email), email)
				}
				if email != "" {
					runtime.Account.Email = email
				}
				runtime.Account = auth.UpsertSession(runtime.Account, model.AuthSession{
					BaseURL:     baseURL,
					TokenType:   session.TokenType,
					AccessToken: session.AccessToken,
					Cookies:     runtime.APIClient.ExportSessionCookies(),
				})
				runtime.AuthToken = ""
				return state.SaveAccount(runtime.Account)
			}
			runtime.APIClient.ClearSessionCookies()
			fmt.Printf("账号 %s 的上次登录状态已经过期，继续尝试其他方式恢复登录。\n", runtime.Email())
		}
		authToken := auth.BuildAuthorization(session.TokenType, session.AccessToken)
		if authToken != "" {
			authMeResp, err := runtime.APIClient.ValidateAuthToken(authToken)
			if err == nil {
				if strings.TrimSpace(authMeResp.Data.Email) != "" {
					runtime.Account.Email = strings.TrimSpace(authMeResp.Data.Email)
				}
				runtime.Account = auth.UpsertSession(runtime.Account, model.AuthSession{
					BaseURL:     baseURL,
					TokenType:   session.TokenType,
					AccessToken: session.AccessToken,
					Cookies:     runtime.APIClient.ExportSessionCookies(),
				})
				runtime.AuthToken = authToken
				return state.SaveAccount(runtime.Account)
			}
			fmt.Printf("账号 %s 的上次登录信息已经失效，准备重新登录。\n", runtime.Email())
		}
	}
	if !auth.PasswordUsable(runtime.Account) {
		return fmt.Errorf("账号 %s 没有保存密码，无法自动重新登录", runtime.Email())
	}
	loginResp, authToken, err := runtime.APIClient.DoLogin(runtime.Account.Email, runtime.Account.Password)
	if err != nil {
		return err
	}
	runtime.Account = auth.CacheFromLogin(loginResp, runtime.Account.Email, runtime.Account.Password, baseURL, runtime.APIClient.ExportSessionCookies())
	runtime.AuthToken = authToken
	return state.SaveAccount(runtime.Account)
}

func Reauthenticate(state *BatchState, runtime *AccountRuntime) error {
	return ReauthenticateWithContext(context.Background(), state, runtime)
}

func ReauthenticateWithContext(ctx context.Context, state *BatchState, runtime *AccountRuntime) error {
	if err := terminal.Check(ctx); err != nil {
		return err
	}
	fmt.Printf("检测到账号 %s 的登录态失效，尝试重新登录。\n", runtime.Email())
	runtime.AuthToken = ""
	return EnsureAuthenticatedWithContext(ctx, state, runtime)
}

func RunRound(apiClient *client.APIClient, authToken string, round int, historyAttempts int, historyWait time.Duration, revealLimiter *RevealLimiter) (result model.ScratchRoundResult) {
	return RunRoundWithContext(context.Background(), apiClient, authToken, round, historyAttempts, historyWait, revealLimiter)
}

func RunRoundWithContext(ctx context.Context, apiClient *client.APIClient, authToken string, round int, historyAttempts int, historyWait time.Duration, revealLimiter *RevealLimiter) (result model.ScratchRoundResult) {
	startedAt := time.Now()
	result = model.ScratchRoundResult{Round: round}
	defer func() {
		result.Duration = time.Since(startedAt)
	}()

	if err := terminal.Check(ctx); err != nil {
		result.PlayErr = err
		return result
	}

	playResp, err := apiClient.PlayScratch(authToken, randomScratchGameType())
	if err != nil {
		result.PlayErr = err
		return result
	}
	result.PlayResp = playResp

	playHistoryItem, attemptsUsed, err := FetchScratchHistoryItemWithRetryWithContext(ctx, apiClient, authToken, playResp.PlayID, historyAttempts, historyWait, nil)
	result.PlayHistoryAttempts = attemptsUsed
	if err != nil {
		result.PlayHistoryErr = err
		return result
	}
	result.PlayHistoryItem = playHistoryItem

	if strings.TrimSpace(playResp.RevealToken) == "" {
		result.RevealErr = fmt.Errorf("对局 %d 的开奖令牌为空", playResp.PlayID)
		return result
	}

	if err := revealLimiter.WaitUntilWithContext(ctx, model.ScratchRevealReadyAt(playResp)); err != nil {
		result.RevealErr = err
		return result
	}
	if err := terminal.Check(ctx); err != nil {
		result.RevealErr = err
		return result
	}
	revealResp, err := apiClient.RevealScratch(authToken, playResp.PlayID, playResp.RevealToken)
	if err != nil {
		result.RevealErr = err
		return result
	}
	result.RevealResp = revealResp

	revealHistoryItem, attemptsUsed, err := FetchScratchHistoryItemWithRetryWithContext(ctx, apiClient, authToken, playResp.PlayID, historyAttempts, historyWait, func(item *model.ScratchHistoryItem) bool {
		return item != nil && !strings.EqualFold(strings.TrimSpace(item.Status), "pending")
	})
	result.RevealHistoryAttempts = attemptsUsed
	if err != nil {
		result.RevealHistoryErr = err
		return result
	}
	result.RevealHistoryItem = revealHistoryItem
	return result
}

func RunAccountRound(state *BatchState, runtime *AccountRuntime, options RunOptions, printf func(string, ...any)) {
	_ = RunAccountRoundWithContext(context.Background(), state, runtime, options, "", printf)
}

func RunAccountRoundWithContext(ctx context.Context, state *BatchState, runtime *AccountRuntime, options RunOptions, logDir string, printf func(string, ...any)) error {
	if err := terminal.Check(ctx); err != nil {
		return err
	}
	printf("\n当前账号：%s\n", runtime.Email())
	if err := EnsureAuthenticatedWithContext(ctx, state, runtime); err != nil {
		printf("账号 %s 登录失败: %v\n", runtime.Email(), err)
		return err
	}

	round := runtime.RoundsPlayed + 1
	result := RunRoundWithContext(ctx, runtime.APIClient, runtime.AuthToken, round, options.HistoryRetries, options.HistoryWait, runtime.RevealLimiter)
	if hasUnauthorized(result) {
		if err := ReauthenticateWithContext(ctx, state, runtime); err != nil {
			printf("账号 %s 自动重登失败: %v\n", runtime.Email(), err)
			if sleepErr := terminal.SleepContext(ctx, playErrorBackoff); sleepErr != nil {
				return sleepErr
			}
			return err
		}
		result = RunRoundWithContext(ctx, runtime.APIClient, runtime.AuthToken, round, options.HistoryRetries, options.HistoryWait, runtime.RevealLimiter)
	}

	runtime.RoundsPlayed++
	runtime.TotalCost, runtime.TotalReward = AddRoundTotals(result, runtime.TotalCost, runtime.TotalReward)
	PrintRoundResult(printf, result, runtime.TotalCost, runtime.TotalReward)
	if err := AppendScratchRoundLog(logDir, runtime.Email(), result, runtime.TotalCost, runtime.TotalReward); err != nil {
		printf("账号 %s 写入刮刮乐日志失败：%v\n", runtime.Email(), err)
	}

	if result.PlayErr != nil {
		if err := terminal.SleepContext(ctx, playErrorBackoff); err != nil {
			return err
		}
	}
	if result.PlayErr != nil {
		return result.PlayErr
	}
	if result.PlayHistoryErr != nil {
		return result.PlayHistoryErr
	}
	if result.RevealErr != nil {
		return result.RevealErr
	}
	if result.RevealHistoryErr != nil {
		return result.RevealHistoryErr
	}
	return nil
}

func waitForNextRound(round int, interval time.Duration) {
	_ = waitForNextRoundWithContext(context.Background(), round, interval)
}

func waitForNextRoundWithContext(ctx context.Context, round int, interval time.Duration) error {
	if round > 1 && interval > 0 {
		return terminal.SleepContext(ctx, interval)
	}
	return nil
}

func AddRoundTotals(result model.ScratchRoundResult, totalCost float64, totalReward float64) (float64, float64) {
	if result.PlayResp != nil {
		totalCost += result.PlayResp.CostAmount
	}
	if result.RevealHistoryItem != nil {
		totalReward += result.RevealHistoryItem.RewardAmount
	} else if result.RevealResp != nil {
		totalReward += result.RevealResp.RewardAmount
	}
	return totalCost, totalReward
}

func randomScratchGameType() string {
	return scratchGameTypes[rand.Intn(len(scratchGameTypes))]
}

func hasUnauthorized(result model.ScratchRoundResult) bool {
	return client.IsUnauthorized(result.PlayErr) ||
		client.IsUnauthorized(result.PlayHistoryErr) ||
		client.IsUnauthorized(result.RevealErr) ||
		client.IsUnauthorized(result.RevealHistoryErr)
}

func FindScratchHistoryItem(items []model.ScratchHistoryItem, playID int) *model.ScratchHistoryItem {
	for _, item := range items {
		if item.ID == playID {
			itemCopy := item
			return &itemCopy
		}
	}
	return nil
}

func FetchScratchHistoryItemWithRetry(apiClient *client.APIClient, authToken string, playID int, attempts int, wait time.Duration, accept func(*model.ScratchHistoryItem) bool) (*model.ScratchHistoryItem, int, error) {
	return FetchScratchHistoryItemWithRetryWithContext(context.Background(), apiClient, authToken, playID, attempts, wait, accept)
}

func FetchScratchHistoryItemWithRetryWithContext(ctx context.Context, apiClient *client.APIClient, authToken string, playID int, attempts int, wait time.Duration, accept func(*model.ScratchHistoryItem) bool) (*model.ScratchHistoryItem, int, error) {
	if attempts < 1 {
		attempts = 1
	}
	if accept == nil {
		accept = func(item *model.ScratchHistoryItem) bool { return item != nil }
	}

	for attempt := 1; attempt <= attempts; attempt++ {
		if err := terminal.Check(ctx); err != nil {
			return nil, attempt, err
		}
		histResp, err := apiClient.GetScratchHistory(authToken)
		if err != nil {
			return nil, attempt, err
		}
		if item := FindScratchHistoryItem(histResp.Items, playID); accept(item) {
			return item, attempt, nil
		}
		if attempt < attempts && wait > 0 {
			if err := terminal.SleepContext(ctx, wait); err != nil {
				return nil, attempt, err
			}
		}
	}

	return nil, attempts, nil
}
