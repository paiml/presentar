//! GPU Process Analyzer
//!
//! Queries GPU process VRAM usage from nvidia-smi or AMDGPU.
//! Falls back gracefully if no GPU is available.

#![allow(clippy::uninlined_format_args)]

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{Analyzer, AnalyzerError};

/// GPU vendor type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Unknown,
}

impl GpuVendor {
    /// Get display name
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Nvidia => "NVIDIA",
            Self::Amd => "AMD",
            Self::Intel => "Intel",
            Self::Unknown => "Unknown",
        }
    }
}

/// GPU information
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// GPU index
    pub index: u32,
    /// GPU name
    pub name: String,
    /// GPU vendor
    pub vendor: GpuVendor,
    /// Total VRAM in bytes
    pub total_memory: u64,
    /// Used VRAM in bytes
    pub used_memory: u64,
    /// Free VRAM in bytes
    pub free_memory: u64,
    /// GPU utilization percentage (0-100)
    pub utilization: f32,
    /// Memory utilization percentage (0-100)
    pub memory_utilization: f32,
    /// Temperature in Celsius
    pub temperature: Option<f32>,
    /// Power draw in watts
    pub power_draw: Option<f32>,
    /// Power limit in watts
    pub power_limit: Option<f32>,
    /// Fan speed percentage
    pub fan_speed: Option<u32>,
    /// Driver version
    pub driver_version: Option<String>,
}

impl GpuInfo {
    /// Memory usage percentage
    pub fn memory_percent(&self) -> f32 {
        if self.total_memory > 0 {
            (self.used_memory as f64 / self.total_memory as f64 * 100.0) as f32
        } else {
            0.0
        }
    }

    /// Format memory for display
    pub fn display_memory(&self) -> String {
        format!(
            "{}/{} MB",
            self.used_memory / (1024 * 1024),
            self.total_memory / (1024 * 1024)
        )
    }
}

/// A process using GPU resources
#[derive(Debug, Clone)]
pub struct GpuProcess {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// GPU index
    pub gpu_index: u32,
    /// Used VRAM in bytes
    pub used_memory: u64,
    /// GPU utilization percentage (if available)
    pub gpu_util: Option<f32>,
    /// Memory utilization percentage (if available)
    pub mem_util: Option<f32>,
    /// Process type (Compute, Graphics, etc.)
    pub process_type: String,
}

impl GpuProcess {
    /// Format memory for display
    pub fn display_memory(&self) -> String {
        let mb = self.used_memory / (1024 * 1024);
        format!("{} MB", mb)
    }
}

/// GPU processes data
#[derive(Debug, Clone, Default)]
pub struct GpuProcsData {
    /// GPU information for each GPU
    pub gpus: Vec<GpuInfo>,
    /// Processes using GPUs
    pub processes: Vec<GpuProcess>,
    /// Detected vendor
    pub vendor: Option<GpuVendor>,
    /// Total VRAM usage across all GPUs
    pub total_vram_used: u64,
    /// Total VRAM available across all GPUs
    pub total_vram: u64,
    /// Average GPU utilization
    pub avg_gpu_util: f32,
    /// Highest temperature across GPUs
    pub max_temperature: Option<f32>,
    /// Total power draw across GPUs
    pub total_power: Option<f32>,
}

impl GpuProcsData {
    /// Get total number of GPU processes
    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    /// Get total number of GPUs
    pub fn gpu_count(&self) -> usize {
        self.gpus.len()
    }

    /// Check if any GPU is available
    pub fn has_gpu(&self) -> bool {
        !self.gpus.is_empty()
    }
}

/// Analyzer for GPU process statistics
pub struct GpuProcsAnalyzer {
    data: GpuProcsData,
    interval: Duration,
    vendor: Option<GpuVendor>,
    nvidia_smi_path: Option<String>,
}

impl Default for GpuProcsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuProcsAnalyzer {
    /// Create a new GPU processes analyzer
    pub fn new() -> Self {
        // Detect available GPU
        let (vendor, nvidia_smi_path) = Self::detect_gpu();

        Self {
            data: GpuProcsData::default(),
            interval: Duration::from_secs(2),
            vendor,
            nvidia_smi_path,
        }
    }

    /// Get the current GPU data
    pub fn data(&self) -> &GpuProcsData {
        &self.data
    }

    /// Detect available GPU and tools
    ///
    /// Uses same detection approach as ttop - directly invoke nvidia-smi
    /// rather than searching PATH with `which`.
    fn detect_gpu() -> (Option<GpuVendor>, Option<String>) {
        // Check for NVIDIA GPU by directly running nvidia-smi --version
        // This is more reliable than using `which` (matches ttop behavior)
        if let Ok(output) = Command::new("nvidia-smi").arg("--version").output() {
            if output.status.success() {
                // nvidia-smi is available, use it directly from PATH
                return (Some(GpuVendor::Nvidia), Some("nvidia-smi".to_string()));
            }
        }

        // Check for AMD GPU via sysfs - scan all cards, not just card0
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("card") && !name_str.contains('-') {
                    let vendor_path = entry.path().join("device/vendor");
                    if let Ok(vendor) = std::fs::read_to_string(&vendor_path) {
                        if vendor.trim() == "0x1002" {
                            return (Some(GpuVendor::Amd), None);
                        }
                    }
                }
            }
        }

        // Check for Intel GPU
        if Path::new("/sys/class/drm/card0/gt/gt0").exists() {
            return (Some(GpuVendor::Intel), None);
        }

        (None, None)
    }

    /// Query NVIDIA GPU using nvidia-smi
    fn query_nvidia(&mut self) -> Result<(), AnalyzerError> {
        let nvidia_smi = self
            .nvidia_smi_path
            .as_ref()
            .ok_or_else(|| AnalyzerError::NotAvailable("nvidia-smi not found".to_string()))?;

        // Query GPU info
        let output = Command::new(nvidia_smi)
            .args([
                "--query-gpu=index,name,memory.total,memory.used,memory.free,utilization.gpu,utilization.memory,temperature.gpu,power.draw,power.limit,fan.speed,driver_version",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .map_err(|e| AnalyzerError::IoError(format!("nvidia-smi failed: {}", e)))?;

        if !output.status.success() {
            return Err(AnalyzerError::IoError(
                "nvidia-smi returned error".to_string(),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut gpus = Vec::new();

        for line in stdout.lines() {
            if let Some(gpu) = self.parse_nvidia_gpu_line(line) {
                gpus.push(gpu);
            }
        }

        // Query per-process memory usage
        let proc_output = Command::new(nvidia_smi)
            .args([
                "--query-compute-apps=pid,process_name,gpu_index,used_memory",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .map_err(|e| {
                AnalyzerError::IoError(format!("nvidia-smi process query failed: {}", e))
            })?;

        let mut processes = Vec::new();
        let proc_stdout = String::from_utf8_lossy(&proc_output.stdout);

        for line in proc_stdout.lines() {
            if let Some(proc) = self.parse_nvidia_process_line(line) {
                processes.push(proc);
            }
        }

        // Also query graphics processes
        let graphics_output = Command::new(nvidia_smi)
            .args([
                "--query-graphics-apps=pid,process_name,gpu_index,used_memory",
                "--format=csv,noheader,nounits",
            ])
            .output();

        if let Ok(gfx_output) = graphics_output {
            let gfx_stdout = String::from_utf8_lossy(&gfx_output.stdout);
            for line in gfx_stdout.lines() {
                if let Some(mut proc) = self.parse_nvidia_process_line(line) {
                    proc.process_type = "Graphics".to_string();
                    // Avoid duplicates
                    if !processes.iter().any(|p| p.pid == proc.pid) {
                        processes.push(proc);
                    }
                }
            }
        }

        // Calculate aggregates
        let total_vram = gpus.iter().map(|g| g.total_memory).sum();
        let total_vram_used = gpus.iter().map(|g| g.used_memory).sum();
        let avg_gpu_util = if gpus.is_empty() {
            0.0
        } else {
            gpus.iter().map(|g| g.utilization).sum::<f32>() / gpus.len() as f32
        };
        let max_temperature = gpus.iter().filter_map(|g| g.temperature).reduce(f32::max);
        let total_power = {
            let powers: Vec<f32> = gpus.iter().filter_map(|g| g.power_draw).collect();
            if powers.is_empty() {
                None
            } else {
                Some(powers.iter().sum())
            }
        };

        self.data = GpuProcsData {
            gpus,
            processes,
            vendor: Some(GpuVendor::Nvidia),
            total_vram_used,
            total_vram,
            avg_gpu_util,
            max_temperature,
            total_power,
        };

        Ok(())
    }

    /// Parse a line of nvidia-smi GPU output
    fn parse_nvidia_gpu_line(&self, line: &str) -> Option<GpuInfo> {
        let parts: Vec<&str> = line.split(", ").collect();
        if parts.len() < 12 {
            return None;
        }

        let index: u32 = parts[0].parse().ok()?;
        let name = parts[1].to_string();
        let total_memory: u64 = parts[2].parse::<u64>().ok()? * 1024 * 1024; // MiB to bytes
        let used_memory: u64 = parts[3].parse::<u64>().ok()? * 1024 * 1024;
        let free_memory: u64 = parts[4].parse::<u64>().ok()? * 1024 * 1024;
        let utilization: f32 = parts[5].parse().unwrap_or(0.0);
        let memory_utilization: f32 = parts[6].parse().unwrap_or(0.0);
        let temperature: Option<f32> = parts[7].parse().ok();
        let power_draw: Option<f32> = parts[8].parse().ok();
        let power_limit: Option<f32> = parts[9].parse().ok();
        let fan_speed: Option<u32> = parts[10].parse().ok();
        let driver_version = if parts[11].is_empty() || parts[11] == "[N/A]" {
            None
        } else {
            Some(parts[11].to_string())
        };

        Some(GpuInfo {
            index,
            name,
            vendor: GpuVendor::Nvidia,
            total_memory,
            used_memory,
            free_memory,
            utilization,
            memory_utilization,
            temperature,
            power_draw,
            power_limit,
            fan_speed,
            driver_version,
        })
    }

    /// Parse a line of nvidia-smi process output
    fn parse_nvidia_process_line(&self, line: &str) -> Option<GpuProcess> {
        let parts: Vec<&str> = line.split(", ").collect();
        if parts.len() < 4 {
            return None;
        }

        let pid: u32 = parts[0].parse().ok()?;
        let name = parts[1].to_string();
        let gpu_index: u32 = parts[2].parse().ok()?;
        let used_memory: u64 = parts[3].parse::<u64>().ok()? * 1024 * 1024; // MiB to bytes

        Some(GpuProcess {
            pid,
            name,
            gpu_index,
            used_memory,
            gpu_util: None,
            mem_util: None,
            process_type: "Compute".to_string(),
        })
    }

    /// Query AMD GPU via sysfs
    fn query_amd(&mut self) -> Result<(), AnalyzerError> {
        let mut gpus = Vec::new();
        let processes = Vec::new();

        // Scan for AMD GPUs in /sys/class/drm/
        let drm_path = Path::new("/sys/class/drm");
        if !drm_path.exists() {
            return Err(AnalyzerError::NotAvailable("DRM not available".to_string()));
        }

        for entry in std::fs::read_dir(drm_path)
            .map_err(|e| AnalyzerError::IoError(format!("Failed to read /sys/class/drm: {}", e)))?
        {
            let entry = entry.map_err(|e| AnalyzerError::IoError(e.to_string()))?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Only process card* directories (not render nodes)
            if !name_str.starts_with("card") || name_str.contains('-') {
                continue;
            }

            let card_path = entry.path();
            let device_path = card_path.join("device");

            // Check if it's an AMD GPU
            let vendor_path = device_path.join("vendor");
            if let Ok(vendor) = std::fs::read_to_string(&vendor_path) {
                if vendor.trim() != "0x1002" {
                    continue; // Not AMD
                }
            } else {
                continue;
            }

            let index: u32 = name_str
                .strip_prefix("card")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            // Read GPU info from hwmon
            let mut gpu = GpuInfo {
                index,
                name: Self::read_amd_gpu_name(&device_path),
                vendor: GpuVendor::Amd,
                total_memory: Self::read_amd_vram_total(&device_path),
                used_memory: Self::read_amd_vram_used(&device_path),
                free_memory: 0,
                utilization: Self::read_amd_gpu_busy(&device_path),
                memory_utilization: 0.0,
                temperature: Self::read_amd_temperature(&device_path),
                power_draw: Self::read_amd_power(&device_path),
                power_limit: None,
                fan_speed: Self::read_amd_fan_speed(&device_path),
                driver_version: None,
            };

            gpu.free_memory = gpu.total_memory.saturating_sub(gpu.used_memory);
            gpu.memory_utilization = gpu.memory_percent();

            gpus.push(gpu);
        }

        // AMD doesn't have easy per-process GPU memory tracking
        // We'd need to parse /proc/[pid]/fdinfo for DRM clients

        // Calculate aggregates
        let total_vram = gpus.iter().map(|g| g.total_memory).sum();
        let total_vram_used = gpus.iter().map(|g| g.used_memory).sum();
        let avg_gpu_util = if gpus.is_empty() {
            0.0
        } else {
            gpus.iter().map(|g| g.utilization).sum::<f32>() / gpus.len() as f32
        };
        let max_temperature = gpus.iter().filter_map(|g| g.temperature).reduce(f32::max);
        let total_power = {
            let powers: Vec<f32> = gpus.iter().filter_map(|g| g.power_draw).collect();
            if powers.is_empty() {
                None
            } else {
                Some(powers.iter().sum())
            }
        };

        self.data = GpuProcsData {
            gpus,
            processes,
            vendor: Some(GpuVendor::Amd),
            total_vram_used,
            total_vram,
            avg_gpu_util,
            max_temperature,
            total_power,
        };

        Ok(())
    }

    /// Read AMD GPU name
    fn read_amd_gpu_name(device_path: &Path) -> String {
        let product_path = device_path.join("product_name");
        if let Ok(name) = std::fs::read_to_string(&product_path) {
            return name.trim().to_string();
        }

        let uevent_path = device_path.join("uevent");
        if let Ok(uevent) = std::fs::read_to_string(&uevent_path) {
            for line in uevent.lines() {
                if let Some(name) = line.strip_prefix("PCI_SLOT_NAME=") {
                    return format!("AMD GPU {}", name);
                }
            }
        }

        "AMD GPU".to_string()
    }

    /// Read AMD VRAM total
    fn read_amd_vram_total(device_path: &Path) -> u64 {
        let path = device_path.join("mem_info_vram_total");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Read AMD VRAM used
    fn read_amd_vram_used(device_path: &Path) -> u64 {
        let path = device_path.join("mem_info_vram_used");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Read AMD GPU busy percentage
    fn read_amd_gpu_busy(device_path: &Path) -> f32 {
        let path = device_path.join("gpu_busy_percent");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0)
    }

    /// Read AMD temperature
    fn read_amd_temperature(device_path: &Path) -> Option<f32> {
        // Try hwmon
        let hwmon_path = device_path.join("hwmon");
        if let Ok(entries) = std::fs::read_dir(&hwmon_path) {
            for entry in entries.flatten() {
                let temp_path = entry.path().join("temp1_input");
                if let Ok(temp) = std::fs::read_to_string(&temp_path) {
                    if let Ok(millicelsius) = temp.trim().parse::<i32>() {
                        return Some(millicelsius as f32 / 1000.0);
                    }
                }
            }
        }
        None
    }

    /// Read AMD power draw
    fn read_amd_power(device_path: &Path) -> Option<f32> {
        let hwmon_path = device_path.join("hwmon");
        if let Ok(entries) = std::fs::read_dir(&hwmon_path) {
            for entry in entries.flatten() {
                let power_path = entry.path().join("power1_average");
                if let Ok(power) = std::fs::read_to_string(&power_path) {
                    if let Ok(microwatts) = power.trim().parse::<u64>() {
                        return Some(microwatts as f32 / 1_000_000.0);
                    }
                }
            }
        }
        None
    }

    /// Read AMD fan speed
    fn read_amd_fan_speed(device_path: &Path) -> Option<u32> {
        let hwmon_path = device_path.join("hwmon");
        if let Ok(entries) = std::fs::read_dir(&hwmon_path) {
            for entry in entries.flatten() {
                let pwm_path = entry.path().join("pwm1");
                let max_path = entry.path().join("pwm1_max");

                if let (Ok(pwm), Ok(max)) = (
                    std::fs::read_to_string(&pwm_path),
                    std::fs::read_to_string(&max_path),
                ) {
                    if let (Ok(pwm_val), Ok(max_val)) =
                        (pwm.trim().parse::<u32>(), max.trim().parse::<u32>())
                    {
                        if max_val > 0 {
                            return Some(pwm_val * 100 / max_val);
                        }
                    }
                }
            }
        }
        None
    }
}

impl Analyzer for GpuProcsAnalyzer {
    fn name(&self) -> &'static str {
        "gpu_procs"
    }

    fn collect(&mut self) -> Result<(), AnalyzerError> {
        match self.vendor {
            Some(GpuVendor::Nvidia) => self.query_nvidia(),
            Some(GpuVendor::Amd) => self.query_amd(),
            Some(GpuVendor::Intel) => {
                // Intel GPU support is limited
                Err(AnalyzerError::NotAvailable(
                    "Intel GPU not fully supported".to_string(),
                ))
            }
            _ => Err(AnalyzerError::NotAvailable("No GPU detected".to_string())),
        }
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    fn available(&self) -> bool {
        self.vendor.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_vendor_display() {
        assert_eq!(GpuVendor::Nvidia.as_str(), "NVIDIA");
        assert_eq!(GpuVendor::Amd.as_str(), "AMD");
        assert_eq!(GpuVendor::Intel.as_str(), "Intel");
    }

    #[test]
    fn test_gpu_info_memory_percent() {
        let gpu = GpuInfo {
            index: 0,
            name: "Test GPU".to_string(),
            vendor: GpuVendor::Nvidia,
            total_memory: 8 * 1024 * 1024 * 1024, // 8 GB
            used_memory: 4 * 1024 * 1024 * 1024,  // 4 GB
            free_memory: 4 * 1024 * 1024 * 1024,
            utilization: 50.0,
            memory_utilization: 50.0,
            temperature: Some(65.0),
            power_draw: Some(150.0),
            power_limit: Some(250.0),
            fan_speed: Some(40),
            driver_version: Some("535.154.05".to_string()),
        };

        assert!((gpu.memory_percent() - 50.0).abs() < 0.01);
        assert_eq!(gpu.display_memory(), "4096/8192 MB");
    }

    #[test]
    fn test_gpu_process_display() {
        let proc = GpuProcess {
            pid: 1234,
            name: "python".to_string(),
            gpu_index: 0,
            used_memory: 2048 * 1024 * 1024, // 2 GB
            gpu_util: Some(80.0),
            mem_util: Some(25.0),
            process_type: "Compute".to_string(),
        };

        assert_eq!(proc.display_memory(), "2048 MB");
    }

    #[test]
    fn test_gpu_procs_data_empty() {
        let data = GpuProcsData::default();
        assert!(!data.has_gpu());
        assert_eq!(data.gpu_count(), 0);
        assert_eq!(data.process_count(), 0);
    }

    #[test]
    fn test_analyzer_creation() {
        let analyzer = GpuProcsAnalyzer::new();
        // Just verify it doesn't panic
        let _ = analyzer.available();
    }

    #[test]
    fn test_parse_nvidia_gpu_line() {
        let analyzer = GpuProcsAnalyzer::new();
        let line = "0, NVIDIA GeForce RTX 3080, 10240, 4096, 6144, 45, 40, 65, 150.00, 320.00, 35, 535.154.05";

        let gpu = analyzer.parse_nvidia_gpu_line(line);
        assert!(gpu.is_some());

        let gpu = gpu.unwrap();
        assert_eq!(gpu.index, 0);
        assert!(gpu.name.contains("RTX 3080"));
        assert_eq!(gpu.total_memory, 10240 * 1024 * 1024);
        assert_eq!(gpu.used_memory, 4096 * 1024 * 1024);
        assert!((gpu.utilization - 45.0).abs() < 0.01);
        assert!((gpu.temperature.unwrap() - 65.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_nvidia_process_line() {
        let analyzer = GpuProcsAnalyzer::new();
        let line = "1234, python3, 0, 2048";

        let proc = analyzer.parse_nvidia_process_line(line);
        assert!(proc.is_some());

        let proc = proc.unwrap();
        assert_eq!(proc.pid, 1234);
        assert_eq!(proc.name, "python3");
        assert_eq!(proc.gpu_index, 0);
        assert_eq!(proc.used_memory, 2048 * 1024 * 1024);
    }
}
