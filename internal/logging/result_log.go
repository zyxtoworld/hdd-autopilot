package logging

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unicode"

	"hdd/internal/model"
)

var cstZone = time.FixedZone("UTC+8", 8*60*60)

func AppendRunHeader(logDir string, email string, when time.Time) error {
	line := fmt.Sprintf("[%s] 开始运行，正在处理账号 %s。\n", when.In(cstZone).Format("2006-01-02 15:04:05"), email)
	return appendLine(logDir, email, line)
}

func AppendRoundResult(logDir string, result model.RoundResultSummary) error {
	return appendLine(logDir, result.Email, formatRoundResultLine(result))
}

func AppendDifficultySummary(logDir string, summary model.AccountRunSummary) error {
	return appendLine(logDir, summary.Email, formatDifficultySummaryLine(summary))
}

func AppendAccountSummary(logDir string, email string, when time.Time, summaries []model.AccountRunSummary) error {
	if len(summaries) == 0 {
		return nil
	}
	var builder strings.Builder
	totalPlayed := 0
	totalWon := 0
	totalAbandoned := 0
	totalFailed := 0
	totalReward := 0.0
	var balanceAfter *float64
	for _, summary := range summaries {
		totalPlayed += summary.Played
		totalWon += summary.Won
		totalAbandoned += summary.Abandoned
		totalFailed += summary.Failed
		totalReward += summary.TotalReward
		if summary.BalanceAfter != nil {
			balanceAfter = float64Ptr(*summary.BalanceAfter)
		}
		builder.WriteString(joinClauses(
			fmt.Sprintf("[%s] 账号 %s 的%s难度汇总：一共玩了 %d 局", when.In(cstZone).Format("2006-01-02 15:04:05"), email, difficultyLabel(summary.Difficulty), summary.Played),
			fmt.Sprintf("成功 %d 局", summary.Won),
			fmt.Sprintf("失败 %d 局", summary.Failed),
			fmt.Sprintf("放弃 %d 局", summary.Abandoned),
			fmt.Sprintf("总收益 %.8f", summary.TotalReward),
			fmt.Sprintf("今天这个难度还剩 %d 次", summary.RemainingAfter),
			formatBalanceClause(summary.BalanceAfter),
			formatReasonClause(summary.Err),
		))
	}
	builder.WriteString(joinClauses(
		fmt.Sprintf("[%s] 账号 %s 的全部难度汇总：一共玩了 %d 局", when.In(cstZone).Format("2006-01-02 15:04:05"), email, totalPlayed),
		fmt.Sprintf("成功 %d 局", totalWon),
		fmt.Sprintf("失败 %d 局", totalFailed),
		fmt.Sprintf("放弃 %d 局", totalAbandoned),
		fmt.Sprintf("总收益 %.8f", totalReward),
		formatBalanceClause(balanceAfter),
	))
	return appendLine(logDir, email, builder.String())
}

func LogFilePath(logDir string, email string) string {
	name := sanitizeEmail(email)
	if name == "" {
		name = "unknown"
	}
	return filepath.Join(logDir, name+".log")
}

func formatRoundResultLine(result model.RoundResultSummary) string {
	return joinClauses(
		fmt.Sprintf("[%s] %s 的%s难度第 %d 局（%s，对局 %d）已结算：%s", result.When.In(cstZone).Format("2006-01-02 15:04:05"), result.Email, difficultyLabel(result.Difficulty), result.RoundIndex, roundModeLabel(result.Continued), result.SessionID, roundResultLabel(result)),
		fmt.Sprintf("收益 %.8f", result.Reward),
		formatBalanceClause(result.BalanceAfter),
		fmt.Sprintf("今天这个难度还剩 %d 次", result.RemainingAfter),
		fmt.Sprintf("这一局走了 %d 步", result.MoveCount),
		formatPowerupsClause(result.UsedPowerups),
		fmt.Sprintf("耗时 %s", result.Duration.Round(time.Millisecond)),
		formatReasonClause(result.Err),
	)
}

func formatDifficultySummaryLine(summary model.AccountRunSummary) string {
	return joinClauses(
		fmt.Sprintf("[%s] %s 的%s难度已跑完：一共玩了 %d 局", summary.When.In(cstZone).Format("2006-01-02 15:04:05"), summary.Email, difficultyLabel(summary.Difficulty), summary.Played),
		fmt.Sprintf("成功 %d 局", summary.Won),
		fmt.Sprintf("放弃 %d 局", summary.Abandoned),
		fmt.Sprintf("失败 %d 局", summary.Failed),
		fmt.Sprintf("总收益 %.8f", summary.TotalReward),
		fmt.Sprintf("今天这个难度还剩 %d 次", summary.RemainingAfter),
		formatBalanceClause(summary.BalanceAfter),
		formatReasonClause(summary.Err),
	)
}

func roundModeLabel(continued bool) string {
	if continued {
		return "续玩"
	}
	return "新开局"
}

func roundResultLabel(result model.RoundResultSummary) string {
	if result.Err != nil {
		return "失败"
	}
	switch result.Status {
	case "won":
		if result.DryRun {
			return "演练成功"
		}
		return "成功通关"
	case "abandoned":
		if result.DryRun {
			return "演练放弃"
		}
		return "已放弃"
	case "undo":
		return "用到撤回后暂停重算"
	case "remove":
		return "用到移除后暂停重算"
	case "shuffle":
		return "用到洗牌后暂停重算"
	default:
		if result.DryRun {
			return "演练结束"
		}
		if strings.TrimSpace(result.Status) == "" {
			return "已结束"
		}
		return result.Status
	}
}

func difficultyLabel(difficulty string) string {
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

func formatBalanceClause(balance *float64) string {
	if balance == nil {
		return ""
	}
	return fmt.Sprintf("当前余额 %.8f", *balance)
}

func formatPowerupsClause(usedPowerups []string) string {
	if len(usedPowerups) == 0 {
		return "这局没用道具"
	}
	labels := make([]string, 0, len(usedPowerups))
	for _, item := range usedPowerups {
		labels = append(labels, powerupLabel(item))
	}
	return fmt.Sprintf("用到的道具：%s", strings.Join(labels, "、"))
}

func powerupLabel(kind string) string {
	switch strings.TrimSpace(strings.ToLower(kind)) {
	case "undo":
		return "撤回"
	case "remove":
		return "移除"
	case "shuffle":
		return "洗牌"
	case "abandon":
		return "放弃"
	default:
		return kind
	}
}

func formatReasonClause(err error) string {
	if err == nil {
		return ""
	}
	message := strings.TrimSpace(err.Error())
	if message == "" {
		return "原因：执行失败"
	}
	return fmt.Sprintf("原因：%s", message)
}

func joinClauses(clauses ...string) string {
	parts := make([]string, 0, len(clauses))
	for _, clause := range clauses {
		clause = strings.TrimSpace(clause)
		if clause == "" {
			continue
		}
		parts = append(parts, clause)
	}
	if len(parts) == 0 {
		return ""
	}
	return strings.Join(parts, "，") + "。\n"
}

func appendLine(logDir string, email string, content string) error {
	if err := os.MkdirAll(logDir, 0700); err != nil {
		return err
	}
	path := LogFilePath(logDir, email)
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0600)
	if err != nil {
		return err
	}
	defer file.Close()
	if _, err := file.WriteString(content); err != nil {
		return err
	}
	return file.Sync()
}

func sanitizeEmail(email string) string {
	email = strings.TrimSpace(strings.ToLower(email))
	if email == "" {
		return ""
	}
	var builder strings.Builder
	for _, r := range email {
		switch {
		case unicode.IsLetter(r), unicode.IsDigit(r):
			builder.WriteRune(r)
		case r == '.', r == '_', r == '-', r == '@':
			builder.WriteRune(r)
		default:
			builder.WriteByte('_')
		}
	}
	return strings.ReplaceAll(builder.String(), "@", "_at_")
}

func float64Ptr(value float64) *float64 {
	result := value
	return &result
}
