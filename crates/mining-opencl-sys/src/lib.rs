#![allow(non_camel_case_types)]
#![cfg_attr(
    not(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    )),
    allow(dead_code)
)]

use std::time::Duration;

#[cfg(all(
    any(target_os = "macos", target_os = "linux", target_os = "windows"),
    mining_opencl_native_enabled
))]
use std::ffi::CStr;

#[repr(C)]
pub struct mining_opencl_solver_config {
    pub batch_size: usize,
    pub by_segment: bool,
    pub precompute_refs: bool,
}

#[repr(C)]
pub struct mining_opencl_job {
    pub seed_ptr: *const u8,
    pub seed_len: usize,
    pub pass_prefix_ptr: *const u8,
    pub pass_prefix_len: usize,
    pub time_cost: u32,
    pub memory_cost_kib: u32,
    pub parallelism: u32,
    pub difficulty_bits: i32,
}

#[repr(C)]
pub struct mining_opencl_benchmark_result {
    pub batch_size: usize,
    pub by_segment: bool,
    pub precompute_refs: bool,
    pub attempts: i64,
    pub elapsed_ms: i64,
    pub attempts_per_second: f64,
}

#[repr(C)]
pub struct mining_opencl_mine_result {
    pub found: bool,
    pub nonce: u64,
    pub attempts: i64,
    pub digest_hex: [u8; 65],
}

#[repr(C)]
pub struct mining_opencl_device_info {
    pub device_index: usize,
    pub device_type: u64,
    pub global_memory_bytes: u64,
    pub max_alloc_bytes: u64,
    pub compute_units: u32,
    pub max_work_group_size: u32,
    pub local_memory_bytes: u64,
    pub host_unified_memory: bool,
    pub device_id: [u8; 32],
    pub name: [u8; 128],
    pub vendor: [u8; 128],
    pub platform: [u8; 128],
}

#[repr(C)]
pub struct mining_opencl_session {
    _private: [u8; 0],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenclSolverConfig {
    pub batch_size: usize,
    pub by_segment: bool,
    pub precompute_refs: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenclJob<'a> {
    pub seed_bytes: &'a [u8],
    pub pass_prefix: &'a [u8],
    pub time_cost: u32,
    pub memory_cost_kib: u32,
    pub parallelism: u32,
    pub difficulty_bits: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenclDeviceInfo {
    pub device_index: usize,
    pub device_type: u64,
    pub global_memory_bytes: u64,
    pub max_alloc_bytes: u64,
    pub compute_units: u32,
    pub max_work_group_size: u32,
    pub local_memory_bytes: u64,
    pub host_unified_memory: bool,
    pub device_id: String,
    pub name: String,
    pub vendor: String,
    pub platform: String,
}

const OPENCL_DEVICE_TYPE_GPU: u64 = 1 << 2;
const OPENCL_DEVICE_TYPE_ACCELERATOR: u64 = 1 << 3;

impl OpenclDeviceInfo {
    pub fn is_gpu_like(&self) -> bool {
        self.device_type & (OPENCL_DEVICE_TYPE_GPU | OPENCL_DEVICE_TYPE_ACCELERATOR) != 0
    }

    pub fn device_type_label(&self) -> &'static str {
        if self.device_type & OPENCL_DEVICE_TYPE_GPU != 0 {
            "GPU"
        } else if self.device_type & OPENCL_DEVICE_TYPE_ACCELERATOR != 0 {
            "Accelerator"
        } else {
            "OpenCL"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenclBenchmarkResult {
    pub config: OpenclSolverConfig,
    pub attempts: i64,
    pub elapsed: Duration,
    pub attempts_per_second: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenclMineResult {
    pub found: bool,
    pub nonce: u64,
    pub attempts: i64,
    pub digest_hex: String,
}

pub struct OpenclMiningSession {
    raw: *mut mining_opencl_session,
}

unsafe extern "C" {
    fn mining_opencl_is_available() -> bool;
    fn mining_opencl_validate() -> bool;
    fn mining_opencl_device_count() -> usize;
    fn mining_opencl_get_device_info(
        device_index: usize,
        result: *mut mining_opencl_device_info,
    ) -> bool;
    fn mining_opencl_last_error_message() -> *const std::ffi::c_char;
    fn mining_opencl_default_solver_config(
        device_index: usize,
        job: *const mining_opencl_job,
        result: *mut mining_opencl_solver_config,
    ) -> bool;
    fn mining_opencl_find_best_benchmark_config(
        device_index: usize,
        result: *mut mining_opencl_benchmark_result,
    ) -> bool;
    fn mining_opencl_mine_batch(
        device_index: usize,
        job: *const mining_opencl_job,
        config: *const mining_opencl_solver_config,
        start_nonce: u64,
        result: *mut mining_opencl_mine_result,
    ) -> bool;
    fn mining_opencl_session_create(
        device_index: usize,
        job: *const mining_opencl_job,
        config: *const mining_opencl_solver_config,
        start_nonce: u64,
    ) -> *mut mining_opencl_session;
    fn mining_opencl_session_mine_next_batch(
        session: *mut mining_opencl_session,
        result: *mut mining_opencl_mine_result,
    ) -> bool;
    fn mining_opencl_session_destroy(session: *mut mining_opencl_session);
}

pub fn is_available() -> Result<bool, String> {
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok(false)
    }
    #[cfg(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        not(mining_opencl_native_enabled)
    ))]
    {
        Err("当前构建未启用 OpenCL 原生后端。".to_string())
    }
    #[cfg(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    ))]
    unsafe {
        if mining_opencl_is_available() {
            Ok(true)
        } else {
            let message = last_error_message();
            if message.is_empty() {
                Ok(false)
            } else {
                Err(message)
            }
        }
    }
}

pub fn validate() -> Result<(), String> {
    #[cfg(not(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    )))]
    {
        Err("当前平台未启用 OpenCL 后端".to_string())
    }
    #[cfg(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    ))]
    unsafe {
        if mining_opencl_validate() {
            Ok(())
        } else {
            Err(last_error_message())
        }
    }
}

pub fn list_devices() -> Result<Vec<OpenclDeviceInfo>, String> {
    #[cfg(not(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    )))]
    {
        Ok(Vec::new())
    }
    #[cfg(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    ))]
    unsafe {
        let count = mining_opencl_device_count();
        let mut devices = Vec::with_capacity(count);
        for device_index in 0..count {
            let mut raw = mining_opencl_device_info {
                device_index,
                device_type: 0,
                global_memory_bytes: 0,
                max_alloc_bytes: 0,
                compute_units: 0,
                max_work_group_size: 0,
                local_memory_bytes: 0,
                host_unified_memory: false,
                device_id: [0; 32],
                name: [0; 128],
                vendor: [0; 128],
                platform: [0; 128],
            };
            if !mining_opencl_get_device_info(device_index, &mut raw) {
                continue;
            }
            devices.push(OpenclDeviceInfo {
                device_index: raw.device_index,
                device_type: raw.device_type,
                global_memory_bytes: raw.global_memory_bytes,
                max_alloc_bytes: raw.max_alloc_bytes,
                compute_units: raw.compute_units,
                max_work_group_size: raw.max_work_group_size,
                local_memory_bytes: raw.local_memory_bytes,
                host_unified_memory: raw.host_unified_memory,
                device_id: decode_c_string(&raw.device_id),
                name: decode_c_string(&raw.name),
                vendor: decode_c_string(&raw.vendor),
                platform: decode_c_string(&raw.platform),
            });
        }
        Ok(devices)
    }
}

pub fn default_solver_config(
    device_index: usize,
    job: &OpenclJob<'_>,
) -> Result<OpenclSolverConfig, String> {
    #[cfg(not(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    )))]
    {
        let _ = (device_index, job);
        Err("当前平台未启用 OpenCL 后端".to_string())
    }
    #[cfg(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    ))]
    unsafe {
        let raw_job = mining_opencl_job {
            seed_ptr: job.seed_bytes.as_ptr(),
            seed_len: job.seed_bytes.len(),
            pass_prefix_ptr: job.pass_prefix.as_ptr(),
            pass_prefix_len: job.pass_prefix.len(),
            time_cost: job.time_cost,
            memory_cost_kib: job.memory_cost_kib,
            parallelism: job.parallelism,
            difficulty_bits: job.difficulty_bits,
        };
        let mut raw_config = mining_opencl_solver_config {
            batch_size: 0,
            by_segment: false,
            precompute_refs: false,
        };
        if !mining_opencl_default_solver_config(device_index, &raw_job, &mut raw_config) {
            return Err(last_error_message());
        }
        Ok(OpenclSolverConfig {
            batch_size: raw_config.batch_size,
            by_segment: raw_config.by_segment,
            precompute_refs: raw_config.precompute_refs,
        })
    }
}

pub fn find_best_benchmark_config(device_index: usize) -> Result<OpenclBenchmarkResult, String> {
    #[cfg(not(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    )))]
    {
        let _ = device_index;
        Err("当前平台未启用 OpenCL 后端".to_string())
    }
    #[cfg(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    ))]
    unsafe {
        let mut raw = mining_opencl_benchmark_result {
            batch_size: 0,
            by_segment: false,
            precompute_refs: false,
            attempts: 0,
            elapsed_ms: 0,
            attempts_per_second: 0.0,
        };
        if !mining_opencl_find_best_benchmark_config(device_index, &mut raw) {
            return Err(last_error_message());
        }
        Ok(OpenclBenchmarkResult {
            config: OpenclSolverConfig {
                batch_size: raw.batch_size,
                by_segment: raw.by_segment,
                precompute_refs: raw.precompute_refs,
            },
            attempts: raw.attempts,
            elapsed: Duration::from_millis(raw.elapsed_ms.max(0) as u64),
            attempts_per_second: raw.attempts_per_second,
        })
    }
}

pub fn mine_batch(
    device_index: usize,
    job: &OpenclJob<'_>,
    config: OpenclSolverConfig,
    start_nonce: u64,
) -> Result<OpenclMineResult, String> {
    #[cfg(not(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    )))]
    {
        let _ = (device_index, job, config, start_nonce);
        Err("当前平台未启用 OpenCL 后端".to_string())
    }
    #[cfg(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    ))]
    unsafe {
        let raw_job = mining_opencl_job {
            seed_ptr: job.seed_bytes.as_ptr(),
            seed_len: job.seed_bytes.len(),
            pass_prefix_ptr: job.pass_prefix.as_ptr(),
            pass_prefix_len: job.pass_prefix.len(),
            time_cost: job.time_cost,
            memory_cost_kib: job.memory_cost_kib,
            parallelism: job.parallelism,
            difficulty_bits: job.difficulty_bits,
        };
        let raw_config = mining_opencl_solver_config {
            batch_size: config.batch_size,
            by_segment: config.by_segment,
            precompute_refs: config.precompute_refs,
        };
        let mut raw_result = mining_opencl_mine_result {
            found: false,
            nonce: 0,
            attempts: 0,
            digest_hex: [0; 65],
        };
        if !mining_opencl_mine_batch(
            device_index,
            &raw_job,
            &raw_config,
            start_nonce,
            &mut raw_result,
        ) {
            return Err(last_error_message());
        }
        let digest_len = raw_result
            .digest_hex
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(raw_result.digest_hex.len());
        let digest_hex = String::from_utf8_lossy(&raw_result.digest_hex[..digest_len]).to_string();
        Ok(OpenclMineResult {
            found: raw_result.found,
            nonce: raw_result.nonce,
            attempts: raw_result.attempts,
            digest_hex,
        })
    }
}

pub fn create_session(
    device_index: usize,
    job: &OpenclJob<'_>,
    config: OpenclSolverConfig,
    start_nonce: u64,
) -> Result<OpenclMiningSession, String> {
    #[cfg(not(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    )))]
    {
        let _ = (device_index, job, config, start_nonce);
        Err("当前平台未启用 OpenCL 后端".to_string())
    }
    #[cfg(all(
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        mining_opencl_native_enabled
    ))]
    unsafe {
        let raw_job = mining_opencl_job {
            seed_ptr: job.seed_bytes.as_ptr(),
            seed_len: job.seed_bytes.len(),
            pass_prefix_ptr: job.pass_prefix.as_ptr(),
            pass_prefix_len: job.pass_prefix.len(),
            time_cost: job.time_cost,
            memory_cost_kib: job.memory_cost_kib,
            parallelism: job.parallelism,
            difficulty_bits: job.difficulty_bits,
        };
        let raw_config = mining_opencl_solver_config {
            batch_size: config.batch_size,
            by_segment: config.by_segment,
            precompute_refs: config.precompute_refs,
        };
        let raw = mining_opencl_session_create(device_index, &raw_job, &raw_config, start_nonce);
        if raw.is_null() {
            Err(last_error_message())
        } else {
            Ok(OpenclMiningSession { raw })
        }
    }
}

impl OpenclMiningSession {
    pub fn mine_next_batch(&mut self) -> Result<OpenclMineResult, String> {
        #[cfg(not(all(
            any(target_os = "macos", target_os = "linux", target_os = "windows"),
            mining_opencl_native_enabled
        )))]
        {
            Err("当前平台未启用 OpenCL 后端".to_string())
        }
        #[cfg(all(
            any(target_os = "macos", target_os = "linux", target_os = "windows"),
            mining_opencl_native_enabled
        ))]
        unsafe {
            let mut raw_result = mining_opencl_mine_result {
                found: false,
                nonce: 0,
                attempts: 0,
                digest_hex: [0; 65],
            };
            if !mining_opencl_session_mine_next_batch(self.raw, &mut raw_result) {
                return Err(last_error_message());
            }
            let digest_len = raw_result
                .digest_hex
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(raw_result.digest_hex.len());
            let digest_hex =
                String::from_utf8_lossy(&raw_result.digest_hex[..digest_len]).to_string();
            Ok(OpenclMineResult {
                found: raw_result.found,
                nonce: raw_result.nonce,
                attempts: raw_result.attempts,
                digest_hex,
            })
        }
    }
}

impl Drop for OpenclMiningSession {
    fn drop(&mut self) {
        #[cfg(all(
            any(target_os = "macos", target_os = "linux", target_os = "windows"),
            mining_opencl_native_enabled
        ))]
        unsafe {
            if !self.raw.is_null() {
                mining_opencl_session_destroy(self.raw);
                self.raw = std::ptr::null_mut();
            }
        }
    }
}

#[cfg(all(
    any(target_os = "macos", target_os = "linux", target_os = "windows"),
    mining_opencl_native_enabled
))]
fn decode_c_string(bytes: &[u8]) -> String {
    let len = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..len]).trim().to_string()
}

#[cfg(all(
    any(target_os = "macos", target_os = "linux", target_os = "windows"),
    mining_opencl_native_enabled
))]
unsafe fn last_error_message() -> String {
    let ptr = unsafe { mining_opencl_last_error_message() };
    if ptr.is_null() {
        return "OpenCL 后端返回了未知错误".to_string();
    }
    let text = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .trim()
        .to_string();
    if text.is_empty() {
        "OpenCL 后端返回了未知错误".to_string()
    } else {
        text
    }
}
