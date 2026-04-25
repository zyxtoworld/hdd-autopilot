package model

import "time"

func ScratchTicketReward(payload ScratchTicketPayload) float64 {
	if payload.RewardAmount == nil {
		return 0
	}
	return *payload.RewardAmount
}

func ScratchRevealReadyAt(playResp *ScratchPlayResponse) time.Time {
	if playResp == nil {
		return time.Time{}
	}
	if playResp.EarliestRevealAtMs > 0 {
		return time.UnixMilli(playResp.EarliestRevealAtMs)
	}
	if playResp.IssuedAtMs > 0 && playResp.MinScratchMs > 0 {
		return time.UnixMilli(playResp.IssuedAtMs).Add(time.Duration(playResp.MinScratchMs) * time.Millisecond)
	}
	return time.Time{}
}

func ScratchCountMatchedNumbers(numbers []ScratchNumber) int {
	matched := 0
	for _, n := range numbers {
		if n.Matched {
			matched++
		}
	}
	return matched
}
