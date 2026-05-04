use crate::solver::minesweeper::Board;

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct MinesweeperRoundSummary {
    pub(super) email: String,
    pub(super) difficulty: String,
    pub(super) round_index: i32,
    pub(super) round_total: i32,
    pub(super) play_id: i32,
    pub(super) continued: bool,
    pub(super) status: String,
    pub(super) reward: f64,
    pub(super) remaining_after: i32,
    pub(super) rows: i32,
    pub(super) cols: i32,
    pub(super) mine_count: i32,
    pub(super) executed_moves: i32,
    pub(super) safe_reveals: i32,
    pub(super) flags: i32,
    pub(super) chords: i32,
    pub(super) guesses: i32,
    pub(super) duration_ms: i64,
    pub(super) when_unix_ms: i64,
    pub(super) error_message: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct MinesweeperDifficultySummary {
    pub(super) email: String,
    pub(super) difficulty: String,
    pub(super) played: i32,
    pub(super) won: i32,
    pub(super) lost: i32,
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

#[derive(Debug, Clone, PartialEq)]
pub(super) struct MinesweeperSnapshot {
    pub(super) difficulty: String,
    pub(super) play_id: i32,
    pub(super) rows: i32,
    pub(super) cols: i32,
    pub(super) mine_count: i32,
    pub(super) status: String,
    pub(super) resolution: String,
    pub(super) reward_amount: f64,
    pub(super) trace_count: i32,
    pub(super) board: Board,
}
