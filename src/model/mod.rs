pub(crate) fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

mod auth;
mod checkin;
mod scratch;
mod sheepmatch;

pub use auth::{
    AuthCache, AuthConfig, AuthMeData, AuthMeResponse, AuthSession, LoginRequest, LoginResponse,
    LoginResponseData, LoginUser, SessionCookie,
};
pub use checkin::{
    CheckinClaimResponse, CheckinMeResponse, CheckinResult, CheckinTodayResponse, CheckinUser,
};
pub use scratch::{
    SCRATCH_GAME_TYPE_ICON_MATCH, SCRATCH_GAME_TYPE_LUCKY_NUMBERS, SCRATCH_GAME_TYPE_PROGRESS_RUN,
    SCRATCH_GAME_TYPE_THREE_KIND, SCRATCH_GAME_TYPE_TREASURE_CHEST, ScratchCell, ScratchCheckpoint,
    ScratchChest, ScratchHistoryItem, ScratchHistoryResponse, ScratchIconCell, ScratchNumber,
    ScratchPlayRequest, ScratchPlayResponse, ScratchRevealRequest, ScratchRevealResponse,
    ScratchRoundResult, ScratchTicketPayload, scratch_reveal_ready_at,
};
pub use sheepmatch::{
    AbandonRequest, AbandonResponse, AccountRunSummary, ConfigResponse, DIFFICULTY_EASY,
    DIFFICULTY_HARD, DIFFICULTY_HELL, DIFFICULTY_NORMAL, DIFFICULTY_ORDER, GameDifficultyConfig,
    HistoryEntry, HistoryItem, HistoryResponse, Powerups, RoundResultSummary, SessionSnapshot,
    StartRequest, StartResponse, StepRequest, StepResponse, Tile, TileMeResponse, TileMeUser,
};
