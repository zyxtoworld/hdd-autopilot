package sheepmatch

import (
	"context"
	"fmt"
	"strings"
	"time"

	"hdd/internal/client"
	"hdd/internal/logging"
	"hdd/internal/model"
	"hdd/internal/solver"
	"hdd/internal/terminal"
)

type actionPacer struct {
	minInterval time.Duration
	lastAction  time.Time
}

type roundProgress struct {
	current int
	total   int
}

func newActionPacer(minIntervalMs int) *actionPacer {
	return &actionPacer{minInterval: time.Duration(maxInt(0, minIntervalMs)) * time.Millisecond}
}

func (p *actionPacer) Wait() {
	if p == nil || p.minInterval <= 0 {
		return
	}
	if !p.lastAction.IsZero() {
		if wait := time.Until(p.lastAction.Add(p.minInterval)); wait > 0 {
			time.Sleep(wait)
		}
	}
	p.lastAction = time.Now()
}

func getConfigWithRetry(apiClient *client.APIClient, authState *runtimeAuthState) (*model.ConfigResponse, error) {
	var lastErr error
	for attempt := 0; attempt < 3; attempt++ {
		resp, err := withRuntimeAuthRetry(apiClient, authState, apiClient.GetConfig)
		if err == nil {
			return resp, nil
		}
		lastErr = err
		time.Sleep(200 * time.Millisecond)
	}
	return nil, lastErr
}

func getTileMeWithRetry(apiClient *client.APIClient, authState *runtimeAuthState) (*model.TileMeResponse, error) {
	return withRuntimeAuthRetry(apiClient, authState, apiClient.GetTileMe)
}

func getRemainingPlays(apiClient *client.APIClient, authState *runtimeAuthState, difficulty string) (int, error) {
	resp, err := getTileMeWithRetry(apiClient, authState)
	if err != nil {
		return 0, err
	}
	if resp == nil || resp.DailyPlaysRemaining == nil {
		return 0, nil
	}
	return resp.DailyPlaysRemaining[difficulty], nil
}

func nextRoundIndexForNewRound(usedToday int) int {
	if usedToday < 0 {
		usedToday = 0
	}
	return usedToday + 1
}

func totalRoundCount(usedToday int, remaining int) int {
	total := usedToday + remaining
	if total < 0 {
		return 0
	}
	return total
}

func normalizeRoundTotal(current int, total int) int {
	if current < 1 {
		current = 1
	}
	if total < current {
		return current
	}
	return total
}

func formatRoundProgress(current int, total int) string {
	total = normalizeRoundTotal(current, total)
	if current < 1 {
		current = 1
	}
	return fmt.Sprintf("今天第 %d/%d 局", current, total)
}

func formatUsedPowerups(usedPowerups []string) string {
	if len(usedPowerups) == 0 {
		return "没用道具"
	}
	labels := make([]string, 0, len(usedPowerups))
	for _, item := range usedPowerups {
		labels = append(labels, powerupLabel(item))
	}
	return "用了" + strings.Join(labels, "、")
}

func formatBalanceForDisplay(balance *float64) string {
	if balance == nil {
		return "未知"
	}
	return fmt.Sprintf("%.8f", *balance)
}

func printRoundResult(result model.RoundResultSummary) {
	total := normalizeRoundTotal(result.RoundIndex, result.RoundTotal)
	reason := ""
	if result.Err != nil {
		reason = fmt.Sprintf("，原因：%v", result.Err)
	}
	fmt.Printf("账号 %s 的%s难度%s结果：%s，%s，耗时 %s，奖励 %.8f，总余额 %s，今天还剩 %d 次，走了 %d 步%s。\n",
		result.Email,
		localizedDifficulty(result.Difficulty),
		formatRoundProgress(result.RoundIndex, total),
		roundStatusLabel(result),
		formatUsedPowerups(result.UsedPowerups),
		result.Duration.Round(time.Millisecond),
		result.Reward,
		formatBalanceForDisplay(result.BalanceAfter),
		result.RemainingAfter,
		result.MoveCount,
		reason,
	)
}

func drainPendingSessionsWithContext(ctx context.Context, apiClient *client.APIClient, authState *runtimeAuthState, dryRun bool, resultLogDir string, pacer *actionPacer, usedTodayByDifficulty map[string]int, remainingByDifficulty map[string]int) ([]model.RoundResultSummary, error) {
	if err := terminal.Check(ctx); err != nil {
		return nil, err
	}
	historyResp, err := withRuntimeAuthRetry(apiClient, authState, apiClient.GetHistory)
	if err != nil {
		return nil, err
	}
	var rounds []model.RoundResultSummary
	for _, item := range historyResp.Items {
		if err := terminal.Check(ctx); err != nil {
			return rounds, err
		}
		if item.Status != "pending" {
			continue
		}
		roundIndex := nextRoundIndexForNewRound(usedTodayByDifficulty[item.Difficulty])
		total := normalizeRoundTotal(roundIndex, totalRoundCount(usedTodayByDifficulty[item.Difficulty], remainingByDifficulty[item.Difficulty]))
		fmt.Printf("账号 %s 继续%s难度，%s（对局 %d）。\n", authState.account.Email, localizedDifficulty(item.Difficulty), formatRoundProgress(roundIndex, total), item.SessionID)
		start := historyItemToStartResponse(&item)
		round, err := playRoundWithContext(ctx, apiClient, authState, start, dryRun, resultLogDir, pacer, true, roundProgress{current: roundIndex, total: total})
		rounds = append(rounds, round)
		printRoundResult(round)
		if err != nil {
			return rounds, err
		}
	}
	return rounds, nil
}

func runDifficultyWithContext(ctx context.Context, apiClient *client.APIClient, authState *runtimeAuthState, difficulty string, dryRun bool, seed model.AccountRunSummary, resultLogDir string, pacer *actionPacer, nextRoundIndex int, totalRounds int) (model.AccountRunSummary, []model.RoundResultSummary) {
	summary := seed
	if err := terminal.Check(ctx); err != nil {
		summary.Err = err
		summary.When = time.Now()
		return summary, nil
	}
	if strings.TrimSpace(summary.Email) == "" {
		summary.Email = authState.account.Email
		summary.Difficulty = difficulty
	}
	remaining, err := getRemainingPlays(apiClient, authState, difficulty)
	if err != nil {
		summary.Err = err
		summary.When = time.Now()
		return summary, nil
	}
	if remaining <= 0 {
		summary.RemainingAfter = 0
		summary.When = time.Now()
		return summary, nil
	}
	totalRounds = normalizeRoundTotal(nextRoundIndex, totalRounds)
	var rounds []model.RoundResultSummary
	for played := 0; played < remaining; played++ {
		if err := terminal.Check(ctx); err != nil {
			summary.Err = err
			break
		}
		progress := roundProgress{current: nextRoundIndex + played, total: totalRounds}
		fmt.Printf("账号 %s 开始玩%s难度，%s。\n", authState.account.Email, localizedDifficulty(difficulty), formatRoundProgress(progress.current, progress.total))
		start, err := withRuntimeAuthRetry(apiClient, authState, func(authToken string) (*model.StartResponse, error) {
			return apiClient.StartGame(authToken, difficulty)
		})
		if err != nil {
			summary.Err = err
			break
		}
		round, roundErr := playRoundWithContext(ctx, apiClient, authState, start, dryRun, resultLogDir, pacer, false, progress)
		rounds = append(rounds, round)
		printRoundResult(round)
		summary = mergeRoundIntoSummary(summary, round)
		if roundErr != nil {
			summary.Err = roundErr
			break
		}
	}
	if len(rounds) == 0 {
		summary.RemainingAfter = remaining
		summary.When = time.Now()
	}
	if resultLogDir != "" {
		if err := logging.AppendDifficultySummary(resultLogDir, summary); err != nil {
			fmt.Printf("账号 %s 写入难度汇总失败：%v\n", authState.account.Email, err)
		}
	}
	return summary, rounds
}

func mergeRoundIntoSummary(summary model.AccountRunSummary, round model.RoundResultSummary) model.AccountRunSummary {
	if strings.TrimSpace(summary.Email) == "" {
		summary.Email = round.Email
		summary.Difficulty = round.Difficulty
	}
	summary.Played++
	summary.TotalReward += round.Reward
	summary.RemainingAfter = round.RemainingAfter
	summary.When = round.When
	if round.BalanceAfter != nil {
		summary.BalanceAfter = float64Ptr(*round.BalanceAfter)
	}
	if round.Err != nil {
		summary.Err = round.Err
		summary.Failed++
		return summary
	}
	switch round.Status {
	case "won":
		summary.Won++
	case "abandoned":
		summary.Abandoned++
	default:
		summary.Failed++
	}
	return summary
}

func playRoundWithContext(ctx context.Context, apiClient *client.APIClient, authState *runtimeAuthState, start *model.StartResponse, dryRun bool, resultLogDir string, pacer *actionPacer, continued bool, progress roundProgress) (model.RoundResultSummary, error) {
	snapshot := snapshotFromStartResponse(start)
	startedAt := time.Now()
	usedPowerups := make([]string, 0)
	for {
		if err := terminal.Check(ctx); err != nil {
			result := buildRoundError(authState.account.Email, snapshot, continued, dryRun, usedPowerups, startedAt, progress, err)
			if resultLogDir != "" {
				_ = logging.AppendRoundResult(resultLogDir, result)
			}
			return result, err
		}
		if snapshot.Status == "won" || (len(snapshot.Tiles) == 0 && len(snapshot.SlotTiles) == 0) {
			remaining, remainingErr := getRemainingPlays(apiClient, authState, snapshot.Difficulty)
			balance := extractBalanceFromStep(nil)
			result := model.RoundResultSummary{
				Email:          authState.account.Email,
				Difficulty:     snapshot.Difficulty,
				RoundIndex:     progress.current,
				RoundTotal:     progress.total,
				SessionID:      snapshot.SessionID,
				Continued:      continued,
				DryRun:         dryRun,
				Status:         "won",
				Reward:         0,
				BalanceAfter:   balance,
				RemainingAfter: remaining,
				MoveCount:      snapshot.MoveCount,
				UsedPowerups:   append([]string(nil), usedPowerups...),
				Duration:       time.Since(startedAt),
				When:           time.Now(),
				Err:            remainingErr,
			}
			if resultLogDir != "" {
				_ = logging.AppendRoundResult(resultLogDir, result)
			}
			return result, remainingErr
		}
		plan, err := solver.PlanToToolBoundary(snapshot)
		if err != nil {
			result := buildRoundError(authState.account.Email, snapshot, continued, dryRun, usedPowerups, startedAt, progress, err)
			if resultLogDir != "" {
				_ = logging.AppendRoundResult(resultLogDir, result)
			}
			return result, err
		}
		if len(plan.Actions) == 0 {
			result := buildRoundError(authState.account.Email, snapshot, continued, dryRun, usedPowerups, startedAt, progress, fmt.Errorf("当前整局无法生成可执行计划"))
			if resultLogDir != "" {
				_ = logging.AppendRoundResult(resultLogDir, result)
			}
			return result, result.Err
		}
		for _, action := range plan.Actions {
			if dryRun {
				var err error
				snapshot, err = applyPlannedActionLocally(snapshot, action)
				if err != nil {
					result := buildRoundError(authState.account.Email, snapshot, continued, dryRun, usedPowerups, startedAt, progress, err)
					if resultLogDir != "" {
						_ = logging.AppendRoundResult(resultLogDir, result)
					}
					return result, err
				}
				if action.Kind != "click" {
					usedPowerups = append(usedPowerups, action.Kind)
				}
				continue
			}
			if pacer != nil {
				pacer.Wait()
			}
			if err := terminal.Check(ctx); err != nil {
				result := buildRoundError(authState.account.Email, snapshot, continued, dryRun, usedPowerups, startedAt, progress, err)
				if resultLogDir != "" {
					_ = logging.AppendRoundResult(resultLogDir, result)
				}
				return result, err
			}
			stepResp, err := withRuntimeAuthRetry(apiClient, authState, func(authToken string) (*model.StepResponse, error) {
				return apiClient.Step(authToken, model.StepRequest{SessionID: snapshot.SessionID, Action: action.Kind, TileID: action.TileID})
			})
			if err != nil {
				if action.Kind == "click" && isSlotFullError(err) {
					next, localErr := applyLocalClickSnapshot(snapshot, action.TileID)
					if localErr == nil {
						snapshot = next
						continue
					}
				}
				result := buildRoundError(authState.account.Email, snapshot, continued, dryRun, usedPowerups, startedAt, progress, err)
				if resultLogDir != "" {
					_ = logging.AppendRoundResult(resultLogDir, result)
				}
				return result, err
			}
			snapshot = snapshotFromStepResponse(snapshot, stepResp)
			if action.Kind != "click" {
				usedPowerups = append(usedPowerups, action.Kind)
			}
			if stepResp.Status == "won" {
				remaining, remainingErr := getRemainingPlays(apiClient, authState, snapshot.Difficulty)
				result := model.RoundResultSummary{
					Email:          authState.account.Email,
					Difficulty:     snapshot.Difficulty,
					RoundIndex:     progress.current,
					RoundTotal:     progress.total,
					SessionID:      snapshot.SessionID,
					Continued:      continued,
					DryRun:         dryRun,
					Status:         stepResp.Status,
					Reward:         stepResp.RewardAmount,
					BalanceAfter:   float64Ptr(stepResp.Balance),
					RemainingAfter: remaining,
					MoveCount:      stepResp.MoveCount,
					UsedPowerups:   append([]string(nil), usedPowerups...),
					Duration:       time.Since(startedAt),
					When:           time.Now(),
					Err:            remainingErr,
				}
				if resultLogDir != "" {
					_ = logging.AppendRoundResult(resultLogDir, result)
				}
				return result, remainingErr
			}
		}
	}
}

func buildRoundError(email string, snapshot model.SessionSnapshot, continued bool, dryRun bool, usedPowerups []string, startedAt time.Time, progress roundProgress, err error) model.RoundResultSummary {
	return model.RoundResultSummary{
		Email:        email,
		Difficulty:   snapshot.Difficulty,
		RoundIndex:   progress.current,
		RoundTotal:   progress.total,
		SessionID:    snapshot.SessionID,
		Continued:    continued,
		DryRun:       dryRun,
		Status:       snapshot.Status,
		MoveCount:    snapshot.MoveCount,
		UsedPowerups: append([]string(nil), usedPowerups...),
		Duration:     time.Since(startedAt),
		When:         time.Now(),
		Err:          err,
	}
}

func extractBalanceFromStep(resp *model.StepResponse) *float64 {
	if resp == nil {
		return nil
	}
	return float64Ptr(resp.Balance)
}

func maxInt(left int, right int) int {
	if left > right {
		return left
	}
	return right
}
