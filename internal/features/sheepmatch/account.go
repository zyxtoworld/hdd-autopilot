package sheepmatch

import (
	"context"
	"fmt"
	"strings"
	"sync"
	"time"

	"hdd/internal/auth"
	"hdd/internal/client"
	"hdd/internal/logging"
	"hdd/internal/model"
	"hdd/internal/terminal"
)

type accountRunResult struct {
	account auth.AuthCache
	rounds  []model.RoundResultSummary
	stats   []model.AccountRunSummary
	err     error
}

type accountCacheSaver struct {
	mu            sync.Mutex
	authCacheFile string
	config        auth.AuthConfig
}

type runtimeAuthState struct {
	account    auth.AuthCache
	authToken  string
	cacheSaver *accountCacheSaver
}

func newAccountCacheSaver(authCacheFile string, config auth.AuthConfig) *accountCacheSaver {
	return &accountCacheSaver{authCacheFile: authCacheFile, config: config}
}

func newRuntimeAuthState(account auth.AuthCache, authToken string, cacheSaver *accountCacheSaver) *runtimeAuthState {
	return &runtimeAuthState{account: account, authToken: authToken, cacheSaver: cacheSaver}
}

func (s *accountCacheSaver) SaveAccount(account auth.AuthCache) error {
	if s == nil {
		return nil
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	s.config = auth.UpsertAccount(s.config, account)
	if strings.TrimSpace(s.authCacheFile) == "" {
		return nil
	}
	return auth.SaveCache(s.authCacheFile, s.config)
}

func (s *accountCacheSaver) Config() auth.AuthConfig {
	if s == nil {
		return auth.AuthConfig{}
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.config
}

func (s *runtimeAuthState) saveAccount() {
	if s == nil || s.cacheSaver == nil {
		return
	}
	if err := s.cacheSaver.SaveAccount(s.account); err != nil {
		fmt.Printf("账号 %s 保存 auth.json 时出错：%v\n", s.account.Email, err)
	}
}

func (s *runtimeAuthState) refresh(apiClient *client.APIClient) error {
	if s == nil {
		return nil
	}
	authToken, updated, err := ensureAuthenticated(apiClient, s.account)
	if err != nil {
		return err
	}
	s.authToken = authToken
	s.account = updated
	s.saveAccount()
	return nil
}

func withRuntimeAuthRetry[T any](apiClient *client.APIClient, authState *runtimeAuthState, action func(string) (*T, error)) (*T, error) {
	if authState == nil {
		return action("")
	}
	resp, err := action(authState.authToken)
	if err == nil || !client.IsUnauthorized(err) {
		return resp, err
	}
	fmt.Printf("账号 %s 的登录状态中途失效了，正在重新登录后继续。\n", authState.account.Email)
	if err := authState.refresh(apiClient); err != nil {
		return nil, err
	}
	return action(authState.authToken)
}

func runAccounts(ctx context.Context, options RunOptions, accounts []auth.AuthCache, difficulties []string, cacheSaver *accountCacheSaver) []accountRunResult {
	results := make([]accountRunResult, len(accounts))
	var wg sync.WaitGroup
	for idx, account := range accounts {
		wg.Add(1)
		go func(index int, initial auth.AuthCache) {
			defer wg.Done()
			results[index] = runAccount(ctx, options, initial, difficulties, cacheSaver)
		}(idx, account)
	}
	wg.Wait()
	return results
}

func runAccount(ctx context.Context, options RunOptions, account auth.AuthCache, difficulties []string, cacheSaver *accountCacheSaver) accountRunResult {
	apiClient := client.New(options.BaseURL)
	defer apiClient.CloseIdleConnections()

	result := accountRunResult{account: account}
	if err := terminal.Check(ctx); err != nil {
		result.err = err
		return result
	}
	authToken, updatedAccount, err := ensureAuthenticated(apiClient, account)
	if err != nil {
		fmt.Printf("账号 %s 登录失败了：%v\n", account.Email, err)
		result.err = err
		return result
	}
	authState := newRuntimeAuthState(updatedAccount, authToken, cacheSaver)
	result.account = authState.account
	authState.saveAccount()
	if options.ResultLogDir != "" {
		if err := logging.AppendRunHeader(options.ResultLogDir, authState.account.Email, time.Now()); err != nil {
			fmt.Printf("账号 %s 写入运行日志开头失败：%v\n", authState.account.Email, err)
		}
	}

	configResp, err := getConfigWithRetry(apiClient, authState)
	if err != nil {
		fmt.Printf("账号 %s 获取游戏配置失败了：%v\n", authState.account.Email, err)
		result.account = authState.account
		result.err = err
		return result
	}
	fmt.Printf("账号 %s 已准备好：槽位上限=%d，最多可同时进行 %d 局，最小操作间隔=%dms。\n",
		authState.account.Email,
		configResp.SlotLimit,
		configResp.MaxActiveSessions,
		configResp.MinIntervalMs,
	)
	pacer := newActionPacer(configResp.MinIntervalMs)

	playStatus, err := getTileMeWithRetry(apiClient, authState)
	if err != nil {
		fmt.Printf("账号 %s 获取今天剩余次数失败了：%v\n", authState.account.Email, err)
		result.account = authState.account
		result.err = err
		return result
	}
	usedTodayByDifficulty := map[string]int{}
	remainingByDifficulty := map[string]int{}
	if playStatus != nil {
		if playStatus.DailyPlaysUsed != nil {
			usedTodayByDifficulty = playStatus.DailyPlaysUsed
		}
		if playStatus.DailyPlaysRemaining != nil {
			remainingByDifficulty = playStatus.DailyPlaysRemaining
		}
	}

	rounds, err := drainPendingSessionsWithContext(ctx, apiClient, authState, options.DryRun, options.ResultLogDir, pacer, usedTodayByDifficulty, remainingByDifficulty)
	result.rounds = append(result.rounds, rounds...)
	if err != nil {
		fmt.Printf("账号 %s 处理未完成的对局时出错：%v\n", authState.account.Email, err)
		result.account = authState.account
		result.err = err
		result.stats = summariesFromMap(summarizeRoundsByDifficulty(authState.account.Email, result.rounds))
		if options.ResultLogDir != "" {
			if logErr := logging.AppendAccountSummary(options.ResultLogDir, authState.account.Email, time.Now(), result.stats); logErr != nil {
				fmt.Printf("账号 %s 写入账号汇总失败了：%v\n", authState.account.Email, logErr)
			}
		}
		return result
	}

	baseStats := summarizeRoundsByDifficulty(authState.account.Email, result.rounds)
	visited := map[string]struct{}{}
	for _, difficulty := range difficulties {
		seed := baseStats[difficulty]
		nextRound := nextRoundIndexForNewRound(usedTodayByDifficulty[difficulty])
		totalRounds := totalRoundCount(usedTodayByDifficulty[difficulty], remainingByDifficulty[difficulty])
		summary, difficultyRounds := runDifficultyWithContext(ctx, apiClient, authState, difficulty, options.DryRun, seed, options.ResultLogDir, pacer, nextRound, totalRounds)
		result.rounds = append(result.rounds, difficultyRounds...)
		result.stats = append(result.stats, summary)
		visited[difficulty] = struct{}{}
		fmt.Printf("账号 %s 的%s难度已完成：一共玩了 %d 局，成功 %d 局，放弃 %d 局，失败 %d 局，总收益 %.8f，今天还剩 %d 次。\n",
			summary.Email,
			localizedDifficulty(summary.Difficulty),
			summary.Played,
			summary.Won,
			summary.Abandoned,
			summary.Failed,
			summary.TotalReward,
			summary.RemainingAfter,
		)
		if summary.Err != nil && result.err == nil {
			result.err = summary.Err
		}
	}
	for difficulty, summary := range baseStats {
		if _, ok := visited[difficulty]; ok {
			continue
		}
		result.stats = append(result.stats, summary)
	}
	result.account = authState.account
	if options.ResultLogDir != "" {
		if logErr := logging.AppendAccountSummary(options.ResultLogDir, authState.account.Email, time.Now(), result.stats); logErr != nil {
			fmt.Printf("账号 %s 写入账号汇总失败：%v\n", authState.account.Email, logErr)
		}
	}
	return result
}

func mergeAccountUpdates(cfg auth.AuthConfig, results []accountRunResult) (auth.AuthConfig, error) {
	merged := cfg
	for _, result := range results {
		if strings.TrimSpace(result.account.Email) == "" {
			continue
		}
		merged = auth.UpsertAccount(merged, result.account)
	}
	return merged, nil
}

func ensureAuthenticated(apiClient *client.APIClient, account auth.AuthCache) (string, auth.AuthCache, error) {
	baseURL := apiClient.BaseURL()
	if session, ok := auth.GetSession(account, baseURL); ok {
		if len(session.Cookies) > 0 {
			if err := apiClient.LoadSessionCookies(session.Cookies); err != nil {
				return "", auth.AuthCache{}, err
			}
			authMeResp, err := apiClient.ValidateAuthToken("")
			if err == nil {
				email := strings.TrimSpace(authMeResp.Data.Email)
				if email != "" && !strings.EqualFold(email, strings.TrimSpace(account.Email)) {
					apiClient.ClearSessionCookies()
					return "", auth.AuthCache{}, fmt.Errorf("账号 %s 读取到的登录状态属于另一个账号 %s，请重新登录或检查 auth.json", strings.TrimSpace(account.Email), email)
				}
				if email != "" {
					account.Email = email
				}
				updated := auth.UpsertSession(account, model.AuthSession{
					BaseURL:     baseURL,
					TokenType:   session.TokenType,
					AccessToken: session.AccessToken,
					Cookies:     apiClient.ExportSessionCookies(),
				})
				return "", updated, nil
			}
			apiClient.ClearSessionCookies()
			fmt.Printf("账号 %s 的上次登录状态已经过期，继续尝试其他方式恢复登录。\n", account.Email)
		}
		authToken := auth.BuildAuthorization(session.TokenType, session.AccessToken)
		if authToken != "" {
			authMeResp, err := apiClient.ValidateAuthToken(authToken)
			if err == nil {
				if strings.TrimSpace(authMeResp.Data.Email) != "" {
					account.Email = strings.TrimSpace(authMeResp.Data.Email)
				}
				updated := auth.UpsertSession(account, model.AuthSession{
					BaseURL:     baseURL,
					TokenType:   session.TokenType,
					AccessToken: session.AccessToken,
					Cookies:     apiClient.ExportSessionCookies(),
				})
				return authToken, updated, nil
			}
			fmt.Printf("账号 %s 的上次登录信息已经失效，准备重新登录。\n", account.Email)
		}
	}
	if !auth.PasswordUsable(account) {
		return "", auth.AuthCache{}, fmt.Errorf("账号 %s 没有保存密码，无法自动重新登录", account.Email)
	}
	loginResp, authToken, err := apiClient.DoLogin(account.Email, account.Password)
	if err != nil {
		return "", auth.AuthCache{}, err
	}
	updated := auth.CacheFromLogin(loginResp, account.Email, account.Password, baseURL, apiClient.ExportSessionCookies())
	return authToken, updated, nil
}

func printOverallSummary(results []accountRunResult) {
	for _, result := range results {
		if strings.TrimSpace(result.account.Email) == "" {
			continue
		}
		if result.err != nil {
			fmt.Printf("账号 %s 运行结束，但有错误：%v\n", result.account.Email, result.err)
			continue
		}
		fmt.Printf("账号 %s 运行完成。\n", result.account.Email)
	}
}
