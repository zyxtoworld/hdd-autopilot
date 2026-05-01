use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    Cpu,
    Cuda,
    Metal,
    Opencl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDescriptor {
    pub kind: BackendKind,
    pub name: String,
    pub device_id: String,
    pub device_index: Option<usize>,
    pub gpu_profile: Option<GpuDeviceProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuDeviceProfile {
    pub global_memory_bytes: u64,
    pub max_alloc_bytes: u64,
    pub compute_units: u32,
    pub max_threads_per_group: u32,
    pub local_memory_bytes: u64,
    pub subgroup_size: u32,
    pub unified_memory: bool,
    pub low_power: bool,
    pub removable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GpuTuningShape {
    pub batch_size: usize,
    pub by_segment: bool,
    pub precompute_refs: bool,
}

pub fn recommended_gpu_tuning_shapes(
    profile: Option<GpuDeviceProfile>,
    memory_cost_kib: u32,
    parallelism: u32,
) -> Vec<GpuTuningShape> {
    let max_batch = recommended_max_batch(profile, memory_cost_kib, parallelism);
    let anchor = recommended_anchor_batch(profile, max_batch);
    let mut batches = Vec::new();
    for batch_size in powers_of_two_up_to(max_batch) {
        batches.push(batch_size);
    }
    if let Some(profile) = profile
        && profile.compute_units > 0
    {
        let units = profile.compute_units as usize;
        for candidate in [units / 2, units, units * 2, units * 4] {
            if candidate > 0 && candidate <= max_batch {
                batches.push(candidate);
            }
        }
    }
    batches.sort_unstable();
    batches.dedup();
    batches.sort_by_key(|batch| batch.abs_diff(anchor));

    let mut shapes = Vec::new();
    for batch_size in batches {
        for (by_segment, precompute_refs) in strategy_order(profile) {
            shapes.push(GpuTuningShape {
                batch_size,
                by_segment,
                precompute_refs,
            });
        }
    }
    if shapes.is_empty() {
        shapes.push(GpuTuningShape {
            batch_size: 1,
            by_segment: false,
            precompute_refs: false,
        });
    }
    shapes
}

fn recommended_max_batch(
    profile: Option<GpuDeviceProfile>,
    memory_cost_kib: u32,
    parallelism: u32,
) -> usize {
    let bytes_per_job = u128::from(memory_cost_kib.max(1)) * 1024 * u128::from(parallelism.max(1));
    let Some(profile) = profile else {
        return 1024;
    };
    let memory_percent = if profile.low_power {
        35
    } else if profile.unified_memory {
        45
    } else {
        65
    };
    let alloc_percent = if profile.low_power || profile.unified_memory {
        75
    } else {
        90
    };
    let batch_cap = if profile.low_power {
        512
    } else if profile.compute_units >= 32 || profile.max_threads_per_group >= 1024 {
        2048
    } else {
        1024
    };
    let mut usable = if profile.global_memory_bytes > 0 {
        u128::from(profile.global_memory_bytes) * memory_percent / 100
    } else {
        bytes_per_job * 1024
    };
    if profile.max_alloc_bytes > 0 {
        usable = usable.min(u128::from(profile.max_alloc_bytes) * alloc_percent / 100);
    }
    let max_by_memory = (usable / bytes_per_job).clamp(1, batch_cap) as usize;
    max_by_memory.max(1)
}

fn recommended_anchor_batch(profile: Option<GpuDeviceProfile>, max_batch: usize) -> usize {
    let Some(profile) = profile else {
        return max_batch.min(64).max(1);
    };
    let thread_anchor = if profile.max_threads_per_group > 0 {
        let subgroup = profile.subgroup_size.max(16) as usize;
        (profile.max_threads_per_group as usize / subgroup).max(1)
    } else {
        0
    };
    let compute_anchor = if profile.compute_units > 0 {
        let multiplier = if profile.low_power || profile.unified_memory {
            1
        } else if profile.removable {
            4
        } else {
            2
        };
        (profile.compute_units as usize).saturating_mul(multiplier)
    } else if profile.max_threads_per_group > 0 {
        thread_anchor
    } else {
        64
    };
    compute_anchor.max(thread_anchor).clamp(1, max_batch.max(1))
}

fn powers_of_two_up_to(max_batch: usize) -> Vec<usize> {
    let mut values = Vec::new();
    let mut value = 1usize;
    while value <= max_batch {
        values.push(value);
        value = value.saturating_mul(2);
        if value == 0 {
            break;
        }
    }
    if !values.contains(&max_batch) {
        values.push(max_batch);
    }
    values
}

fn strategy_order(profile: Option<GpuDeviceProfile>) -> [(bool, bool); 3] {
    let Some(profile) = profile else {
        return [(true, true), (true, false), (false, false)];
    };
    if profile.low_power
        || (profile.unified_memory && profile.global_memory_bytes <= 8 * 1024 * 1024 * 1024)
        || (profile.local_memory_bytes > 0 && profile.local_memory_bytes < 32 * 1024)
    {
        return [(false, false), (true, false), (true, true)];
    }
    let high_parallel_device = profile.compute_units >= 16 || profile.max_threads_per_group >= 512;
    if high_parallel_device {
        [(true, true), (true, false), (false, false)]
    } else {
        [(false, false), (true, false), (true, true)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BenchmarkResult {
    pub workers: usize,
    pub concurrency: usize,
    pub by_segment: bool,
    pub precompute_refs: bool,
    pub attempts: i64,
    pub elapsed: Duration,
    pub attempts_per_s: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuMiningSessionConfig {
    pub workers: usize,
    pub concurrency: usize,
    pub start_nonce: usize,
    pub nonce_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuBenchmarkConfig {
    pub device_index: usize,
    pub batch_size: usize,
    pub by_segment: bool,
    pub precompute_refs: bool,
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuMiningSessionConfig {
    pub device_index: usize,
    pub batch_size: usize,
    pub by_segment: bool,
    pub precompute_refs: bool,
    pub start_nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GPUAvailability {
    pub available: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MineResult {
    pub nonce: usize,
    pub digest: String,
    pub attempts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MineBlockResult {
    pub found: Option<MineResult>,
    pub attempts: i64,
}

#[cfg(test)]
mod tests {
    use super::{GpuDeviceProfile, recommended_gpu_tuning_shapes};

    #[test]
    fn recommended_gpu_tuning_shapes_respects_memory_limit() {
        let shapes = recommended_gpu_tuning_shapes(
            Some(GpuDeviceProfile {
                global_memory_bytes: 512 * 1024 * 1024,
                max_alloc_bytes: 256 * 1024 * 1024,
                compute_units: 8,
                max_threads_per_group: 256,
                local_memory_bytes: 32 * 1024,
                subgroup_size: 32,
                unified_memory: false,
                low_power: false,
                removable: false,
            }),
            64 * 1024,
            1,
        );

        assert!(shapes.iter().all(|shape| shape.batch_size <= 3));
        assert!(shapes.iter().any(|shape| shape.batch_size == 1));
    }

    #[test]
    fn recommended_gpu_tuning_shapes_prefers_compute_anchor_first() {
        let shapes = recommended_gpu_tuning_shapes(
            Some(GpuDeviceProfile {
                global_memory_bytes: 24 * 1024 * 1024 * 1024,
                max_alloc_bytes: 24 * 1024 * 1024 * 1024,
                compute_units: 64,
                max_threads_per_group: 1024,
                local_memory_bytes: 64 * 1024,
                subgroup_size: 32,
                unified_memory: false,
                low_power: false,
                removable: false,
            }),
            64 * 1024,
            1,
        );

        assert_eq!(shapes[0].batch_size, 128);
        assert!(shapes[0].by_segment);
        assert!(shapes[0].precompute_refs);
    }

    #[test]
    fn recommended_gpu_tuning_shapes_conserves_low_power_unified_devices() {
        let shapes = recommended_gpu_tuning_shapes(
            Some(GpuDeviceProfile {
                global_memory_bytes: 8 * 1024 * 1024 * 1024,
                max_alloc_bytes: 2 * 1024 * 1024 * 1024,
                compute_units: 16,
                max_threads_per_group: 512,
                local_memory_bytes: 16 * 1024,
                subgroup_size: 32,
                unified_memory: true,
                low_power: true,
                removable: false,
            }),
            64 * 1024,
            1,
        );

        assert!(shapes.iter().all(|shape| shape.batch_size <= 24));
        assert_eq!(shapes[0].batch_size, 16);
        assert!(!shapes[0].by_segment);
        assert!(!shapes[0].precompute_refs);
    }
}
