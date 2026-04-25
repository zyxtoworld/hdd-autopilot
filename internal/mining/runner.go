package mining

import (
	"context"
	"errors"
	"log"
	"runtime"

	"hdd/internal/terminal"
)

var errRetryNow = errors.New("retry_now")

type rewardKind struct {
	name       string
	preference string
	remaining  func(*StatusResponse) int
	save       func(string) error
	outputPath func() string
}

type Runner struct {
	ctx              context.Context
	cfg              Config
	poolClient       *Client
	inviteStore      *InviteStore
	balanceCodeStore *BalanceCodeStore
	rewardKinds      []rewardKind
}

func NewRunner(ctx context.Context, cfg Config, poolClient *Client, inviteStore *InviteStore, balanceCodeStore *BalanceCodeStore) *Runner {
	r := &Runner{ctx: ctx, cfg: cfg, poolClient: poolClient, inviteStore: inviteStore, balanceCodeStore: balanceCodeStore}
	r.rewardKinds = rewardKindsForMode(cfg.Mode, inviteStore, balanceCodeStore)
	return r
}

func rewardKindsForMode(mode Mode, inviteStore *InviteStore, balanceCodeStore *BalanceCodeStore) []rewardKind {
	invite := rewardKind{
		name:       "邀请码",
		preference: "invite",
		remaining:  func(statusResp *StatusResponse) int { return statusResp.InviteInventoryRemaining() },
		save:       inviteStore.Save,
		outputPath: inviteStore.Path,
	}
	balance := rewardKind{
		name:       "余额兑换码",
		preference: "balance",
		remaining:  func(statusResp *StatusResponse) int { return statusResp.BalanceInventoryRemaining() },
		save:       balanceCodeStore.Save,
		outputPath: balanceCodeStore.Path,
	}
	switch normalizeMode(mode) {
	case ModeBalanceThenInvite:
		return []rewardKind{balance, invite}
	case ModeInviteOnly:
		return []rewardKind{invite}
	case ModeBalanceOnly:
		return []rewardKind{balance}
	default:
		return []rewardKind{invite, balance}
	}
}

func (r *Runner) Run() error {
	log.Printf("开始运行挖矿：当前使用 %d 个线程。", r.cfg.ThreadCount)
	log.Printf("当前模式：%s。", modeDescription(r.cfg.Mode))
	log.Printf("邀请码会写入：%s", r.inviteStore.Path())
	log.Printf("余额兑换码会写入：%s", r.balanceCodeStore.Path())
	return r.runLoop(r.cfg.ThreadCount)
}

func (r *Runner) RunAutoTuned() error {
	log.Printf("开始运行挖矿自动调优模式：先按本机推荐配置挖矿。")
	best := FindBestBenchmarkConfig()
	runtime.GOMAXPROCS(best.GOMAXPROCS)
	threadCount := best.Workers
	log.Printf("已应用推荐配置：线程数 %d，并发调度 %d，预计速度约 %.2f 次/秒。", best.Workers, best.GOMAXPROCS, best.AttemptsPerS)
	log.Printf("当前模式：%s。", modeDescription(r.cfg.Mode))
	log.Printf("邀请码会写入：%s", r.inviteStore.Path())
	log.Printf("余额兑换码会写入：%s", r.balanceCodeStore.Path())
	return r.runLoop(threadCount)
}

func (r *Runner) selectRewardKind(statusResp *StatusResponse) (rewardKind, bool) {
	for _, target := range r.rewardKinds {
		if target.remaining(statusResp) > 0 {
			return target, true
		}
	}
	return rewardKind{}, false
}

func (r *Runner) runLoop(threadCount int) error {
	for {
		if err := terminal.Check(r.ctx); err != nil {
			return err
		}
		err := r.runCycle(threadCount)
		if err != nil {
			switch {
			case errors.Is(err, terminal.ErrInterrupted):
				return err
			case errors.Is(err, errRetryNow):
				continue
			case errors.Is(err, ErrDailyLimit):
				log.Printf("今天的命中次数已经用完，%s后再试。", r.cfg.DailyLimitDelay)
				if err := terminal.SleepContext(r.ctx, r.cfg.DailyLimitDelay); err != nil {
					return err
				}
			case errors.Is(err, ErrInventoryDepleted):
				log.Printf("这一轮的目标库存已经发完了，%s后再试。", r.cfg.InventoryDepletedDelay)
				if err := terminal.SleepContext(r.ctx, r.cfg.InventoryDepletedDelay); err != nil {
					return err
				}
			default:
				log.Printf("这一轮没有顺利完成：%s。%s后自动重试。", HumanizeError(err), r.cfg.RetryDelay)
				if err := terminal.SleepContext(r.ctx, r.cfg.RetryDelay); err != nil {
					return err
				}
			}
			continue
		}
		log.Printf("本轮已经命中，等待下一轮开放。")
		if err := terminal.SleepContext(r.ctx, r.cfg.SuccessDelay); err != nil {
			return err
		}
	}
}
