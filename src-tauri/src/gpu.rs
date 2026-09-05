//! Hardware capability detection for the local inference pipeline.
//!
//! This module separates adapter discovery from DirectML runtime probing.
//! Adapter discovery alone never claims ONNX models can run on the GPU;
//! DirectML availability is reported only after runtime/provider probing.

use std::path::Path;

use serde::{Deserialize, Serialize};

const DISCRETE_MEMORY_HEURISTIC_BYTES: u64 = 256 * 1024 * 1024;
const GIB: u64 = 1024 * 1024 * 1024;

/// The CPU path deliberately stays conservative. A wider value is only
/// allowed after DirectML has passed provider and model-session checks.
pub const CPU_ANALYSIS_BATCH_LIMIT: usize = 8;
pub const MAX_DIRECTML_ANALYSIS_BATCH_SIZE: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GpuProviderStatus {
    pub id: String,
    pub state: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GpuAdapterInfo {
    pub index: u32,
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_vram_bytes: u64,
    pub shared_system_memory_bytes: u64,
    pub is_software: bool,
    /// A hardware adapter with dedicated memory is a first-pass signal for
    /// the product's "independent GPU" option. Provider initialization is
    /// still the final availability check.
    pub is_discrete_candidate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GpuCapabilities {
    pub status: String,
    pub message: String,
    pub adapters: Vec<GpuAdapterInfo>,
    pub selected_adapter_index: Option<u32>,
    pub dedicated_gpu_available: bool,
    pub directml: GpuProviderStatus,
    /// A conservative capacity tier derived from dedicated VRAM. This is a
    /// scheduling ceiling, not a claim about live VRAM usage.
    pub recommended_analysis_batch_size: usize,
}

/// Return the initial DirectML batch ceiling for a physical adapter.
///
/// The model stack contains several sessions (Places365, SigLIP 2, PicoDet
/// and YuNet), so the tier intentionally leaves headroom instead of mapping
/// every byte of VRAM to a larger batch. A later runtime failure can still
/// reduce the batch further in the worker.
pub fn recommended_directml_batch_size(dedicated_vram_bytes: u64) -> usize {
    if dedicated_vram_bytes < 2 * GIB {
        2
    } else if dedicated_vram_bytes < 4 * GIB {
        4
    } else if dedicated_vram_bytes < 6 * GIB {
        8
    } else if dedicated_vram_bytes < 8 * GIB {
        12
    } else if dedicated_vram_bytes < 12 * GIB {
        16
    } else if dedicated_vram_bytes < 16 * GIB {
        24
    } else {
        MAX_DIRECTML_ANALYSIS_BATCH_SIZE
    }
}

pub fn analysis_batch_limit_for_backend(
    capabilities: &GpuCapabilities,
    backend: crate::semantic::ExecutionBackend,
) -> usize {
    if backend == crate::semantic::ExecutionBackend::DirectMl
        && capabilities.selected_adapter_index.is_some()
    {
        capabilities
            .recommended_analysis_batch_size
            .clamp(1, MAX_DIRECTML_ANALYSIS_BATCH_SIZE)
    } else {
        CPU_ANALYSIS_BATCH_LIMIT
    }
}

pub fn detect_gpu_capabilities_with_runtime(runtime_path: &Path) -> GpuCapabilities {
    let mut capabilities = detect_gpu_capabilities();
    if capabilities.dedicated_gpu_available {
        capabilities.directml = probe_directml_provider(runtime_path);
        capabilities.recommended_analysis_batch_size = if capabilities.directml.state == "ready" {
            capabilities
                .selected_adapter_index
                .and_then(|selected_index| {
                    capabilities
                        .adapters
                        .iter()
                        .find(|adapter| adapter.index == selected_index)
                })
                .map(|adapter| recommended_directml_batch_size(adapter.dedicated_vram_bytes))
                .unwrap_or(CPU_ANALYSIS_BATCH_LIMIT)
        } else {
            CPU_ANALYSIS_BATCH_LIMIT
        };
        capabilities.message = match capabilities.directml.state.as_str() {
            "ready" => "已检测到独立 GPU，DirectML Provider 已就绪；分析可按设置使用 GPU。".into(),
            "error" => format!(
                "已检测到独立 GPU，但 DirectML 初始化失败；当前分析使用 CPU。{}",
                capabilities.directml.message
            ),
            _ => "已检测到独立 GPU，但 DirectML Provider 不可用；当前分析使用 CPU。".into(),
        };
    } else {
        capabilities.directml =
            directml_unavailable("当前未检测到满足策略的独立 GPU，DirectML 加速选项不可用。");
        capabilities.recommended_analysis_batch_size = CPU_ANALYSIS_BATCH_LIMIT;
    }
    capabilities
}

pub fn probe_directml_provider(runtime_path: &Path) -> GpuProviderStatus {
    if !cfg!(windows) {
        return directml_unavailable("DirectML 仅在 Windows 桌面版本中可用。");
    }
    if let Err(error) =
        crate::semantic::verify_sha256(runtime_path, crate::semantic::RUNTIME_SHA256)
    {
        return directml_error(format!("ONNX Runtime 主 DLL 校验失败：{error}"));
    }
    if let Err(error) = crate::semantic::initialize_ort(runtime_path) {
        return directml_error(format!("ONNX Runtime 初始化失败：{error}"));
    }

    use ort::ep::ExecutionProvider;
    match ort::ep::DirectML::default().is_available() {
        Ok(true) => GpuProviderStatus {
            id: "directml".into(),
            state: "ready".into(),
            message: "DirectML Provider 已注册；模型装载时仍会进行会话自检。".into(),
        },
        Ok(false) => {
            directml_unavailable("当前分发的 ONNX Runtime 未包含可用的 DirectML Provider。")
        }
        Err(error) => directml_error(format!("DirectML Provider 探测失败：{error}")),
    }
}

pub fn detect_gpu_capabilities() -> GpuCapabilities {
    #[cfg(windows)]
    {
        detect_windows_gpu_capabilities()
    }

    #[cfg(not(windows))]
    {
        GpuCapabilities {
            status: "unsupported_platform".into(),
            message: "当前版本只在 Windows 上检测独立 GPU；分析仍使用 CPU。".into(),
            adapters: Vec::new(),
            selected_adapter_index: None,
            dedicated_gpu_available: false,
            directml: directml_not_configured(),
            recommended_analysis_batch_size: CPU_ANALYSIS_BATCH_LIMIT,
        }
    }
}

fn directml_not_configured() -> GpuProviderStatus {
    GpuProviderStatus {
        id: "directml".into(),
        state: "not_configured".into(),
        message: "DirectML Provider 尚未完成运行库自检，当前分析使用 CPU。".into(),
    }
}

fn directml_unavailable(message: impl Into<String>) -> GpuProviderStatus {
    GpuProviderStatus {
        id: "directml".into(),
        state: "unavailable".into(),
        message: message.into(),
    }
}

fn directml_error(message: impl Into<String>) -> GpuProviderStatus {
    GpuProviderStatus {
        id: "directml".into(),
        state: "error".into(),
        message: message.into(),
    }
}

fn is_discrete_candidate(is_software: bool, dedicated_vram_bytes: u64) -> bool {
    !is_software && dedicated_vram_bytes >= DISCRETE_MEMORY_HEURISTIC_BYTES
}

#[cfg(windows)]
fn detect_windows_gpu_capabilities() -> GpuCapabilities {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
    };

    let factory = match unsafe { CreateDXGIFactory1::<IDXGIFactory1>() } {
        Ok(factory) => factory,
        Err(error) => {
            return GpuCapabilities {
                status: "detection_failed".into(),
                message: format!("无法枚举 Windows 图形适配器：{error}"),
                adapters: Vec::new(),
                selected_adapter_index: None,
                dedicated_gpu_available: false,
                directml: directml_not_configured(),
                recommended_analysis_batch_size: CPU_ANALYSIS_BATCH_LIMIT,
            };
        }
    };

    let mut adapters = Vec::new();
    for index in 0..u32::MAX {
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(_) => break,
        };
        let description = match unsafe { adapter.GetDesc1() } {
            Ok(description) => description,
            Err(_) => continue,
        };
        let is_software = (description.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
        let dedicated_vram_bytes = description.DedicatedVideoMemory as u64;
        adapters.push(GpuAdapterInfo {
            index,
            name: utf16_string(&description.Description),
            vendor_id: description.VendorId,
            device_id: description.DeviceId,
            dedicated_vram_bytes,
            shared_system_memory_bytes: description.SharedSystemMemory as u64,
            is_software,
            is_discrete_candidate: is_discrete_candidate(is_software, dedicated_vram_bytes),
        });
    }

    let selected_adapter_index = adapters
        .iter()
        .filter(|adapter| adapter.is_discrete_candidate)
        .max_by_key(|adapter| adapter.dedicated_vram_bytes)
        .map(|adapter| adapter.index);
    let dedicated_gpu_available = selected_adapter_index.is_some();
    let status = if dedicated_gpu_available {
        "hardware_detected"
    } else if adapters.iter().any(|adapter| !adapter.is_software) {
        "integrated_only"
    } else {
        "no_gpu"
    };
    let recommended_analysis_batch_size = selected_adapter_index
        .and_then(|selected_index| {
            adapters
                .iter()
                .find(|adapter| adapter.index == selected_index)
        })
        .map(|adapter| recommended_directml_batch_size(adapter.dedicated_vram_bytes))
        .unwrap_or(CPU_ANALYSIS_BATCH_LIMIT);
    let message = if dedicated_gpu_available {
        "已检测到独立 GPU；DirectML Provider 尚未接入，当前分析仍使用 CPU。".into()
    } else if status == "integrated_only" {
        "仅检测到集成或共享显存适配器；按当前策略不启用独立 GPU 加速。".into()
    } else {
        "未检测到可用的硬件 GPU；当前分析使用 CPU。".into()
    };

    GpuCapabilities {
        status: status.into(),
        message,
        adapters,
        selected_adapter_index,
        dedicated_gpu_available,
        directml: directml_not_configured(),
        recommended_analysis_batch_size,
    }
}

#[cfg(windows)]
fn utf16_string(value: &[u16]) -> String {
    let end = value
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end]).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        analysis_batch_limit_for_backend, is_discrete_candidate, recommended_directml_batch_size,
    };
    use crate::semantic::ExecutionBackend;

    #[test]
    fn software_adapter_is_never_classified_as_discrete() {
        assert!(!is_discrete_candidate(true, 8 * 1024 * 1024 * 1024));
    }

    #[test]
    fn dedicated_memory_is_only_a_first_pass_candidate_signal() {
        assert!(!is_discrete_candidate(false, 128 * 1024 * 1024));
        assert!(is_discrete_candidate(false, 4 * 1024 * 1024 * 1024));
    }

    #[test]
    fn directml_batch_tiers_leave_headroom_for_the_model_stack() {
        assert_eq!(recommended_directml_batch_size(super::GIB), 2);
        assert_eq!(recommended_directml_batch_size(4 * super::GIB), 8);
        assert_eq!(recommended_directml_batch_size(8 * super::GIB), 16);
        assert_eq!(recommended_directml_batch_size(16 * super::GIB), 32);
    }

    #[test]
    fn directml_uses_the_selected_adapter_capacity_tier() {
        let capabilities = super::GpuCapabilities {
            status: "hardware_detected".into(),
            message: String::new(),
            adapters: Vec::new(),
            selected_adapter_index: Some(0),
            dedicated_gpu_available: true,
            directml: super::GpuProviderStatus {
                id: "directml".into(),
                state: "ready".into(),
                message: String::new(),
            },
            recommended_analysis_batch_size: 16,
        };

        assert_eq!(
            analysis_batch_limit_for_backend(&capabilities, ExecutionBackend::DirectMl),
            16
        );
    }

    #[test]
    fn cpu_backend_never_uses_the_directml_capacity_tier() {
        let capabilities = super::GpuCapabilities {
            status: "hardware_detected".into(),
            message: String::new(),
            adapters: Vec::new(),
            selected_adapter_index: Some(0),
            dedicated_gpu_available: true,
            directml: super::GpuProviderStatus {
                id: "directml".into(),
                state: "ready".into(),
                message: String::new(),
            },
            recommended_analysis_batch_size: 32,
        };
        assert_eq!(
            analysis_batch_limit_for_backend(&capabilities, ExecutionBackend::Cpu),
            super::CPU_ANALYSIS_BATCH_LIMIT
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_detection_returns_a_consistent_provider_contract() {
        let capabilities = super::detect_gpu_capabilities();
        assert!(
            [
                "hardware_detected",
                "integrated_only",
                "no_gpu",
                "detection_failed"
            ]
            .contains(&capabilities.status.as_str())
        );
        assert_eq!(
            capabilities.dedicated_gpu_available,
            capabilities.selected_adapter_index.is_some()
        );
        assert_eq!(capabilities.directml.state, "not_configured");
        if let Some(selected_index) = capabilities.selected_adapter_index {
            let selected = capabilities
                .adapters
                .iter()
                .find(|adapter| adapter.index == selected_index)
                .expect("selected adapter must be present in the enumeration");
            assert!(selected.is_discrete_candidate);
            assert!(!selected.is_software);
        }
    }
}
