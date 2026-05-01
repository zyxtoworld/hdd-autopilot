use chrono as _;
use crossterm as _;
use hdd_autopilot as _;
use mining::find_best_benchmark_config;
use rand as _;
use reqwest as _;
use serde as _;
use serde_json as _;
use unicode_width as _;
use url as _;

#[cfg(test)]
use tempfile as _;

fn main() {
    let result = find_best_benchmark_config();
    println!(
        "rust_best workers={} concurrency={} attempts={} elapsed={:.3}s aps={:.2}",
        result.workers,
        result.concurrency,
        result.attempts,
        result.elapsed.as_secs_f64(),
        result.attempts_per_s
    );
}
