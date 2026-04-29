use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use argon2::{Algorithm, Argon2, Params, Version};

use crate::backend::types::{
    BackendDescriptor, BackendKind, BenchmarkResult, MineBlockResult, MineResult,
};
use crate::error::{MiningError, interrupted_error};

pub(crate) const CPU_BENCHMARK_CASE_DURATION: Duration = Duration::from_secs(2);

struct CpuDigestEngine {
    argon2: Argon2<'static>,
}

impl CpuDigestEngine {
    fn new(job: &ComputeJob) -> Self {
        let params = Params::new(
            job.memory_cost_kib,
            job.time_cost,
            job.parallelism,
            Some(32),
        )
        .expect("valid argon2 params");
        Self {
            argon2: Argon2::new(Algorithm::Argon2id, Version::V0x13, params),
        }
    }

    fn compute_into(
        &self,
        job: &ComputeJob,
        nonce: usize,
        password: &mut Vec<u8>,
        output: &mut [u8; 32],
    ) {
        password.clear();
        password.extend_from_slice(&job.pass_prefix);
        append_nonce_ascii(password, nonce);
        self.argon2
            .hash_password_into(password, &job.seed_bytes, output)
            .expect("argon2 hash should succeed");
    }
}

#[derive(Debug, Clone)]
pub struct ComputeJob {
    pub seed_bytes: Vec<u8>,
    pub pass_prefix: Vec<u8>,
    pub time_cost: u32,
    pub memory_cost_kib: u32,
    pub parallelism: u32,
    pub difficulty_bits: i32,
}

const BENCHMARK_SEED_BYTES: &[u8] = b"benchmark-seed-fixed";
const BENCHMARK_PASS_PREFIX: &[u8] =
    b"benchmark-seed-fixed:1:benchmark-visitor-fixed:1:benchmark-session-salt-fixed:";

pub(crate) fn default_benchmark_job() -> ComputeJob {
    ComputeJob {
        seed_bytes: BENCHMARK_SEED_BYTES.to_vec(),
        pass_prefix: BENCHMARK_PASS_PREFIX.to_vec(),
        time_cost: 1,
        memory_cost_kib: 64 * 1024,
        parallelism: 1,
        difficulty_bits: 255,
    }
}

pub(crate) fn benchmark_job_for_shape(job: &ComputeJob) -> ComputeJob {
    ComputeJob {
        seed_bytes: BENCHMARK_SEED_BYTES.to_vec(),
        pass_prefix: BENCHMARK_PASS_PREFIX.to_vec(),
        time_cost: job.time_cost,
        memory_cost_kib: job.memory_cost_kib,
        parallelism: job.parallelism.max(1),
        difficulty_bits: 255,
    }
}

#[derive(Debug, Clone)]
pub struct CpuBackend {
    descriptor: BackendDescriptor,
}

pub struct CpuMiningSession {
    receiver: mpsc::Receiver<MineResult>,
    handles: Vec<thread::JoinHandle<()>>,
    attempts: Arc<AtomicI64>,
    stop: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl Drop for CpuMiningSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

impl CpuMiningSession {
    pub fn wait_for_result(&mut self) -> Result<MineBlockResult, MiningError> {
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                self.stop.store(true, Ordering::SeqCst);
                return Err(interrupted_error());
            }
            match self.receiver.recv_timeout(Duration::from_millis(20)) {
                Ok(result) => {
                    self.stop.store(true, Ordering::SeqCst);
                    return Ok(MineBlockResult {
                        found: Some(result),
                        attempts: self.attempts.load(Ordering::Relaxed),
                    });
                }
                Err(RecvTimeoutError::Timeout) => {
                    if self.stop.load(Ordering::SeqCst)
                        || self.handles.iter().all(thread::JoinHandle::is_finished)
                    {
                        return Ok(MineBlockResult {
                            found: None,
                            attempts: self.attempts.load(Ordering::Relaxed),
                        });
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    if self.cancel.load(Ordering::SeqCst) {
                        return Err(interrupted_error());
                    }
                    return Ok(MineBlockResult {
                        found: None,
                        attempts: self.attempts.load(Ordering::Relaxed),
                    });
                }
            }
        }
    }
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuBackend {
    pub fn new() -> Self {
        Self {
            descriptor: BackendDescriptor {
                kind: BackendKind::Cpu,
                name: "CPU".to_string(),
                device_id: "cpu".to_string(),
                device_index: None,
            },
        }
    }

    pub fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }

    pub fn default_thread_count() -> usize {
        std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
    }

    pub fn find_best_benchmark_config(&self) -> BenchmarkResult {
        self.find_best_benchmark_config_with_cancel(
            &default_benchmark_job(),
            Self::default_thread_count(),
            &Arc::new(AtomicBool::new(false)),
        )
        .expect("benchmark should complete without cancellation")
    }

    pub fn find_best_benchmark_config_with_cancel_and_output(
        &self,
        job: &ComputeJob,
        max_threads: usize,
        cancel: &Arc<AtomicBool>,
        output: &crate::OutputSink,
    ) -> Result<BenchmarkResult, MiningError> {
        let cpu_count = max_threads.max(1).min(Self::default_thread_count().max(1));
        let mut concurrency_candidates = vec![1, cpu_count / 2, cpu_count];
        concurrency_candidates.retain(|value| *value > 0);
        concurrency_candidates.sort_unstable();
        concurrency_candidates.dedup();

        let mut worker_candidates = vec![1, cpu_count / 4, cpu_count / 2, cpu_count];
        worker_candidates.retain(|workers| *workers > 0);
        worker_candidates.sort_unstable();
        worker_candidates.dedup();

        let total_cases = concurrency_candidates.len() * worker_candidates.len();

        let mut best: Option<BenchmarkResult> = None;
        let mut case_index = 0;
        for concurrency in concurrency_candidates.iter().copied() {
            for workers in worker_candidates.iter().copied() {
                if cancel.load(Ordering::SeqCst) {
                    return Err(interrupted_error());
                }
                case_index += 1;
                let result = self.run_benchmark_case_with_cancel(
                    job,
                    workers,
                    concurrency,
                    CPU_BENCHMARK_CASE_DURATION,
                    cancel,
                )?;
                output.line_fmt(format_args!(
                    "CPU 自动调优结果 {}/{}：线程数 {}，并发数 {}，速度约 {:.2} 次/秒。",
                    case_index,
                    total_cases,
                    result.workers,
                    result.concurrency,
                    result.attempts_per_s
                ));
                if best
                    .as_ref()
                    .is_none_or(|current| result.attempts_per_s > current.attempts_per_s)
                {
                    best = Some(result);
                }
            }
        }

        let best = best.unwrap_or_else(|| {
            self.run_benchmark_case_with_concurrency(job, 1, 1, CPU_BENCHMARK_CASE_DURATION)
        });
        output.line_fmt(format_args!(
            "CPU 自动调优完成：推荐线程数 {}，并发数 {}，预计速度约 {:.2} 次/秒。",
            best.workers, best.concurrency, best.attempts_per_s
        ));
        Ok(best)
    }

    pub fn find_best_benchmark_config_with_cancel(
        &self,
        job: &ComputeJob,
        max_threads: usize,
        cancel: &Arc<AtomicBool>,
    ) -> Result<BenchmarkResult, MiningError> {
        self.find_best_benchmark_config_with_cancel_and_output(
            job,
            max_threads,
            cancel,
            &crate::OutputSink::stdout(),
        )
    }

    pub fn run_benchmark_case(&self, workers: usize, duration: Duration) -> BenchmarkResult {
        self.run_benchmark_case_with_concurrency(
            &default_benchmark_job(),
            workers,
            workers,
            duration,
        )
    }

    pub fn run_benchmark_case_with_concurrency(
        &self,
        job: &ComputeJob,
        workers: usize,
        concurrency: usize,
        duration: Duration,
    ) -> BenchmarkResult {
        self.run_benchmark_case_with_cancel(
            job,
            workers,
            concurrency,
            duration,
            &Arc::new(AtomicBool::new(false)),
        )
        .expect("benchmark should complete without cancellation")
    }

    pub fn run_benchmark_case_with_cancel(
        &self,
        job: &ComputeJob,
        workers: usize,
        concurrency: usize,
        duration: Duration,
        cancel: &Arc<AtomicBool>,
    ) -> Result<BenchmarkResult, MiningError> {
        let job = benchmark_job_for_shape(job);
        let attempts = Arc::new(AtomicI64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        let worker_total = workers.max(1);
        let concurrency_total = concurrency.max(1).min(worker_total);
        let shard_groups = build_shard_groups(worker_total, concurrency_total);
        let mut handles = Vec::new();
        for assigned_shards in shard_groups {
            let job = job.clone();
            let attempts = Arc::clone(&attempts);
            let stop = Arc::clone(&stop);
            let cancel = Arc::clone(cancel);
            handles.push(thread::spawn(move || {
                let deadline = Instant::now() + duration;
                let hasher = CpuDigestEngine::new(&job);
                let mut password = Vec::with_capacity(job.pass_prefix.len() + 20);
                let mut output = [0u8; 32];
                let mut local_attempts = 0i64;
                let mut next_nonces = assigned_shards
                    .into_iter()
                    .map(|shard_index| shard_index + 1)
                    .collect::<Vec<_>>();
                while Instant::now() < deadline
                    && !stop.load(Ordering::Relaxed)
                    && !cancel.load(Ordering::Relaxed)
                {
                    for nonce in &mut next_nonces {
                        if Instant::now() >= deadline
                            || stop.load(Ordering::Relaxed)
                            || cancel.load(Ordering::Relaxed)
                        {
                            break;
                        }
                        hasher.compute_into(&job, *nonce, &mut password, &mut output);
                        *nonce += worker_total;
                        local_attempts += 1;
                        if local_attempts % 64 == 0 {
                            attempts.fetch_add(64, Ordering::Relaxed);
                        }
                    }
                }
                attempts.fetch_add(local_attempts % 64, Ordering::Relaxed);
            }));
        }
        for handle in handles {
            let _ = handle.join();
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(interrupted_error());
        }
        let elapsed = started.elapsed();
        let attempts = attempts.load(Ordering::Relaxed);
        Ok(BenchmarkResult {
            workers: worker_total,
            concurrency: concurrency_total,
            by_segment: false,
            precompute_refs: false,
            attempts,
            elapsed,
            attempts_per_s: attempts as f64 / elapsed.as_secs_f64().max(0.001),
        })
    }

    pub fn start_mining_session(
        &self,
        job: &ComputeJob,
        workers: usize,
        concurrency: usize,
        start_nonce: usize,
        nonce_count: usize,
        stop: &Arc<AtomicBool>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<CpuMiningSession, MiningError> {
        if nonce_count == 0 {
            return Err(MiningError::Message("nonce 范围不能为空。".to_string()));
        }
        let attempts = Arc::new(AtomicI64::new(0));
        let (sender, receiver) = mpsc::channel();
        let worker_total = workers.max(1).min(nonce_count.max(1));
        let concurrency_total = concurrency.max(1).min(worker_total);
        let shard_groups = build_shard_groups(worker_total, concurrency_total);
        let mut handles = Vec::new();

        for assigned_shards in shard_groups {
            let job = job.clone();
            let stop = Arc::clone(stop);
            let cancel = Arc::clone(cancel);
            let attempts = Arc::clone(&attempts);
            let sender = sender.clone();
            handles.push(thread::spawn(move || {
                let hasher = CpuDigestEngine::new(&job);
                let mut password = Vec::with_capacity(job.pass_prefix.len() + 20);
                let mut output = [0u8; 32];
                let mut local_attempts = 0i64;
                let mut next_nonces = assigned_shards
                    .into_iter()
                    .map(|shard_index| start_nonce.saturating_add(shard_index))
                    .collect::<Vec<_>>();
                let upper_bound = start_nonce.saturating_add(nonce_count);
                while !stop.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst) {
                    let mut advanced = false;
                    for nonce in &mut next_nonces {
                        if *nonce >= upper_bound
                            || stop.load(Ordering::SeqCst)
                            || cancel.load(Ordering::SeqCst)
                        {
                            continue;
                        }
                        advanced = true;
                        hasher.compute_into(&job, *nonce, &mut password, &mut output);
                        local_attempts += 1;
                        if local_attempts % 64 == 0 {
                            attempts.fetch_add(64, Ordering::Relaxed);
                        }
                        if meets_difficulty(&output, job.difficulty_bits) {
                            let pending = local_attempts % 64;
                            let attempt_count = if pending == 0 {
                                attempts.load(Ordering::Relaxed)
                            } else {
                                attempts.fetch_add(pending, Ordering::Relaxed) + pending
                            };
                            let _ = sender.send(MineResult {
                                nonce: *nonce,
                                digest: hex_lower(&output),
                                attempts: attempt_count,
                            });
                            stop.store(true, Ordering::SeqCst);
                            return;
                        }
                        *nonce = nonce.saturating_add(worker_total);
                    }
                    if !advanced {
                        break;
                    }
                }
                attempts.fetch_add(local_attempts % 64, Ordering::Relaxed);
            }));
        }
        drop(sender);

        Ok(CpuMiningSession {
            receiver,
            handles,
            attempts,
            stop: Arc::clone(stop),
            cancel: Arc::clone(cancel),
        })
    }
}

pub(crate) fn compute_digest(job: &ComputeJob, nonce: usize) -> Vec<u8> {
    let hasher = CpuDigestEngine::new(job);
    let mut password = Vec::with_capacity(job.pass_prefix.len() + 20);
    let mut output = [0u8; 32];
    hasher.compute_into(job, nonce, &mut password, &mut output);
    output.to_vec()
}

fn build_shard_groups(worker_total: usize, concurrency_total: usize) -> Vec<Vec<usize>> {
    let mut shard_groups = vec![Vec::new(); concurrency_total.max(1)];
    for shard_index in 0..worker_total.max(1) {
        shard_groups[shard_index % concurrency_total.max(1)].push(shard_index);
    }
    shard_groups
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::*;

    #[test]
    fn build_shard_groups_distributes_workers_across_concurrency() {
        let groups = build_shard_groups(5, 2);

        assert_eq!(groups, vec![vec![0, 2, 4], vec![1, 3]]);
    }

    #[test]
    fn benchmark_respects_max_threads_limit() {
        let backend = CpuBackend::new();
        let cancel = Arc::new(AtomicBool::new(false));

        let result = backend
            .find_best_benchmark_config_with_cancel(&default_benchmark_job(), 2, &cancel)
            .expect("benchmark should succeed");

        assert!(result.workers <= 2);
        assert!(result.concurrency <= 2);
        assert!(result.workers >= 1);
        assert!(result.concurrency >= 1);
    }
}

fn append_nonce_ascii(buffer: &mut Vec<u8>, mut value: usize) {
    if value == 0 {
        buffer.push(b'0');
        return;
    }
    let mut digits = [0u8; 20];
    let mut index = digits.len();
    while value > 0 {
        index -= 1;
        digits[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    buffer.extend_from_slice(&digits[index..]);
}

pub(crate) fn meets_difficulty(digest: &[u8], difficulty_bits: i32) -> bool {
    let difficulty_bits = difficulty_bits.max(0) as usize;
    let full_bytes = difficulty_bits / 8;
    for index in 0..full_bytes {
        if digest.get(index).copied().unwrap_or_default() != 0 {
            return false;
        }
    }
    let remaining_bits = difficulty_bits % 8;
    if remaining_bits == 0 {
        return true;
    }
    let byte = match digest.get(full_bytes) {
        Some(value) => *value,
        None => return false,
    };
    let mask = 0xFFu8 << (8 - remaining_bits);
    byte & mask == 0
}

pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
