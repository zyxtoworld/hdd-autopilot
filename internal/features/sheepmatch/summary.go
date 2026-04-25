package sheepmatch

import (
	"strings"

	"hdd/internal/model"
)

func summarizeRoundsByDifficulty(email string, rounds []model.RoundResultSummary) map[string]model.AccountRunSummary {
	statsByDifficulty := make(map[string]model.AccountRunSummary)
	for _, round := range rounds {
		summary := statsByDifficulty[round.Difficulty]
		if strings.TrimSpace(summary.Email) == "" {
			summary.Email = email
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
		} else {
			switch round.Status {
			case "won":
				summary.Won++
			case "abandoned":
				summary.Abandoned++
			default:
				summary.Failed++
			}
		}
		statsByDifficulty[round.Difficulty] = summary
	}
	return statsByDifficulty
}

func summariesFromMap(statsByDifficulty map[string]model.AccountRunSummary) []model.AccountRunSummary {
	stats := make([]model.AccountRunSummary, 0, len(statsByDifficulty))
	for _, difficulty := range model.DifficultyOrder {
		if summary, ok := statsByDifficulty[difficulty]; ok {
			stats = append(stats, summary)
		}
	}
	for difficulty, summary := range statsByDifficulty {
		if containsDifficulty(model.DifficultyOrder, difficulty) {
			continue
		}
		stats = append(stats, summary)
	}
	return stats
}

func containsDifficulty(items []string, target string) bool {
	for _, item := range items {
		if item == target {
			return true
		}
	}
	return false
}

func localizedDifficulty(difficulty string) string {
	return localizeDifficulty(difficulty)
}

func localizedDifficultyList(difficulties []string) string {
	labels := make([]string, 0, len(difficulties))
	for _, difficulty := range difficulties {
		labels = append(labels, localizedDifficulty(difficulty))
	}
	return strings.Join(labels, "、")
}

func localizeDifficulty(difficulty string) string {
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

func roundStatusLabel(result model.RoundResultSummary) string {
	if result.Err != nil {
		return "失败"
	}
	if strings.TrimSpace(result.Status) == "" {
		return "已结束"
	}
	return loggingRoundLabel(result)
}

func loggingRoundLabel(result model.RoundResultSummary) string {
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
		return result.Status
	}
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

func float64Ptr(value float64) *float64 {
	result := value
	return &result
}
