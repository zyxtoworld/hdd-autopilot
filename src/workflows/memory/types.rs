use crate::model::MemoryCard;

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct MemoryRoundSummary {
    pub(super) email: String,
    pub(super) difficulty: String,
    pub(super) round_index: i32,
    pub(super) round_total: i32,
    pub(super) session_id: i32,
    pub(super) continued: bool,
    pub(super) status: String,
    pub(super) reward: f64,
    pub(super) remaining_after: i32,
    pub(super) peek_count: i32,
    pub(super) match_count: i32,
    pub(super) pairs: i32,
    pub(super) duration_ms: i64,
    pub(super) when_unix_ms: i64,
    pub(super) error_message: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct MemoryDifficultySummary {
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

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct MemorySnapshot {
    pub(super) difficulty: String,
    pub(super) session_id: i32,
    pub(super) rows: i32,
    pub(super) cols: i32,
    pub(super) pairs: i32,
    pub(super) peek_limit: i32,
    pub(super) peek_count: i32,
    pub(super) match_count: i32,
    pub(super) matched_indices: Vec<i32>,
    pub(super) currently_revealed: Vec<MemoryCard>,
    pub(super) status: String,
    pub(super) game_over: bool,
    pub(super) won: bool,
    pub(super) reward_amount: f64,
}

impl MemorySnapshot {
    pub(super) fn total_cards(&self) -> i32 {
        let grid_total = self.rows.saturating_mul(self.cols);
        if grid_total > 0 {
            grid_total
        } else {
            self.pairs.saturating_mul(2)
        }
    }
}
