package scratch

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"hdd/internal/logging"
	"hdd/internal/model"
)

func AppendScratchRoundLog(logDir string, email string, result model.ScratchRoundResult, totalCost float64, totalReward float64) error {
	if logDir == "" {
		return nil
	}
	return appendScratchLogLine(logDir, email, formatScratchRoundLogLine(result, totalCost, totalReward))
}

func appendScratchLogLine(logDir string, email string, line string) error {
	path := logging.LogFilePath(logDir, email)
	if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		return err
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0600)
	if err != nil {
		return err
	}
	defer file.Close()
	if _, err := file.WriteString(line); err != nil {
		return err
	}
	return file.Sync()
}

func formatScratchRoundLogLine(result model.ScratchRoundResult, totalCost float64, totalReward float64) string {
	balanceSuffix := roundBalanceSuffix(result)
	if result.PlayErr != nil {
		return fmt.Sprintf("===== 第 %d 轮结束，用时 %s =====\n这一轮开局失败了：%v\n累计情况：到现在一共花了 %.2f，一共中了 %.2f，净收益 %.2f%s。\n",
			result.Round,
			result.Duration.Round(time.Millisecond),
			result.PlayErr,
			totalCost,
			totalReward,
			totalReward-totalCost,
			balanceSuffix,
		)
	}
	line := fmt.Sprintf("===== 第 %d 轮结束，用时 %s =====\n", result.Round, result.Duration.Round(time.Millisecond))
	if result.PlayResp != nil {
		line += fmt.Sprintf("这一轮已经开局：玩法是%s，对局编号是 %d，目前状态是%s，这一局花了 %.2f，当前余额 %.8f。\n",
			gameTypeLabel(result.PlayResp.GameType),
			result.PlayResp.PlayID,
			playStatusLabel(result.PlayResp.Status),
			result.PlayResp.CostAmount,
			result.PlayResp.Balance,
		)
	}
	if result.PlayHistoryErr != nil {
		line += fmt.Sprintf("开局记录同步失败：%v\n", result.PlayHistoryErr)
		line += fmt.Sprintf("累计情况：到现在一共花了 %.2f，一共中了 %.2f，净收益 %.2f%s。\n", totalCost, totalReward, totalReward-totalCost, balanceSuffix)
		return line
	}
	if result.RevealErr != nil {
		line += fmt.Sprintf("这一轮开奖失败了：%v\n", result.RevealErr)
		line += fmt.Sprintf("累计情况：到现在一共花了 %.2f，一共中了 %.2f，净收益 %.2f%s。\n", totalCost, totalReward, totalReward-totalCost, balanceSuffix)
		return line
	}
	if result.RevealResp != nil {
		line += fmt.Sprintf("这一轮已经开奖：玩法是%s，对局编号是 %d，结果是%s，奖金 %.2f，净收益 %.2f，当前余额 %.8f。\n",
			gameTypeLabel(result.RevealResp.GameType),
			result.RevealResp.PlayID,
			roundOutcomeLabel(model.ScratchRoundResult{RevealResp: result.RevealResp}),
			result.RevealResp.RewardAmount,
			result.RevealResp.NetAmount,
			result.RevealResp.Balance,
		)
	}
	if result.RevealHistoryErr != nil {
		line += fmt.Sprintf("开奖记录同步失败：%v\n", result.RevealHistoryErr)
	}
	line += fmt.Sprintf("累计情况：到现在一共花了 %.2f，一共中了 %.2f，净收益 %.2f%s。\n", totalCost, totalReward, totalReward-totalCost, balanceSuffix)
	return line
}
