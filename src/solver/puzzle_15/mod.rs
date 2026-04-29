mod board;
mod search;
#[cfg(test)]
mod tests;

pub use board::{TileMove, apply_tile_move};
pub use search::solve;
