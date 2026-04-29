mod board;
mod strategy;

#[cfg(test)]
mod tests;

pub use board::{DEFAULT_DIRECTIONS, Direction, MoveOutcome, apply_move, legal_moves};

pub fn choose_next_move(
    board: &[Vec<i32>],
    _target_tile: i32,
    _four_ratio: f64,
    allowed_directions: &[Direction],
) -> Option<Direction> {
    strategy::choose_next_move(board, allowed_directions)
}

pub fn choose_next_move_fast(
    board: &[Vec<i32>],
    _target_tile: i32,
    _four_ratio: f64,
    allowed_directions: &[Direction],
) -> Option<Direction> {
    strategy::choose_next_move(board, allowed_directions)
}
