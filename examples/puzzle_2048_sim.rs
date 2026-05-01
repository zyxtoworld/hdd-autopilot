use chrono as _;
use crossterm as _;
use mining as _;
use rand as _;
use reqwest as _;
use serde as _;
use serde_json as _;
use tempfile as _;
use unicode_width as _;
use url as _;

use std::time::Instant;

use hdd_autopilot::solver::puzzle_2048::{
    DEFAULT_DIRECTIONS, apply_move, choose_next_move, choose_next_move_fast, legal_moves,
};

#[derive(Debug, Clone)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn index(&mut self, len: usize) -> usize {
        (self.next_u64() as usize) % len
    }

    fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.next_u64() % denominator < numerator
    }
}

#[derive(Debug, Default)]
struct Stats {
    wins: usize,
    total_moves: usize,
    max_tile_sum: usize,
    best_tile: i32,
}

fn empty_cells(board: &[Vec<i32>]) -> Vec<(usize, usize)> {
    let mut cells = Vec::new();
    for (r, row) in board.iter().enumerate() {
        for (c, value) in row.iter().enumerate() {
            if *value == 0 {
                cells.push((r, c));
            }
        }
    }
    cells
}

fn max_tile(board: &[Vec<i32>]) -> i32 {
    board.iter().flatten().copied().max().unwrap_or(0)
}

fn spawn(board: &mut [Vec<i32>], rng: &mut Rng) -> bool {
    let empty = empty_cells(board);
    if empty.is_empty() {
        return false;
    }
    let (r, c) = empty[rng.index(empty.len())];
    board[r][c] = if rng.chance(1, 10) { 4 } else { 2 };
    true
}

fn play(size: usize, target: i32, seed: u64, strong: bool) -> (bool, usize, i32, Vec<Vec<i32>>) {
    let mut rng = Rng::new(seed);
    let mut board = vec![vec![0; size]; size];
    spawn(&mut board, &mut rng);
    spawn(&mut board, &mut rng);
    let mut moves = 0;
    let max_moves = match size {
        3 => 500,
        4 => 2500,
        _ => 4500,
    };
    while max_tile(&board) < target
        && !legal_moves(&board, DEFAULT_DIRECTIONS).is_empty()
        && moves < max_moves
    {
        let direction = if strong {
            choose_next_move(&board, target, 0.1, DEFAULT_DIRECTIONS)
        } else {
            choose_next_move_fast(&board, target, 0.1, DEFAULT_DIRECTIONS)
        };
        let Some(direction) = direction else {
            break;
        };
        let outcome = apply_move(&board, direction);
        if !outcome.moved {
            break;
        }
        board = outcome.board;
        moves += 1;
        if outcome.max_tile < target {
            spawn(&mut board, &mut rng);
        }
    }
    (max_tile(&board) >= target, moves, max_tile(&board), board)
}

fn run(size: usize, target: i32, trials: usize, strong: bool) {
    let started = Instant::now();
    let mut stats = Stats::default();
    for i in 0..trials {
        let (won, moves, best, board) = play(
            size,
            target,
            0x2048_0000 + (size as u64) * 10_000 + i as u64,
            strong,
        );
        if won {
            stats.wins += 1;
        }
        stats.total_moves += moves;
        stats.max_tile_sum += best as usize;
        stats.best_tile = stats.best_tile.max(best);
        println!(
            "{}x{} #{:03}: {} moves={} max={}",
            size,
            size,
            i + 1,
            if won { "win" } else { "lose" },
            moves,
            best
        );
        if std::env::var("PUZZLE_2048_SIM_PRINT_BOARD").is_ok() {
            for row in &board {
                println!("  {:?}", row);
            }
        }
    }
    println!(
        "{}x{} target {} [{}]: {}/{} = {:.2}% avg_moves={:.1} avg_max={:.1} best={} elapsed={:.2?}",
        size,
        size,
        target,
        if strong { "strong" } else { "fast" },
        stats.wins,
        trials,
        stats.wins as f64 * 100.0 / trials as f64,
        stats.total_moves as f64 / trials as f64,
        stats.max_tile_sum as f64 / trials as f64,
        stats.best_tile,
        started.elapsed()
    );
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let trials = args
        .first()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100);
    let strong = args.iter().any(|value| value == "strong");
    match args.get(1).map(String::as_str) {
        Some("3") | Some("3x3") => run(3, 512, trials, strong),
        Some("4") | Some("4x4") => run(4, 2048, trials, strong),
        Some("5") | Some("5x5") => run(5, 4096, trials, strong),
        _ => {
            run(3, 512, trials, strong);
            run(4, 2048, trials, strong);
            run(5, 4096, trials, strong);
        }
    }
}
