use chrono as _;
use crossterm as _;
use hdd_autopilot as _;
use mining::benchmark_best_gpu_runtime;
use rand as _;
use reqwest as _;
use serde as _;
use serde_json as _;
use unicode_width as _;
use url as _;

#[cfg(test)]
use tempfile as _;

fn main() {
    let (best, runtime) = match benchmark_best_gpu_runtime() {
        Ok(results) => results,
        Err(error) => {
            eprintln!("gpu_best error={error}");
            std::process::exit(1);
        }
    };
    println!(
        "gpu_best batch_size={} by_segment={} precompute_refs={} attempts={} elapsed={:.3}s aps={:.2}",
        best.workers,
        best.by_segment,
        best.precompute_refs,
        best.attempts,
        best.elapsed.as_secs_f64(),
        best.attempts_per_s
    );

    println!(
        "gpu_runtime batch_size={} by_segment={} precompute_refs={} attempts={} elapsed={:.3}s aps={:.2}",
        runtime.workers,
        runtime.by_segment,
        runtime.precompute_refs,
        runtime.attempts,
        runtime.elapsed.as_secs_f64(),
        runtime.attempts_per_s
    );
}
