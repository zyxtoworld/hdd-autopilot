package mining

const (
	ResultDailyWinLimitReached = "daily win limit reached"
	ResultRoundClosed          = "round_closed"
	ResultLate                 = "late"
	RoundStatusOpen            = "open"
)

type PoolStats struct {
	BalanceUnused int `json:"balance_unused"`
	InviteUnused  int `json:"invite_unused"`
}

type CurrentRound struct {
	ID             int    `json:"id"`
	DifficultyBits int    `json:"difficulty_bits"`
	ExpiresAt      string `json:"expires_at"`
	MemoryCostMB   int    `json:"memory_cost_mb"`
	Parallelism    int    `json:"parallelism"`
	RoundKey       string `json:"round_key"`
	Seed           string `json:"seed"`
	Status         string `json:"status"`
	TimeCost       int    `json:"time_cost"`
}

func (r *CurrentRound) IsOpen() bool {
	return r != nil && (r.Status == "" || r.Status == RoundStatusOpen)
}

type StatusResponse struct {
	AdminLock          string        `json:"admin_lock"`
	CurrentRound       *CurrentRound `json:"current_round"`
	DailyDropRemaining *int          `json:"daily_drop_remaining"`
	DesktopOnly        bool          `json:"desktop_only"`
	Enabled            bool          `json:"enabled"`
	InventoryRemaining int           `json:"inventory_remaining"`
	PoolStats          *PoolStats    `json:"pool_stats"`
	Result             string        `json:"result"`
	ServerTime         string        `json:"server_time"`
}

func (r *StatusResponse) DailyLimitReached() bool {
	if r == nil {
		return false
	}
	if r.Result == ResultDailyWinLimitReached {
		return true
	}
	return r.DailyDropRemaining != nil && *r.DailyDropRemaining <= 0
}

func (r *StatusResponse) BalanceInventoryRemaining() int {
	if r == nil {
		return 0
	}
	if r.PoolStats != nil {
		return r.PoolStats.BalanceUnused
	}
	return r.InventoryRemaining
}

func (r *StatusResponse) InviteInventoryRemaining() int {
	if r == nil {
		return 0
	}
	if r.PoolStats != nil {
		return r.PoolStats.InviteUnused
	}
	return r.InventoryRemaining
}

type ChallengeResponse struct {
	AdminLock      string     `json:"admin_lock"`
	ChallengeID    int        `json:"challenge_id"`
	DifficultyBits int        `json:"difficulty_bits"`
	ExpiresAt      string     `json:"expires_at"`
	MemoryCostMB   int        `json:"memory_cost_mb"`
	Message        string     `json:"message"`
	Ok             bool       `json:"ok"`
	Parallelism    int        `json:"parallelism"`
	PoolStats      *PoolStats `json:"pool_stats"`
	Result         string     `json:"result"`
	RoundID        int        `json:"round_id"`
	Seed           string     `json:"seed"`
	SessionSalt    string     `json:"session_salt"`
	TimeCost       int        `json:"time_cost"`
	VisitorID      string     `json:"visitor_id"`
}

type HeartbeatRequest struct {
	ChallengeID int `json:"challenge_id"`
	RoundID     int `json:"round_id"`
}

type HeartbeatResponse struct {
	Result string `json:"result"`
}

type SubmitRequest struct {
	ChallengeID int    `json:"challenge_id"`
	RoundID     int    `json:"round_id"`
	Nonce       string `json:"nonce"`
	Digest      string `json:"digest"`
	Preference  string `json:"preference"`
}

type SubmitResponse struct {
	BalanceAmount float64 `json:"balance_amount"`
	CodeType      string  `json:"code_type"`
	DropType      string  `json:"drop_type"`
	ForcedBy      string  `json:"forced_by"`
	RewardCode    string  `json:"invite_code"`
	Ok            bool    `json:"ok"`
	Result        string  `json:"result"`
	RewardCodeID  int     `json:"reward_code_id"`
}
