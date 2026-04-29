#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct PuzzleRoundSummary {
    pub(super) email: String,
    pub(super) difficulty: String,
    pub(super) round_index: i32,
    pub(super) round_total: i32,
    pub(super) session_id: i32,
    pub(super) continued: bool,
    pub(super) status: String,
    pub(super) reward: f64,
    pub(super) remaining_after: i32,
    pub(super) move_count: i32,
    pub(super) max_tile: i32,
    pub(super) score: i32,
    pub(super) duration_ms: i64,
    pub(super) when_unix_ms: i64,
    pub(super) error_message: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct PuzzleDifficultySummary {
    pub(super) email: String,
    pub(super) difficulty: String,
    pub(super) played: i32,
    pub(super) won: i32,
    pub(super) failed: i32,
    pub(super) total_reward: f64,
    pub(super) remaining_after: i32,
    pub(super) when_unix_ms: i64,
    pub(super) error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoundProgress {
    pub(super) current: i32,
    pub(super) total: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PuzzleSnapshot {
    pub(super) board: Vec<Vec<i32>>,
    pub(super) difficulty: String,
    pub(super) game_over: bool,
    pub(super) max_tile: i32,
    pub(super) move_count: i32,
    pub(super) score: i32,
    pub(super) session_id: i32,
    pub(super) size: i32,
    pub(super) status: String,
    pub(super) target_tile: i32,
    pub(super) won: bool,
}
