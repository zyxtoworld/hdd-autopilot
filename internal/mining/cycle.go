package mining

import (
	"fmt"
	"log"
	"strconv"
	"sync"
	"sync/atomic"
	"time"

	"hdd/internal/terminal"
)

func (r *Runner) runCycle(threadCount int) error {
	if err := terminal.Check(r.ctx); err != nil {
		return err
	}
	log.Printf("获取矿池状态...")
	statusResp, err := r.poolClient.GetStatus()
	if err != nil {
		switch err {
		case ErrPoolDisabled, ErrNoOpenRound:
			return err
		default:
			return fmt.Errorf("获取状态失败: %w", err)
		}
	}

	inviteRemaining := statusResp.InviteInventoryRemaining()
	balanceRemaining := statusResp.BalanceInventoryRemaining()
	log.Printf("当前轮次 #%d，难度 %d，剩余邀请码 %d，剩余余额兑换码 %d", statusResp.CurrentRound.ID, statusResp.CurrentRound.DifficultyBits, inviteRemaining, balanceRemaining)

	target, ok := r.selectRewardKind(statusResp)
	if !ok {
		return ErrInventoryDepleted
	}
	if target.preference == "balance" && inviteRemaining <= 0 && normalizeMode(r.cfg.Mode) != ModeBalanceOnly {
		log.Printf("邀请码库存为 0，切换到余额兑换码")
	}
	if target.preference == "invite" && balanceRemaining <= 0 && normalizeMode(r.cfg.Mode) == ModeInviteThenBalance {
		log.Printf("余额兑换码库存为 0，继续尝试邀请码")
	}
	log.Printf("本轮选择: %s", target.name)
	log.Printf("获取挑战...")
	challenge, err := r.poolClient.GetChallenge()
	if err != nil {
		if err == ErrDailyLimit {
			return err
		}
		return fmt.Errorf("获取挑战失败: %w", err)
	}

	log.Printf("挑战 #%d，轮次 #%d，难度 %d", challenge.ChallengeID, challenge.RoundID, challenge.DifficultyBits)
	job := NewJob(jobConfigFromChallenge(challenge))

	stopMining := &atomic.Bool{}
	roundClosed := &atomic.Bool{}
	dailyLimit := &atomic.Bool{}
	inventoryDepleted := &atomic.Bool{}
	stopHeartbeat := make(chan struct{})
	statusSignal := make(chan struct{}, 1)
	signalStatus := func() {
		select {
		case statusSignal <- struct{}{}:
		default:
		}
	}

	go func() {
		ticker := time.NewTicker(r.cfg.HeartbeatInterval)
		defer ticker.Stop()
		for {
			select {
			case <-r.ctx.Done():
				stopMining.Store(true)
				signalStatus()
				return
			case <-ticker.C:
				if stopMining.Load() {
					return
				}
				_, err := r.poolClient.Heartbeat(HeartbeatRequest{ChallengeID: challenge.ChallengeID, RoundID: challenge.RoundID})
				if err != nil {
					if err == ErrRoundClosed {
						log.Printf("心跳检测到轮次已关闭，停止挖矿")
						roundClosed.Store(true)
						stopMining.Store(true)
						signalStatus()
						return
					}
					log.Printf("心跳失败: %v", err)
					continue
				}
				status, err := r.checkRoundStatus(challenge, target)
				if err != nil {
					continue
				}
				if status.inventoryDepleted {
					log.Printf("%s已耗尽，停止当前挖矿", target.name)
					inventoryDepleted.Store(true)
					stopMining.Store(true)
					signalStatus()
					return
				}
				if status.roundClosed {
					log.Printf("轮次已变更，停止挖矿")
					roundClosed.Store(true)
					stopMining.Store(true)
					signalStatus()
					return
				}
				if status.dailyLimit {
					log.Printf("今日命中次数已达上限")
					dailyLimit.Store(true)
					stopMining.Store(true)
					signalStatus()
					return
				}
			case <-stopHeartbeat:
				return
			}
		}
	}()

	log.Printf("开始挖矿，%d 线程并行...", threadCount)
	var totalAttempts atomic.Int64
	resultCh := make(chan Result, threadCount)

	var wg sync.WaitGroup
	for i := 0; i < threadCount; i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			result := MineWorker(job, idx, threadCount, stopMining, &totalAttempts)
			if result.Digest != "" {
				resultCh <- result
			}
		}(i)
	}

	progressDone := &atomic.Bool{}
	go func() {
		ticker := time.NewTicker(r.cfg.ProgressInterval)
		defer ticker.Stop()
		for {
			select {
			case <-r.ctx.Done():
				return
			case <-ticker.C:
				if progressDone.Load() {
					return
				}
				log.Printf("已尝试 %d 次...", totalAttempts.Load())
			}
		}
	}()

	select {
	case <-r.ctx.Done():
		progressDone.Store(true)
		stopMining.Store(true)
		close(stopHeartbeat)
		wg.Wait()
		return terminal.ErrInterrupted
	case result := <-resultCh:
		progressDone.Store(true)
		close(stopHeartbeat)
		log.Printf("找到解！nonce=%d, digest=%s, 总尝试 %d 次", result.Nonce, result.Digest, totalAttempts.Load())
		log.Printf("提交结果...")
		submitResp, err := r.poolClient.Submit(SubmitRequest{ChallengeID: challenge.ChallengeID, RoundID: challenge.RoundID, Nonce: strconv.Itoa(result.Nonce), Digest: result.Digest, Preference: target.preference})
		if err != nil {
			wg.Wait()
			return fmt.Errorf("提交失败: %w", err)
		}
		log.Printf("提交结果已经返回：这次想要的是%s，实际拿到的是%s，结果是%s，余额面额 %.2f，奖励编号 %d", PreferenceLabel(target.preference), CodeTypeLabel(submitResp.CodeType), ResultLabel(submitResp.Result), submitResp.BalanceAmount, submitResp.RewardCodeID)
		if submitResp.Result == ResultLate || submitResp.Result == ResultRoundClosed {
			log.Printf("提交太晚，轮次已关闭")
			wg.Wait()
			return nil
		}
		if submitResp.Result == ResultDailyWinLimitReached {
			wg.Wait()
			return ErrDailyLimit
		}
		if submitResp.RewardCode != "" {
			log.Printf("命中了%s：%s", target.name, submitResp.RewardCode)
			if err := target.save(submitResp.RewardCode); err != nil {
				log.Printf("无法保存%s: %v", target.name, err)
			} else {
				log.Printf("%s已保存到 %s: %s", target.name, target.outputPath(), submitResp.RewardCode)
			}
		}
		if dailyLimit.Load() {
			wg.Wait()
			return ErrDailyLimit
		}
		if inventoryDepleted.Load() {
			wg.Wait()
			return errRetryNow
		}
	case <-statusSignal:
		progressDone.Store(true)
		stopMining.Store(true)
		close(stopHeartbeat)
		if dailyLimit.Load() {
			wg.Wait()
			return ErrDailyLimit
		}
		if inventoryDepleted.Load() {
			wg.Wait()
			return errRetryNow
		}
		if roundClosed.Load() {
			log.Printf("轮次已被别人命中，切换新轮次...")
		}
	}

	wg.Wait()
	return nil
}
