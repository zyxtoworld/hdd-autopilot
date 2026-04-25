package cli

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"
	"sync"
	"time"

	"hdd/internal/auth"
	"hdd/internal/client"
	"hdd/internal/config"
	checkinfeature "hdd/internal/features/checkin"
	"hdd/internal/features/scratch"
	"hdd/internal/features/sheepmatch"
	"hdd/internal/mining"
	"hdd/internal/terminal"
)

type appState struct {
	config        auth.AuthConfig
	authCacheFile string
}

var (
	detectGPUAvailability      = mining.DetectGPUAvailability
	runAutoTunedWithContext    = mining.RunAutoTunedWithContext
	runAutoTunedGPUWithContext = mining.RunAutoTunedGPUWithContext
	startEscapeWatcherHook     = startEscapeWatcher
)

func Run() {
	reader := bufio.NewReader(os.Stdin)
	state, err := loadAppState(config.ResolveDataFilePath("auth.json"))
	if err != nil {
		fmt.Printf("加载账号信息失败：%v\n", err)
		return
	}

	fmt.Println("欢迎使用号多多脚本整合工具。")
	for {
		choice, err := promptMainMenuChoice(reader)
		if err != nil {
			fmt.Printf("读取选项失败：%v\n", err)
			return
		}
		switch choice {
		case "1":
			exit, err := runMiningMenu(reader)
			if err != nil {
				fmt.Printf("处理挖矿菜单失败：%v\n", err)
				return
			}
			if exit {
				fmt.Println("已退出脚本。")
				return
			}
		case "2":
			exit, err := runBatchMenu(reader, &state)
			if err != nil {
				fmt.Printf("处理多账号批量菜单失败：%v\n", err)
				return
			}
			if exit {
				fmt.Println("已退出脚本。")
				return
			}
		case "3":
			fmt.Println("已退出脚本。")
			return
		}
	}
}

func loadAppState(authCacheFile string) (appState, error) {
	cfg, err := auth.LoadCache(authCacheFile)
	if err != nil {
		return appState{}, err
	}
	return appState{config: cfg, authCacheFile: authCacheFile}, nil
}

func runBatchMenu(reader *bufio.Reader, state *appState) (bool, error) {
	for {
		printAccountList(state.config)
		choice, err := promptBatchMenuChoice(reader)
		if err != nil {
			return false, err
		}
		switch choice {
		case "1":
			if err := addOneAccount(state); err != nil {
				fmt.Printf("添加账号失败：%v\n", err)
			}
		case "2":
			exit, err := runFeatureMenu(reader, state)
			if err != nil {
				return false, err
			}
			if exit {
				return true, nil
			}
		case "3":
			return false, nil
		case "4":
			return true, nil
		}
	}
}

func runFeatureMenu(reader *bufio.Reader, state *appState) (bool, error) {
	for {
		choice, err := promptFeatureMenuChoice(reader)
		if err != nil {
			return false, err
		}
		switch choice {
		case "1":
			if err := queryAllBalances(state); err != nil {
				fmt.Printf("查询所有账号余额失败：%v\n", err)
			}
		case "2":
			if err := runCheckinBatch(state); err != nil {
				fmt.Printf("自动签到运行失败：%v\n", err)
			}
		case "3":
			if err := runScratchBatch(state); err != nil {
				fmt.Printf("自动随机刮刮乐运行失败：%v\n", err)
			}
		case "4":
			if err := runSheepMatchBatch(state); err != nil {
				fmt.Printf("自动羊了个羊运行失败：%v\n", err)
			}
		case "5":
			return false, nil
		case "6":
			return true, nil
		}
	}
}

func addOneAccount(state *appState) error {
	apiClient := client.New(state.config.BaseURL)
	defer apiClient.CloseIdleConnections()

	email, err := auth.PromptEmail()
	if err != nil {
		return err
	}
	password, err := auth.PromptPassword()
	if err != nil {
		return err
	}
	loginResp, _, err := apiClient.DoLogin(email, password)
	if err != nil {
		return err
	}
	account := auth.CacheFromLogin(loginResp, email, password, apiClient.BaseURL(), apiClient.ExportSessionCookies())
	state.config = auth.UpsertAccount(state.config, account)
	if err := auth.SaveCache(state.authCacheFile, state.config); err != nil {
		return err
	}
	fmt.Printf("登录成功并已保存账号：%s\n", account.Email)
	return nil
}

func queryAllBalances(state *appState) error {
	ctx, cleanup, err := startEscapeWatcherHook()
	if err != nil {
		return err
	}
	defer cleanup()
	if len(state.config.Accounts) == 0 {
		fmt.Println("当前还没有可用账号。")
		return nil
	}

	batchState := scratch.BatchState{Config: state.config, AuthCacheFile: state.authCacheFile}
	runtimes := scratch.NewAccountRuntimes(batchState.Config.Accounts, batchState.Config.BaseURL)
	defer closeScratchClients(runtimes)

	fmt.Printf("开始查询所有账号余额：这次会处理 %d 个账号。\n", len(runtimes))
	var printMu sync.Mutex
	var wg sync.WaitGroup
	for _, runtime := range runtimes {
		if err := terminal.Check(ctx); err != nil {
			return handleInterrupted(err)
		}
		wg.Add(1)
		go func(runtime *scratch.AccountRuntime) {
			defer wg.Done()
			if err := terminal.Check(ctx); err != nil {
				return
			}
			if err := scratch.EnsureAuthenticatedWithContext(ctx, &batchState, runtime); err != nil {
				printMu.Lock()
				fmt.Printf("账号 %s 查询余额失败：%v\n", runtime.Email(), err)
				printMu.Unlock()
				return
			}
			if err := terminal.Check(ctx); err != nil {
				return
			}
			authMeResp, err := runtime.APIClient.ValidateAuthToken(runtime.AuthToken)
			if err != nil {
				printMu.Lock()
				fmt.Printf("账号 %s 查询余额失败：%v\n", runtime.Email(), err)
				printMu.Unlock()
				return
			}
			printMu.Lock()
			fmt.Printf("账号 %s 当前余额 %.8f，状态 %s。\n", runtime.Email(), authMeResp.Data.Balance, strings.TrimSpace(authMeResp.Data.Status))
			printMu.Unlock()
		}(runtime)
	}
	wg.Wait()
	state.config = batchState.Config
	return waitForBatchReturn(ctx, "全部账号余额查询完成。")
}

func runCheckinBatch(state *appState) error {
	ctx, cleanup, err := startEscapeWatcherHook()
	if err != nil {
		return err
	}
	defer cleanup()
	if len(state.config.Accounts) == 0 {
		fmt.Println("当前还没有可用账号。")
		return nil
	}

	batchState := scratch.BatchState{Config: state.config, AuthCacheFile: state.authCacheFile}
	runtimes := scratch.NewAccountRuntimes(batchState.Config.Accounts, batchState.Config.BaseURL)
	defer closeScratchClients(runtimes)

	fmt.Printf("开始运行自动签到：这次会处理 %d 个账号。\n", len(runtimes))
	logDir := config.ResolveDataFilePath("log/checkin")
	var printMu sync.Mutex
	var wg sync.WaitGroup
	for _, runtime := range runtimes {
		if err := terminal.Check(ctx); err != nil {
			return handleInterrupted(err)
		}
		wg.Add(1)
		go func(runtime *scratch.AccountRuntime) {
			defer wg.Done()
			if err := terminal.Check(ctx); err != nil {
				return
			}
			if err := scratch.EnsureAuthenticatedWithContext(ctx, &batchState, runtime); err != nil {
				printMu.Lock()
				fmt.Printf("账号 %s 签到失败：%v\n", runtime.Email(), err)
				printMu.Unlock()
				return
			}
			result := checkinfeature.RunWithContext(ctx, runtime.APIClient, runtime.AuthToken, runtime.Email())
			printMu.Lock()
			fmt.Printf("这次处理账号 %s 的结果：%s，本次余额增加 %.2f，签到后余额 %.8f。\n", result.Email, result.Status, result.Delta, result.BalanceAfter)
			printMu.Unlock()
			if err := checkinfeature.AppendCheckinLog(logDir, result); err != nil {
				printMu.Lock()
				fmt.Printf("写入签到日志失败：%v\n", err)
				printMu.Unlock()
			}
		}(runtime)
	}
	wg.Wait()
	state.config = batchState.Config
	return waitForBatchReturn(ctx, "全部账号签到完成。")
}

func runScratchBatch(state *appState) error {
	ctx, cleanup, err := startEscapeWatcherHook()
	if err != nil {
		return err
	}
	defer cleanup()
	if len(state.config.Accounts) == 0 {
		fmt.Println("当前还没有可用账号。")
		return nil
	}

	batchState := scratch.BatchState{Config: state.config, AuthCacheFile: state.authCacheFile}
	runtimes := scratch.NewAccountRuntimes(batchState.Config.Accounts, batchState.Config.BaseURL)
	defer closeScratchClients(runtimes)

	fmt.Printf("开始运行自动随机刮刮乐：这次会处理 %d 个账号，每个账号执行 1 次。\n", len(runtimes))
	options := scratch.RunOptions{
		HistoryRetries: 3,
		HistoryWait:    400 * time.Millisecond,
	}
	logDir := config.ResolveDataFilePath("log/scratch")
	var printMu sync.Mutex
	var wg sync.WaitGroup
	for _, runtime := range runtimes {
		if err := terminal.Check(ctx); err != nil {
			return handleInterrupted(err)
		}
		wg.Add(1)
		go func(runtime *scratch.AccountRuntime) {
			defer wg.Done()
			_ = scratch.RunAccountRoundWithContext(ctx, &batchState, runtime, options, logDir, func(format string, args ...any) {
				printMu.Lock()
				defer printMu.Unlock()
				fmt.Printf(format, args...)
			})
		}(runtime)
	}
	wg.Wait()
	state.config = batchState.Config
	return waitForBatchReturn(ctx, "自动随机刮刮乐处理完成。")
}

func runSheepMatchBatch(state *appState) error {
	ctx, cleanup, err := startEscapeWatcherHook()
	if err != nil {
		return err
	}
	defer cleanup()
	if len(state.config.Accounts) == 0 {
		fmt.Println("当前还没有可用账号。")
		return nil
	}

	batchState := sheepmatch.BatchState{
		Config:        state.config,
		AuthCacheFile: state.authCacheFile,
		ResultLogDir:  config.ResolveDataFilePath("log/sheep-match"),
	}
	if err := sheepmatch.RunBatch(ctx, &batchState); err != nil {
		return handleInterrupted(err)
	}
	state.config = batchState.Config
	return waitForBatchReturn(ctx, "自动羊了个羊处理完成。")
}

func closeScratchClients(runtimes []*scratch.AccountRuntime) {
	for _, runtime := range runtimes {
		if runtime == nil || runtime.APIClient == nil {
			continue
		}
		runtime.APIClient.CloseIdleConnections()
	}
}

func printAccountList(cfg auth.AuthConfig) {
	fmt.Println("当前已保存的账号：")
	if len(cfg.Accounts) == 0 {
		fmt.Println("（还没有）")
	} else {
		for i, account := range cfg.Accounts {
			marker := ""
			if cfg.SelectedEmail != "" && strings.EqualFold(strings.TrimSpace(account.Email), strings.TrimSpace(cfg.SelectedEmail)) {
				marker = " [上次使用]"
			}
			fmt.Printf("%d. %s%s\n", i+1, account.Email, marker)
		}
	}
}

func promptMainMenuChoice(reader *bufio.Reader) (string, error) {
	for {
		fmt.Println("1. 挖矿")
		fmt.Println("2. 需要登录的多账号批量操作功能")
		fmt.Println("3. 退出脚本")
		fmt.Print("请输入选项 (1/2/3): ")
		line, err := reader.ReadString('\n')
		if err != nil && err != io.EOF {
			return "", err
		}
		choice := strings.TrimSpace(line)
		switch choice {
		case "1", "2", "3":
			return choice, nil
		case "":
			if err == io.EOF {
				return "", fmt.Errorf("未输入选项")
			}
			fmt.Println("你还没有输入选项，请输入 1、2 或 3。")
		default:
			fmt.Printf("无法识别的选项 %q，请输入 1、2 或 3。\n", choice)
		}
		if err == io.EOF {
			return "", fmt.Errorf("未输入有效选项")
		}
	}
}

func runMiningMenu(reader *bufio.Reader) (bool, error) {
	for {
		choice, err := promptMiningMenuChoice(reader)
		if err != nil {
			return false, err
		}
		switch choice {
		case "1":
			exit, err := runMiningMode(reader, mining.ModeInviteThenBalance)
			if err != nil {
				fmt.Printf("先挖邀请码再挖余额码失败：%v\n", err)
			}
			if exit {
				return true, nil
			}
		case "2":
			exit, err := runMiningMode(reader, mining.ModeBalanceThenInvite)
			if err != nil {
				fmt.Printf("先挖余额码再挖邀请码失败：%v\n", err)
			}
			if exit {
				return true, nil
			}
		case "3":
			exit, err := runMiningMode(reader, mining.ModeInviteOnly)
			if err != nil {
				fmt.Printf("只挖邀请码失败：%v\n", err)
			}
			if exit {
				return true, nil
			}
		case "4":
			exit, err := runMiningMode(reader, mining.ModeBalanceOnly)
			if err != nil {
				fmt.Printf("只挖余额码失败：%v\n", err)
			}
			if exit {
				return true, nil
			}
		case "5":
			return false, nil
		case "6":
			return true, nil
		}
	}
}

func promptMiningMenuChoice(reader *bufio.Reader) (string, error) {
	for {
		fmt.Println("1. 先挖邀请码再挖余额码")
		fmt.Println("2. 先挖余额码再挖邀请码")
		fmt.Println("3. 只挖邀请码")
		fmt.Println("4. 只挖余额码")
		fmt.Println("5. 返回上一级菜单")
		fmt.Println("6. 退出脚本")
		fmt.Print("请输入选项 (1/2/3/4/5/6): ")
		line, err := reader.ReadString('\n')
		if err != nil && err != io.EOF {
			return "", err
		}
		choice := strings.TrimSpace(line)
		switch choice {
		case "1", "2", "3", "4", "5", "6":
			return choice, nil
		case "":
			if err == io.EOF {
				return "", fmt.Errorf("未输入选项")
			}
			fmt.Println("你还没有输入选项，请输入 1、2、3、4、5 或 6。")
		default:
			fmt.Printf("无法识别的选项 %q，请输入 1、2、3、4、5 或 6。\n", choice)
		}
		if err == io.EOF {
			return "", fmt.Errorf("未输入有效选项")
		}
	}
}

func runMiningMode(reader *bufio.Reader, mode mining.Mode) (bool, error) {
	availability := detectGPUAvailability(mode)
	if !availability.Available {
		if availability.Reason != "" {
			fmt.Println(availability.Reason)
		}
		ctx, cleanup, err := startEscapeWatcherHook()
		if err != nil {
			return false, err
		}
		defer cleanup()
		return false, handleInterrupted(runAutoTunedWithContext(ctx, mode))
	}
	choice, err := promptMiningRuntimeChoice(reader)
	if err != nil {
		return false, err
	}
	if choice == "3" {
		return false, nil
	}
	if choice == "4" {
		return true, nil
	}
	ctx, cleanup, err := startEscapeWatcherHook()
	if err != nil {
		return false, err
	}
	defer cleanup()
	switch choice {
	case "1":
		return false, handleInterrupted(runAutoTunedGPUWithContext(ctx, mode))
	case "2":
		return false, handleInterrupted(runAutoTunedWithContext(ctx, mode))
	}
	return false, nil
}

func promptMiningRuntimeChoice(reader *bufio.Reader) (string, error) {
	for {
		fmt.Println("1. GPU 挖矿")
		fmt.Println("2. CPU 挖矿")
		fmt.Println("3. 返回上一级菜单")
		fmt.Println("4. 退出脚本")
		fmt.Print("请输入选项 (1/2/3/4): ")
		line, err := reader.ReadString('\n')
		if err != nil && err != io.EOF {
			return "", err
		}
		choice := strings.TrimSpace(line)
		switch choice {
		case "1", "2", "3", "4":
			return choice, nil
		case "":
			if err == io.EOF {
				return "", fmt.Errorf("未输入选项")
			}
			fmt.Println("你还没有输入选项，请输入 1、2、3 或 4。")
		default:
			fmt.Printf("无法识别的选项 %q，请输入 1、2、3 或 4。\n", choice)
		}
		if err == io.EOF {
			return "", fmt.Errorf("未输入有效选项")
		}
	}
}

func startEscapeWatcher() (context.Context, func(), error) {
	return terminal.StartEscapeWatcher("按 ESC 返回上一级菜单")
}

func waitForBatchReturn(ctx context.Context, message string) error {
	if err := terminal.Check(ctx); err != nil {
		return handleInterrupted(err)
	}
	fmt.Printf("%s若要返回上一级菜单，请按 ESC。\n", message)
	return handleInterrupted(terminal.SleepContext(ctx, 24*time.Hour))
}

func handleInterrupted(err error) error {
	if err == nil {
		return nil
	}
	if errors.Is(err, terminal.ErrInterrupted) {
		fmt.Println("已返回上一级菜单。")
		return nil
	}
	return err
}

func promptBatchMenuChoice(reader *bufio.Reader) (string, error) {
	for {
		fmt.Println("1. 添加账号")
		fmt.Println("2. 账号添加完成，选择脚本功能")
		fmt.Println("3. 返回上一级菜单")
		fmt.Println("4. 退出脚本")
		fmt.Print("请输入选项 (1/2/3/4): ")
		line, err := reader.ReadString('\n')
		if err != nil && err != io.EOF {
			return "", err
		}
		choice := strings.TrimSpace(line)
		switch choice {
		case "1", "2", "3", "4":
			return choice, nil
		case "":
			if err == io.EOF {
				return "", fmt.Errorf("未输入选项")
			}
			fmt.Println("你还没有输入选项，请输入 1、2、3 或 4。")
		default:
			fmt.Printf("无法识别的选项 %q，请输入 1、2、3 或 4。\n", choice)
		}
		if err == io.EOF {
			return "", fmt.Errorf("未输入有效选项")
		}
	}
}

func promptFeatureMenuChoice(reader *bufio.Reader) (string, error) {
	for {
		fmt.Println("1. 查询所有账号余额")
		fmt.Println("2. 自动签到")
		fmt.Println("3. 自动随机刮刮乐")
		fmt.Println("4. 自动羊了个羊")
		fmt.Println("5. 返回上一级菜单")
		fmt.Println("6. 退出脚本")
		fmt.Print("请输入选项 (1/2/3/4/5/6): ")
		line, err := reader.ReadString('\n')
		if err != nil && err != io.EOF {
			return "", err
		}
		choice := strings.TrimSpace(line)
		switch choice {
		case "1", "2", "3", "4", "5", "6":
			return choice, nil
		case "":
			if err == io.EOF {
				return "", fmt.Errorf("未输入选项")
			}
			fmt.Println("你还没有输入选项，请输入 1、2、3、4、5 或 6。")
		default:
			fmt.Printf("无法识别的选项 %q，请输入 1、2、3、4、5 或 6。\n", choice)
		}
		if err == io.EOF {
			return "", fmt.Errorf("未输入有效选项")
		}
	}
}
