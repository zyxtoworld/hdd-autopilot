package mining

import (
	"runtime"
	"time"

	rootconfig "hdd/internal/config"
)

type Mode string

const (
	ModeInviteThenBalance Mode = "invite_then_balance"
	ModeBalanceThenInvite Mode = "balance_then_invite"
	ModeInviteOnly        Mode = "invite_only"
	ModeBalanceOnly       Mode = "balance_only"
)

type Config struct {
	BaseURL                 string
	InviteOutputFile        string
	BalanceOutputFile       string
	ThreadCount             int
	HTTPTimeout             time.Duration
	HeartbeatInterval       time.Duration
	ProgressInterval        time.Duration
	RetryDelay              time.Duration
	SuccessDelay            time.Duration
	DailyLimitDelay         time.Duration
	InventoryDepletedDelay  time.Duration
	RoundStatusPollInterval time.Duration
	Mode                    Mode
}

func normalizeMode(mode Mode) Mode {
	switch mode {
	case ModeInviteThenBalance, ModeBalanceThenInvite, ModeInviteOnly, ModeBalanceOnly:
		return mode
	default:
		return ModeInviteThenBalance
	}
}

func defaultConfig(threadCount int, mode Mode) Config {
	mode = normalizeMode(mode)
	return Config{
		BaseURL:                 "https://sub.hdd.sb",
		InviteOutputFile:        rootconfig.ResolveDataFilePath("log/mining/system/invite-codes.txt"),
		BalanceOutputFile:       rootconfig.ResolveDataFilePath("log/mining/system/balance-codes.txt"),
		ThreadCount:             threadCount,
		HTTPTimeout:             30 * time.Second,
		HeartbeatInterval:       4 * time.Second,
		ProgressInterval:        10 * time.Second,
		RetryDelay:              3 * time.Second,
		SuccessDelay:            3 * time.Second,
		DailyLimitDelay:         60 * time.Second,
		InventoryDepletedDelay:  60 * time.Second,
		RoundStatusPollInterval: 500 * time.Millisecond,
		Mode:                    mode,
	}
}

func defaultThreadCount() int {
	return runtime.NumCPU()
}

func modeLabel(mode Mode) string {
	switch normalizeMode(mode) {
	case ModeInviteThenBalance:
		return "先挖邀请码再挖余额码"
	case ModeBalanceThenInvite:
		return "先挖余额码再挖邀请码"
	case ModeInviteOnly:
		return "只挖邀请码"
	case ModeBalanceOnly:
		return "只挖余额码"
	default:
		return "先挖邀请码再挖余额码"
	}
}

func modeDescription(mode Mode) string {
	switch normalizeMode(mode) {
	case ModeInviteThenBalance:
		return "先尝试邀请码，不够时再切换到余额兑换码"
	case ModeBalanceThenInvite:
		return "先尝试余额兑换码，不够时再切换到邀请码"
	case ModeInviteOnly:
		return "只尝试邀请码"
	case ModeBalanceOnly:
		return "只尝试余额兑换码"
	default:
		return "先尝试邀请码，不够时再切换到余额兑换码"
	}
}
