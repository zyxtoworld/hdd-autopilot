package scratch

import (
	"fmt"
	"strings"
	"time"

	"hdd/internal/model"
)

func gameTypeLabel(gameType string) string {
	switch gameType {
	case model.ScratchGameTypeLuckyNumbers:
		return "幸运数字"
	case model.ScratchGameTypeThreeKind:
		return "三连相同"
	case model.ScratchGameTypeIconMatch:
		return "图标配对"
	case model.ScratchGameTypeTreasureChest:
		return "宝箱开奖"
	case model.ScratchGameTypeProgressRun:
		return "进度冲刺"
	default:
		if strings.TrimSpace(gameType) == "" {
			return "未知玩法"
		}
		return gameType
	}
}

func playStatusLabel(status string) string {
	switch strings.ToLower(strings.TrimSpace(status)) {
	case "pending":
		return "等待开奖"
	case "played":
		return "已开局"
	case "revealed":
		return "已开奖"
	default:
		if strings.TrimSpace(status) == "" {
			return "状态未知"
		}
		return status
	}
}

func roundOutcomeLabel(result model.ScratchRoundResult) string {
	reward := roundReward(result)
	switch {
	case result.RevealResp == nil && result.RevealHistoryItem == nil:
		return "本局未完成"
	case reward > 0:
		return "中奖了"
	default:
		return "未中奖"
	}
}

func roundReward(result model.ScratchRoundResult) float64 {
	if result.RevealHistoryItem != nil {
		return result.RevealHistoryItem.RewardAmount
	}
	if result.RevealResp != nil {
		return result.RevealResp.RewardAmount
	}
	return 0
}

func PrintRoundResult(printf func(string, ...any), result model.ScratchRoundResult, totalCost float64, totalReward float64) {
	printf("\n===== 第 %d 轮结束，用时 %s =====\n", result.Round, result.Duration.Round(time.Millisecond))

	balanceSuffix := roundBalanceSuffix(result)
	if result.PlayErr != nil {
		printf("这一轮开局失败了：%v\n", result.PlayErr)
		printStats(printf, totalCost, totalReward, balanceSuffix)
		return
	}

	printPlayResult(printf, result.PlayResp)
	if result.PlayHistoryErr != nil {
		printf("开局记录同步失败：%v\n", result.PlayHistoryErr)
		printStats(printf, totalCost, totalReward, balanceSuffix)
		return
	}
	if result.RevealErr != nil {
		printf("这一轮开奖失败了：%v\n", result.RevealErr)
		printStats(printf, totalCost, totalReward, balanceSuffix)
		return
	}

	printRevealResult(printf, result.RevealResp)
	if result.RevealHistoryErr != nil {
		printf("开奖记录同步失败：%v\n", result.RevealHistoryErr)
	}
	printStats(printf, totalCost, totalReward, balanceSuffix)
}

func printPlayResult(printf func(string, ...any), playResp *model.ScratchPlayResponse) {
	printf("这一轮已经开局：玩法是%s，对局编号是 %d，目前状态是%s，这一局花了 %.2f，当前余额 %.8f。\n",
		gameTypeLabel(playResp.GameType),
		playResp.PlayID,
		playStatusLabel(playResp.Status),
		playResp.CostAmount,
		playResp.Balance,
	)
}

func printRevealResult(printf func(string, ...any), revealResp *model.ScratchRevealResponse) {
	printf("这一轮已经开奖：玩法是%s，对局编号是 %d，结果是%s，奖金 %.2f，净收益 %.2f，当前余额 %.8f。\n",
		gameTypeLabel(revealResp.GameType),
		revealResp.PlayID,
		roundOutcomeLabel(model.ScratchRoundResult{RevealResp: revealResp}),
		revealResp.RewardAmount,
		revealResp.NetAmount,
		revealResp.Balance,
	)
}

func printStats(printf func(string, ...any), totalCost float64, totalReward float64, balanceSuffix string) {
	printf("累计情况：到现在一共花了 %.2f，一共中了 %.2f，净收益 %.2f%s。\n", totalCost, totalReward, totalReward-totalCost, balanceSuffix)
}

func roundBalanceSuffix(result model.ScratchRoundResult) string {
	currentBalance, ok := roundBalance(result)
	if !ok {
		return ""
	}
	return fmt.Sprintf("，当前余额 %.8f", currentBalance)
}

func roundBalance(result model.ScratchRoundResult) (float64, bool) {
	if result.RevealResp != nil {
		return result.RevealResp.Balance, true
	}
	if result.PlayResp != nil {
		return result.PlayResp.Balance, true
	}
	return 0, false
}
