package mining

import (
	"errors"
	"fmt"
	"strings"
)

var (
	ErrChallengeRejected = errors.New("challenge_rejected")
	ErrDailyLimit        = errors.New("daily_limit")
	ErrInventoryDepleted = errors.New("inventory_depleted")
	ErrNoOpenRound       = errors.New("no_open_round")
	ErrPoolDisabled      = errors.New("pool_disabled")
	ErrRoundClosed       = errors.New("round_closed")
)

func ChallengeError(resp *ChallengeResponse) error {
	if resp == nil {
		return fmt.Errorf("%w: 挑战被矿池拒绝", ErrChallengeRejected)
	}
	if resp.Result == ResultDailyWinLimitReached || resp.Message == ResultDailyWinLimitReached {
		return ErrDailyLimit
	}
	if message := strings.TrimSpace(resp.Message); message != "" {
		return fmt.Errorf("%w: %s", ErrChallengeRejected, LocalizedMessage(message))
	}
	if result := strings.TrimSpace(resp.Result); result != "" {
		return fmt.Errorf("%w: %s", ErrChallengeRejected, ResultLabel(result))
	}
	return fmt.Errorf("%w: 挑战被矿池拒绝", ErrChallengeRejected)
}

func HumanizeError(err error) string {
	switch {
	case err == nil:
		return ""
	case errors.Is(err, ErrPoolDisabled):
		return "矿池当前未开放"
	case errors.Is(err, ErrNoOpenRound):
		return "当前没有开放轮次"
	case errors.Is(err, ErrInventoryDepleted):
		return "当前邀请码和余额兑换码库存都已耗尽"
	case errors.Is(err, ErrRoundClosed):
		return "当前轮次已关闭"
	case errors.Is(err, ErrDailyLimit):
		return "今日命中次数已达上限"
	case errors.Is(err, ErrChallengeRejected):
		prefix := ErrChallengeRejected.Error() + ":"
		message := strings.TrimSpace(err.Error())
		if strings.HasPrefix(message, prefix) {
			detail := strings.TrimSpace(strings.TrimPrefix(message, prefix))
			if detail != "" {
				return "挑战被矿池拒绝：" + detail
			}
		}
		return "挑战被矿池拒绝"
	default:
		message := strings.TrimSpace(err.Error())
		if message == "" {
			return "执行失败"
		}
		return message
	}
}

func LocalizedMessage(message string) string {
	return localizedMessage(message, "服务端返回错误")
}

func ResultLabel(result string) string {
	switch strings.ToLower(strings.TrimSpace(result)) {
	case ResultDailyWinLimitReached:
		return "今日命中次数已达上限"
	case ResultRoundClosed:
		return "轮次已关闭"
	case ResultLate:
		return "提交过晚"
	case "ok", "accepted", "success":
		return "成功"
	default:
		return fallbackVisibleText(result, "未说明")
	}
}

func PreferenceLabel(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "invite":
		return "邀请码"
	case "balance":
		return "余额兑换码"
	default:
		return fallbackVisibleText(value, "未说明")
	}
}

func CodeTypeLabel(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "invite":
		return "邀请码"
	case "balance":
		return "余额兑换码"
	case "none", "":
		return "无"
	default:
		return fallbackVisibleText(value, "未说明")
	}
}

func DropTypeLabel(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "invite":
		return "邀请码"
	case "balance":
		return "余额兑换码"
	case "fallback", "inventory_fallback":
		return "库存不足后切换"
	case "forced", "forced_balance", "forced_invite":
		return "强制指定"
	case "none", "":
		return "无"
	default:
		return fallbackVisibleText(value, "未说明")
	}
}

func ForcedByLabel(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "invite":
		return "邀请码"
	case "balance":
		return "余额兑换码"
	case "auto":
		return "自动"
	case "fallback", "inventory_fallback":
		return "库存不足后切换"
	case "none", "":
		return "无"
	default:
		return fallbackVisibleText(value, "未说明")
	}
}

func localizedMessage(message string, fallback string) string {
	trimmed := strings.TrimSpace(message)
	if trimmed == "" {
		return fallback
	}
	lower := strings.ToLower(trimmed)
	switch {
	case strings.Contains(lower, "daily win limit reached") || strings.Contains(lower, "daily limit reached"):
		return "今日命中次数已达上限"
	case strings.Contains(lower, "no open round"):
		return "当前没有开放轮次"
	case strings.Contains(lower, "round closed"):
		return "当前轮次已关闭"
	case strings.Contains(lower, "pool disabled"):
		return "矿池当前未开放"
	case strings.Contains(lower, "challenge rejected"):
		return "挑战被矿池拒绝"
	case strings.Contains(lower, "inventory depleted"):
		return "当前邀请码和余额兑换码库存都已耗尽"
	default:
		return fallbackVisibleText(trimmed, fallback)
	}
}

func fallbackVisibleText(value string, fallback string) string {
	trimmed := strings.TrimSpace(value)
	if trimmed == "" {
		return fallback
	}
	if containsASCIIAlpha(trimmed) {
		return fallback
	}
	return trimmed
}

func containsASCIIAlpha(text string) bool {
	for _, ch := range text {
		if (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') {
			return true
		}
	}
	return false
}
