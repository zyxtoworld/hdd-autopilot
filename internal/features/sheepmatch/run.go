package sheepmatch

import (
	"context"
	"fmt"
	"strings"

	"hdd/internal/auth"
	"hdd/internal/model"
)

type RunOptions struct {
	BaseURL      string
	Difficulty   string
	Account      string
	DryRun       bool
	ResultLogDir string
}

type BatchState struct {
	Config        auth.AuthConfig
	AuthCacheFile string
	ResultLogDir  string
}

func RunBatch(ctx context.Context, state *BatchState) error {
	if state == nil {
		return fmt.Errorf("批量状态不能为空")
	}

	accounts := filterAccounts(state.Config.Accounts, "")
	if len(accounts) == 0 {
		fmt.Println("当前还没有可用账号。")
		return nil
	}

	difficulties := filterDifficulties("")
	if len(difficulties) == 0 {
		fmt.Println("没有可执行的难度设置。")
		return nil
	}

	options := RunOptions{
		BaseURL:      resolveBaseURL("", state.Config.BaseURL),
		Difficulty:   "",
		Account:      "",
		DryRun:       false,
		ResultLogDir: strings.TrimSpace(state.ResultLogDir),
	}

	fmt.Printf("开始运行自动羊了个羊：这次会处理 %d 个账号，难度包括 %s，演练模式：%v。\n", len(accounts), localizedDifficultyList(difficulties), options.DryRun)
	if options.ResultLogDir != "" {
		fmt.Printf("运行时会持续把结果写进账号日志，目录在：%s\n", options.ResultLogDir)
	}

	cacheSaver := newAccountCacheSaver(state.AuthCacheFile, state.Config)
	results := runAccounts(ctx, options, accounts, difficulties, cacheSaver)
	merged, err := mergeAccountUpdates(cacheSaver.Config(), results)
	if err != nil {
		return fmt.Errorf("整理账号信息时出错：%w", err)
	}
	state.Config = merged
	if strings.TrimSpace(state.AuthCacheFile) != "" {
		if err := auth.SaveCache(state.AuthCacheFile, state.Config); err != nil {
			fmt.Printf("保存 auth.json 时出错：%v\n", err)
		}
	}

	printOverallSummary(results)
	fmt.Println("自动羊了个羊处理完成。")
	return nil
}

func resolveBaseURL(flagBaseURL string, cachedBaseURL string) string {
	flagBaseURL = auth.NormalizeBaseURL(flagBaseURL)
	if flagBaseURL != "" {
		return flagBaseURL
	}
	return auth.NormalizeBaseURL(cachedBaseURL)
}

func filterAccounts(accounts []auth.AuthCache, selected string) []auth.AuthCache {
	selected = strings.TrimSpace(selected)
	if selected == "" {
		result := make([]auth.AuthCache, len(accounts))
		copy(result, accounts)
		return result
	}
	var filtered []auth.AuthCache
	for _, account := range accounts {
		if strings.EqualFold(strings.TrimSpace(account.Email), selected) {
			filtered = append(filtered, account)
		}
	}
	return filtered
}

func filterDifficulties(selected string) []string {
	selected = strings.TrimSpace(strings.ToLower(selected))
	if selected == "" {
		result := make([]string, len(model.DifficultyOrder))
		copy(result, model.DifficultyOrder)
		return result
	}
	for _, difficulty := range model.DifficultyOrder {
		if difficulty == selected {
			return []string{difficulty}
		}
	}
	return nil
}
