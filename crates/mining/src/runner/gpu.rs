use crate::backend::cuda::{GPU_FINALIST_COUNT, GPU_RUNTIME_BENCHMARK_DURATION};
use crate::backend::types::GpuBenchmarkConfig;
use crate::backend::{BackendDescriptor, BenchmarkResult, ComputeJob};
use crate::error::is_interrupted_error;
use crate::{MiningError, humanize_error};

use super::Runner;
use super::support::{SelectedBackend, localized_bool};

impl Runner {
    pub(super) fn collect_gpu_backend_candidates(
        &self,
        job: &ComputeJob,
    ) -> Result<Vec<SelectedBackend>, MiningError> {
        let mut candidates = self.collect_cuda_backend_candidates(job)?;
        candidates.extend(self.collect_opencl_backend_candidates(job)?);
        candidates.extend(self.collect_metal_backend_candidates(job)?);
        Ok(candidates)
    }

    fn collect_cuda_backend_candidates(
        &self,
        job: &ComputeJob,
    ) -> Result<Vec<SelectedBackend>, MiningError> {
        let cuda_availability = self.cuda_backend.detect_availability();
        if !cuda_availability.available {
            if !cuda_availability.reason.trim().is_empty() {
                self.log(format_args!(
                    "CUDA 后端不可用：{}",
                    cuda_availability.reason
                ));
            }
            return Ok(Vec::new());
        }

        self.collect_gpu_candidates_by_device(
            "CUDA",
            self.cuda_backend.list_devices()?,
            |descriptor| {
                self.cuda_backend
                    .quick_screen_benchmark_for_descriptor(descriptor, job)
            },
            |descriptor| self.tune_cuda_backend(descriptor, job),
        )
    }

    fn collect_opencl_backend_candidates(
        &self,
        job: &ComputeJob,
    ) -> Result<Vec<SelectedBackend>, MiningError> {
        let opencl_availability = self.opencl_backend.detect_availability();
        if !opencl_availability.available {
            if !opencl_availability.reason.trim().is_empty() {
                self.log(format_args!(
                    "OpenCL 后端不可用：{}",
                    opencl_availability.reason
                ));
            }
            return Ok(Vec::new());
        }

        self.collect_gpu_candidates_by_device(
            "OpenCL",
            self.opencl_backend.list_devices()?,
            |descriptor| {
                self.opencl_backend
                    .quick_screen_benchmark_for_descriptor(descriptor, job)
            },
            |descriptor| self.tune_opencl_backend(descriptor, job),
        )
    }

    fn collect_metal_backend_candidates(
        &self,
        job: &ComputeJob,
    ) -> Result<Vec<SelectedBackend>, MiningError> {
        let metal_availability = self.metal_backend.detect_availability();
        if !metal_availability.available {
            if !metal_availability.reason.trim().is_empty() {
                self.log(format_args!(
                    "Metal 后端不可用：{}",
                    metal_availability.reason
                ));
            }
            return Ok(Vec::new());
        }

        self.collect_gpu_candidates_by_device(
            "Metal",
            self.metal_backend.list_devices()?,
            |descriptor| {
                self.metal_backend
                    .quick_screen_benchmark_for_descriptor(descriptor, job)
            },
            |descriptor| self.tune_metal_backend(descriptor, job),
        )
    }

    fn collect_gpu_candidates_by_device<FScreen, FTune>(
        &self,
        label: &str,
        devices: Vec<BackendDescriptor>,
        screen: FScreen,
        tune: FTune,
    ) -> Result<Vec<SelectedBackend>, MiningError>
    where
        FScreen: Fn(&BackendDescriptor) -> Result<BenchmarkResult, MiningError>,
        FTune: Fn(&BackendDescriptor) -> Result<BenchmarkResult, MiningError>,
    {
        if devices.is_empty() {
            self.log(format_args!(
                "{} 后端可用，但没有检测到可用设备，回退 CPU。",
                label
            ));
            return Ok(Vec::new());
        }

        let mut screened = Vec::new();
        for descriptor in devices {
            self.check_cancel()?;
            match screen(&descriptor) {
                Ok(result) => {
                    self.log(format_args!(
                        "{} 设备初筛完成：设备 {}，默认批大小 {}，按分段 {}，预计算参考值 {}，预计速度约 {:.2} 次/秒。",
                        label,
                        descriptor.name,
                        result.workers,
                        localized_bool(result.by_segment),
                        localized_bool(result.precompute_refs),
                        result.attempts_per_s
                    ));
                    screened.push((descriptor, result));
                }
                Err(error) => {
                    if is_interrupted_error(&error) {
                        return Err(error);
                    }
                    self.log(format_args!(
                        "{} 设备 {} 初筛失败，回退 CPU：{}",
                        label,
                        descriptor.name,
                        humanize_error(&error)
                    ));
                }
            }
        }

        screened.sort_by(|left, right| right.1.attempts_per_s.total_cmp(&left.1.attempts_per_s));
        let finalists = screened
            .into_iter()
            .take(GPU_FINALIST_COUNT)
            .collect::<Vec<_>>();

        let mut candidates = Vec::new();
        for (descriptor, _) in finalists {
            match tune(&descriptor) {
                Ok(result) => {
                    self.log(format_args!(
                        "{} 自动调优完成：设备 {}，推荐批大小 {}，按分段 {}，预计算参考值 {}，预计速度约 {:.2} 次/秒。",
                        label,
                        descriptor.name,
                        result.workers,
                        localized_bool(result.by_segment),
                        localized_bool(result.precompute_refs),
                        result.attempts_per_s
                    ));
                    candidates.push(SelectedBackend::new(&descriptor, result));
                }
                Err(error) => {
                    if is_interrupted_error(&error) {
                        return Err(error);
                    }
                    self.log(format_args!(
                        "{} 设备 {} 自动调优失败，回退 CPU：{}",
                        label,
                        descriptor.name,
                        humanize_error(&error)
                    ));
                }
            }
        }
        Ok(candidates)
    }

    fn tune_cuda_backend(
        &self,
        descriptor: &BackendDescriptor,
        job: &ComputeJob,
    ) -> Result<BenchmarkResult, MiningError> {
        let templates = self
            .cuda_backend
            .solver_templates_for_descriptor(descriptor, job);
        self.tune_gpu_backend("CUDA", descriptor, &templates, |candidate| {
            self.cuda_backend.run_runtime_loop_benchmark_with_cancel(
                job,
                GpuBenchmarkConfig {
                    device_index: descriptor.device_index.unwrap_or(0),
                    batch_size: candidate.batch_size,
                    by_segment: candidate.by_segment,
                    precompute_refs: candidate.precompute_refs,
                    duration: GPU_RUNTIME_BENCHMARK_DURATION,
                },
                &self.cancel,
            )
        })
    }

    fn tune_opencl_backend(
        &self,
        descriptor: &BackendDescriptor,
        job: &ComputeJob,
    ) -> Result<BenchmarkResult, MiningError> {
        let templates = self
            .opencl_backend
            .solver_templates_for_descriptor(descriptor, job);
        self.tune_gpu_backend("OpenCL", descriptor, &templates, |candidate| {
            self.opencl_backend.run_runtime_loop_benchmark_with_cancel(
                job,
                GpuBenchmarkConfig {
                    device_index: descriptor.device_index.unwrap_or(0),
                    batch_size: candidate.batch_size,
                    by_segment: candidate.by_segment,
                    precompute_refs: candidate.precompute_refs,
                    duration: GPU_RUNTIME_BENCHMARK_DURATION,
                },
                &self.cancel,
            )
        })
    }

    fn tune_metal_backend(
        &self,
        descriptor: &BackendDescriptor,
        job: &ComputeJob,
    ) -> Result<BenchmarkResult, MiningError> {
        let templates = self
            .metal_backend
            .solver_templates_for_descriptor(descriptor, job);
        self.tune_gpu_backend("Metal", descriptor, &templates, |candidate| {
            self.metal_backend.run_runtime_loop_benchmark_with_cancel(
                job,
                GpuBenchmarkConfig {
                    device_index: descriptor.device_index.unwrap_or(0),
                    batch_size: candidate.batch_size,
                    by_segment: candidate.by_segment,
                    precompute_refs: candidate.precompute_refs,
                    duration: GPU_RUNTIME_BENCHMARK_DURATION,
                },
                &self.cancel,
            )
        })
    }

    fn tune_gpu_backend<TConfig, FRun>(
        &self,
        label: &str,
        descriptor: &BackendDescriptor,
        templates: &[TConfig],
        run: FRun,
    ) -> Result<BenchmarkResult, MiningError>
    where
        TConfig: Copy,
        FRun: Fn(TConfig) -> Result<BenchmarkResult, MiningError>,
    {
        let total_cases = templates.len();
        let mut best: Option<BenchmarkResult> = None;
        for (index, candidate) in templates.iter().copied().enumerate() {
            self.check_cancel()?;
            let result = match run(candidate) {
                Ok(result) => result,
                Err(error) if is_interrupted_error(&error) => return Err(error),
                Err(error) => {
                    self.log(format_args!(
                        "{} 自动调优配置 {}/{} 不可用，已跳过：{}",
                        label,
                        index + 1,
                        total_cases,
                        humanize_error(&error)
                    ));
                    continue;
                }
            };
            self.log(format_args!(
                "{} 自动调优结果 {}/{}：设备 {}，批大小 {}，按分段 {}，预计算参考值 {}，速度约 {:.2} 次/秒。",
                label,
                index + 1,
                total_cases,
                descriptor.name,
                result.workers,
                localized_bool(result.by_segment),
                localized_bool(result.precompute_refs),
                result.attempts_per_s
            ));
            if best
                .as_ref()
                .is_none_or(|existing| result.attempts_per_s > existing.attempts_per_s)
            {
                best = Some(result);
            }
        }
        best.ok_or_else(|| MiningError::Message(format!("{} 自动调优没有得到可用结果。", label)))
    }

    pub(super) fn filter_blacklisted(
        &self,
        candidates: Vec<SelectedBackend>,
    ) -> Vec<SelectedBackend> {
        let blacklist = self
            .backend_blacklist
            .lock()
            .expect("backend blacklist poisoned");
        candidates
            .into_iter()
            .filter(|candidate| !blacklist.contains(&(candidate.kind, candidate.device_id.clone())))
            .collect()
    }

    pub(super) fn run_backend_self_test(
        &self,
        backend: &SelectedBackend,
        job: &ComputeJob,
    ) -> Result<(), MiningError> {
        let digest = crate::backend::cpu::compute_digest(job, 1);
        if digest.is_empty()
            || !crate::backend::cpu::hex_lower(&digest)
                .chars()
                .all(|ch| ch.is_ascii_hexdigit())
        {
            return Err(MiningError::Message(format!(
                "{} 后端自检失败：摘要格式无效",
                backend.label
            )));
        }
        Ok(())
    }
}
