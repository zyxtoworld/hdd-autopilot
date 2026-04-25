package mining

func (r *Runner) checkRoundStatus(challenge *ChallengeResponse, target rewardKind) (roundStatus, error) {
	statusResp, err := r.poolClient.GetStatusSnapshot()
	if err != nil {
		return roundStatus{}, err
	}
	if !statusResp.Enabled || statusResp.CurrentRound == nil || !statusResp.CurrentRound.IsOpen() {
		return roundStatus{roundClosed: true}, nil
	}
	if statusResp.CurrentRound.ID != challenge.RoundID {
		return roundStatus{roundClosed: true}, nil
	}
	if target.remaining(statusResp) <= 0 {
		return roundStatus{inventoryDepleted: true}, nil
	}
	if statusResp.DailyLimitReached() {
		return roundStatus{dailyLimit: true}, nil
	}
	return roundStatus{}, nil
}

type roundStatus struct {
	roundClosed       bool
	dailyLimit        bool
	inventoryDepleted bool
}

func jobConfigFromChallenge(challenge *ChallengeResponse) JobConfig {
	return JobConfig{
		ChallengeID:    challenge.ChallengeID,
		DifficultyBits: challenge.DifficultyBits,
		MemoryCostMB:   challenge.MemoryCostMB,
		Parallelism:    challenge.Parallelism,
		RoundID:        challenge.RoundID,
		Seed:           challenge.Seed,
		SessionSalt:    challenge.SessionSalt,
		TimeCost:       challenge.TimeCost,
		VisitorID:      challenge.VisitorID,
	}
}
