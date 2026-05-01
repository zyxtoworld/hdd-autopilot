pub mod cpu;
pub mod cuda;
pub mod metal;
pub mod opencl;
pub mod tuning;
pub mod types;

use crate::MiningError;

pub use cpu::{ComputeJob, CpuBackend};
pub use cuda::CudaBackend;
pub use metal::MetalBackend;
pub use opencl::OpenclBackend;
pub use types::{BackendDescriptor, BackendKind, BenchmarkResult, GPUAvailability};

pub(crate) fn assign_nonce_ranges(worker_count: usize) -> Result<Vec<(u64, u64)>, MiningError> {
    let worker_count = worker_count.max(1);
    let max_nonce = usize::MAX as u64;
    let usable = max_nonce.saturating_sub(1);
    let base = usable / worker_count as u64;
    if base == 0 {
        return Err(MiningError::Message("可用 nonce 空间不足。".to_string()));
    }
    let mut ranges = Vec::with_capacity(worker_count);
    let mut start = 1u64;
    for index in 0..worker_count {
        let count = if index + 1 == worker_count {
            max_nonce.saturating_sub(start).saturating_add(1)
        } else {
            base
        };
        ranges.push((start, count.max(1)));
        start = start
            .checked_add(base)
            .ok_or_else(|| MiningError::Message("nonce 空间已经耗尽。".to_string()))?;
    }
    Ok(ranges)
}

#[cfg(test)]
mod tests {
    use super::assign_nonce_ranges;

    #[test]
    fn assign_nonce_ranges_splits_space_without_overlap() {
        let ranges = assign_nonce_ranges(2).expect("assign ranges");

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].0, 1);
        assert_eq!(ranges[0].0 + ranges[0].1, ranges[1].0);
        assert!(ranges[1].1 >= ranges[0].1);
    }
}
