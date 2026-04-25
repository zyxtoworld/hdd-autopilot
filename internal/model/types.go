package model

import "time"

const (
	DifficultyEasy   = "easy"
	DifficultyNormal = "normal"
	DifficultyHard   = "hard"
	DifficultyHell   = "hell"
)

var DifficultyOrder = []string{
	DifficultyEasy,
	DifficultyNormal,
	DifficultyHard,
	DifficultyHell,
}

type SessionCookie struct {
	Name      string `json:"name"`
	Value     string `json:"value"`
	Domain    string `json:"domain,omitempty"`
	Path      string `json:"path,omitempty"`
	ExpiresAt string `json:"expires_at,omitempty"`
	Secure    bool   `json:"secure,omitempty"`
	HttpOnly  bool   `json:"http_only,omitempty"`
}

type AuthSession struct {
	BaseURL     string          `json:"base_url"`
	TokenType   string          `json:"token_type,omitempty"`
	AccessToken string          `json:"access_token,omitempty"`
	Cookies     []SessionCookie `json:"cookies,omitempty"`
}

type AuthCache struct {
	Email       string          `json:"email"`
	Password    string          `json:"password,omitempty"`
	TokenType   string          `json:"token_type,omitempty"`
	AccessToken string          `json:"access_token,omitempty"`
	Cookies     []SessionCookie `json:"cookies,omitempty"`
	Sessions    []AuthSession   `json:"sessions,omitempty"`
}

type AuthConfig struct {
	BaseURL       string      `json:"base_url,omitempty"`
	SelectedEmail string      `json:"selected_email,omitempty"`
	Accounts      []AuthCache `json:"accounts,omitempty"`
}

type LoginRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

type LoginUser struct {
	Email string `json:"email"`
}

type LoginResponseData struct {
	AccessToken string    `json:"access_token"`
	TokenType   string    `json:"token_type"`
	User        LoginUser `json:"user"`
}

type LoginResponse struct {
	Code    int               `json:"code"`
	Message string            `json:"message"`
	Reason  string            `json:"reason"`
	Data    LoginResponseData `json:"data"`
}

type AuthMeResponse struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
	Data    struct {
		Email   string  `json:"email"`
		Balance float64 `json:"balance"`
		Status  string  `json:"status"`
	} `json:"data"`
}

type TileMeUser struct {
	Balance float64 `json:"balance"`
	Email   string  `json:"email"`
	ID      int     `json:"id"`
	Status  string  `json:"status"`
}

type TileMeResponse struct {
	ActiveSession       *HistoryItem   `json:"active_session"`
	Authenticated       bool           `json:"authenticated"`
	DailyPlaysRemaining map[string]int `json:"daily_plays_remaining"`
	DailyPlaysUsed      map[string]int `json:"daily_plays_used"`
	ServerNowMs         int64          `json:"server_now_ms"`
	User                TileMeUser     `json:"user"`
}

type Tile struct {
	GX      int    `json:"gx"`
	GY      int    `json:"gy"`
	ID      int    `json:"id"`
	Layer   int    `json:"layer"`
	Pattern string `json:"pattern"`
}

type Powerups struct {
	Remove  int `json:"remove"`
	Shuffle int `json:"shuffle"`
	Undo    int `json:"undo"`
}

type StartRequest struct {
	Difficulty string `json:"difficulty"`
}

type StartResponse struct {
	DailyPlaysRemaining map[string]int `json:"daily_plays_remaining"`
	Difficulty          string         `json:"difficulty"`
	History             []HistoryEntry `json:"history"`
	MoveCount           int            `json:"move_count"`
	OK                  bool           `json:"ok"`
	PatternCount        int            `json:"pattern_count"`
	Powerups            Powerups       `json:"powerups"`
	ServerNowMs         int64          `json:"server_now_ms"`
	ServerSeedHash      string         `json:"server_seed_hash"`
	SessionID           int            `json:"session_id"`
	SlotLimit           int            `json:"slot_limit"`
	Slots               []int          `json:"slots"`
	SlotTiles           []Tile         `json:"slot_tiles"`
	StartedAtMs         int64          `json:"started_at_ms"`
	Status              string         `json:"status"`
	Tiles               []Tile         `json:"tiles"`
	TotalTiles          int            `json:"total_tiles"`
}

type StepRequest struct {
	SessionID int    `json:"session_id"`
	Action    string `json:"action"`
	TileID    int    `json:"tile_id,omitempty"`
}

type HistoryEntry struct {
	Action      string `json:"action"`
	MoveCount   int    `json:"move_count,omitempty"`
	TileID      int    `json:"tile_id,omitempty"`
	Reversible  bool   `json:"reversible"`
	PrevSlots   []int  `json:"prev_slots,omitempty"`
	Removed     []int  `json:"removed,omitempty"`
	ReturnedIDs []int  `json:"returned_ids,omitempty"`
}

type StepResponse struct {
	Action         string         `json:"action"`
	Balance        float64        `json:"balance"`
	EndedAtMs      *int64         `json:"ended_at_ms"`
	GrantRef       string         `json:"grant_ref"`
	History        []HistoryEntry `json:"history"`
	MoveCount      int            `json:"move_count"`
	OK             bool           `json:"ok"`
	PatternMatched *string        `json:"pattern_matched"`
	Powerups       Powerups       `json:"powerups"`
	Removed        []int          `json:"removed"`
	RewardAmount   float64        `json:"reward_amount"`
	SchemaVersion  int            `json:"schema_version"`
	ServerNowMs    int64          `json:"server_now_ms"`
	ServerSeed     *string        `json:"server_seed"`
	SessionID      int            `json:"session_id"`
	SlotLimit      int            `json:"slot_limit"`
	Slots          []int          `json:"slots"`
	StartedAtMs    int64          `json:"started_at_ms"`
	Status         string         `json:"status"`
	Tiles          []Tile         `json:"tiles"`
	TotalTiles     int            `json:"total_tiles"`
}

type AbandonRequest struct {
	SessionID int `json:"session_id"`
}

type AbandonResponse struct {
	Balance        *float64       `json:"balance"`
	CreatedAt      string         `json:"created_at"`
	Difficulty     string         `json:"difficulty"`
	EndedAtMs      int64          `json:"ended_at_ms"`
	History        []HistoryEntry `json:"history"`
	MoveCount      int            `json:"move_count"`
	OK             bool           `json:"ok"`
	PatternCount   int            `json:"pattern_count"`
	Powerups       Powerups       `json:"powerups"`
	RewardAmount   float64        `json:"reward_amount"`
	SchemaVersion  int            `json:"schema_version"`
	ServerSeed     string         `json:"server_seed"`
	ServerSeedHash string         `json:"server_seed_hash"`
	SessionID      int            `json:"session_id"`
	SlotLimit      int            `json:"slot_limit"`
	Slots          []int          `json:"slots"`
	StartedAtMs    int64          `json:"started_at_ms"`
	Status         string         `json:"status"`
	Tiles          []Tile         `json:"tiles"`
	TotalTiles     int            `json:"total_tiles"`
}

type GameDifficultyConfig struct {
	DailyPlays int     `json:"daily_plays"`
	Layers     int     `json:"layers"`
	Patterns   int     `json:"patterns"`
	RewardMax  float64 `json:"reward_max"`
	RewardMin  float64 `json:"reward_min"`
	Tiles      int     `json:"tiles"`
}

type ConfigResponse struct {
	Actions           []string                        `json:"actions"`
	Difficulties      map[string]GameDifficultyConfig `json:"difficulties"`
	MaxActiveSessions int                             `json:"max_active_sessions"`
	MinIntervalMs     int                             `json:"min_interval_ms"`
	PowerupsDefault   Powerups                        `json:"powerups_default"`
	SchemaVersion     int                             `json:"schema_version"`
	SlotLimit         int                             `json:"slot_limit"`
	TileHMACMessage   string                          `json:"tile_hmac_message"`
}

type HistoryItem struct {
	CreatedAt      string         `json:"created_at"`
	Difficulty     string         `json:"difficulty"`
	EndedAtMs      *int64         `json:"ended_at_ms"`
	History        []HistoryEntry `json:"history"`
	MoveCount      int            `json:"move_count"`
	PatternCount   int            `json:"pattern_count"`
	Powerups       Powerups       `json:"powerups"`
	RewardAmount   float64        `json:"reward_amount"`
	SchemaVersion  int            `json:"schema_version"`
	ServerSeedHash string         `json:"server_seed_hash"`
	SessionID      int            `json:"session_id"`
	SlotLimit      int            `json:"slot_limit"`
	Slots          []int          `json:"slots"`
	SlotTiles      []Tile         `json:"slot_tiles"`
	StartedAtMs    int64          `json:"started_at_ms"`
	Status         string         `json:"status"`
	TemplateDigest *string        `json:"template_digest"`
	TemplateID     *string        `json:"template_id"`
	Tiles          []Tile         `json:"tiles"`
	TotalTiles     int            `json:"total_tiles"`
}

type HistoryResponse struct {
	Items       []HistoryItem `json:"items"`
	ServerNowMs int64         `json:"server_now_ms"`
}

type SessionSnapshot struct {
	Difficulty string
	SessionID  int
	SlotLimit  int
	Powerups   Powerups
	Status     string
	Tiles      []Tile
	SlotTiles  []Tile
	MoveCount  int
}

type AccountRunSummary struct {
	Email          string
	Difficulty     string
	Played         int
	Won            int
	Abandoned      int
	Failed         int
	TotalReward    float64
	BalanceAfter   *float64
	RemainingAfter int
	When           time.Time
	Err            error
}

type RoundResultSummary struct {
	Email          string
	Difficulty     string
	RoundIndex     int
	RoundTotal     int
	SessionID      int
	Continued      bool
	DryRun         bool
	Status         string
	Reward         float64
	BalanceAfter   *float64
	RemainingAfter int
	MoveCount      int
	UsedPowerups   []string
	Duration       time.Duration
	When           time.Time
	Err            error
}

const (
	ScratchGameTypeLuckyNumbers  = "lucky-numbers"
	ScratchGameTypeThreeKind     = "three-kind"
	ScratchGameTypeIconMatch     = "icon-match"
	ScratchGameTypeTreasureChest = "treasure-chests"
	ScratchGameTypeProgressRun   = "progress-run"
)

type ScratchPlayRequest struct {
	GameType string `json:"game_type"`
}

type ScratchRevealRequest struct {
	PlayID      int    `json:"play_id"`
	RevealToken string `json:"reveal_token"`
}

type ScratchNumber struct {
	Matched    bool   `json:"matched"`
	PrizeLabel string `json:"prize_label"`
	Value      int    `json:"value"`
}

type ScratchCell struct {
	Label   string `json:"label"`
	Winning bool   `json:"winning"`
}

type ScratchIconCell struct {
	Badge   string `json:"badge"`
	Icon    string `json:"icon"`
	Winning bool   `json:"winning"`
}

type ScratchChest struct {
	Tone    string `json:"tone"`
	Value   string `json:"value"`
	Winning bool   `json:"winning"`
}

type ScratchCheckpoint struct {
	Label   string `json:"label"`
	State   string `json:"state"`
	Winning bool   `json:"winning"`
}

type ScratchTicketPayload struct {
	Layout         string              `json:"layout"`
	Title          string              `json:"title"`
	Subtitle       string              `json:"subtitle"`
	LuckyNumbers   []int               `json:"lucky_numbers"`
	Numbers        []ScratchNumber     `json:"numbers"`
	Cells          []ScratchCell       `json:"cells"`
	WinningIndexes []int               `json:"winning_indexes"`
	Icons          []ScratchIconCell   `json:"icons"`
	WinningIcon    *string             `json:"winning_icon"`
	Chests         []ScratchChest      `json:"chests"`
	Checkpoints    []ScratchCheckpoint `json:"checkpoints"`
	FinishIndex    int                 `json:"finish_index"`
	RewardAmount   *float64            `json:"reward_amount"`
	RewardLabel    string              `json:"reward_label"`
}

type ScratchPlayResponse struct {
	Balance            float64              `json:"balance"`
	CostAmount         float64              `json:"cost_amount"`
	EarliestRevealAtMs int64                `json:"earliest_reveal_at_ms"`
	GameType           string               `json:"game_type"`
	IssuedAtMs         int64                `json:"issued_at_ms"`
	MinScratchMs       int                  `json:"min_scratch_ms"`
	PlayID             int                  `json:"play_id"`
	RevealToken        string               `json:"reveal_token"`
	Status             string               `json:"status"`
	TicketPayload      ScratchTicketPayload `json:"ticket_payload"`
}

type ScratchRevealResponse struct {
	Balance       float64              `json:"balance"`
	GameType      string               `json:"game_type"`
	NetAmount     float64              `json:"net_amount"`
	PlayID        int                  `json:"play_id"`
	RewardAmount  float64              `json:"reward_amount"`
	Status        string               `json:"status"`
	TicketPayload ScratchTicketPayload `json:"ticket_payload"`
}

type ScratchHistoryItem struct {
	CostAmount   float64 `json:"cost_amount"`
	ID           int     `json:"id"`
	NetAmount    float64 `json:"net_amount"`
	RewardAmount float64 `json:"reward_amount"`
	Status       string  `json:"status"`
}

type ScratchHistoryResponse struct {
	Items []ScratchHistoryItem `json:"items"`
}

type ScratchRoundResult struct {
	Round                 int
	Duration              time.Duration
	PlayResp              *ScratchPlayResponse
	RevealResp            *ScratchRevealResponse
	PlayHistoryAttempts   int
	RevealHistoryAttempts int
	PlayHistoryItem       *ScratchHistoryItem
	RevealHistoryItem     *ScratchHistoryItem
	PlayErr               error
	PlayHistoryErr        error
	RevealErr             error
	RevealHistoryErr      error
}

type CheckinUser struct {
	Balance float64 `json:"balance"`
	Email   string  `json:"email"`
	ID      int     `json:"id"`
	Status  string  `json:"status"`
}

type CheckinMeResponse struct {
	Authenticated bool        `json:"authenticated"`
	User          CheckinUser `json:"user"`
}

type CheckinTodayResponse struct {
	ClaimDate    string  `json:"claim_date"`
	Claimed      bool    `json:"claimed"`
	ClaimedAt    string  `json:"claimed_at"`
	RewardAmount float64 `json:"reward_amount"`
}

type CheckinClaimResponse struct {
	AlreadyClaimed bool    `json:"already_claimed"`
	ClaimDate      string  `json:"claim_date"`
	CreatedAt      string  `json:"created_at"`
	Ok             bool    `json:"ok"`
	RewardAmount   float64 `json:"reward_amount"`
}

type CheckinResult struct {
	Email        string
	Status       string
	Success      bool
	Delta        float64
	BalanceAfter float64
	When         time.Time
	Err          error
}
