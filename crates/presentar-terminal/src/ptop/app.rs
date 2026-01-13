//! Application state and data collectors for ptop.
//!
//! Mirrors ttop's app.rs - maintains system state and history.

use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Duration;

use sysinfo::{
    CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind, ProcessesToUpdate,
    System, Users,
};

use super::config::{DetailLevel, FilesViewMode, PanelType, PtopConfig, SignalType};
use super::ui::{read_gpu_info, GpuInfo};

/// Parse a single meminfo line to extract value in bytes.
/// Format: "Label:          1234567 kB"
#[cfg(target_os = "linux")]
fn parse_meminfo_line(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    parts.get(1).and_then(|s| s.parse::<u64>().ok()).map(|kb| kb * 1024)
}

/// Check if line is the Cached memory line (not SwapCached).
#[cfg(target_os = "linux")]
fn is_cached_line(line: &str) -> bool {
    line.starts_with("Cached:") && !line.starts_with("CachedSwap")
}

/// Read cached memory from /proc/meminfo (Linux only).
/// Returns bytes, or 0 if unavailable.
#[cfg(target_os = "linux")]
fn read_cached_memory() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| {
            contents.lines()
                .find(|line| is_cached_line(line))
                .and_then(parse_meminfo_line)
        })
        .unwrap_or(0)
}

#[cfg(not(target_os = "linux"))]
fn read_cached_memory() -> u64 {
    // On non-Linux systems, return 0 (cached memory not available via /proc)
    0
}

/// Map CCD temperatures to core array (AMD processors).
///
/// AMD Threadripper/EPYC: cores are distributed across CCDs.
/// Each CCD gets an equal share of cores.
#[cfg(target_os = "linux")]
fn map_ccd_temps_to_cores(
    ccd_temps: &std::collections::HashMap<String, f32>,
    temps: &mut [f32],
) {
    let core_count = temps.len();
    let cores_per_ccd = core_count / 4;

    // Assign CCD temperatures to corresponding cores.
    for (ccd_idx, label) in ["Tccd1", "Tccd2", "Tccd3", "Tccd4"].iter().enumerate() {
        if let Some(&temp) = ccd_temps.get(*label) {
            let start = cores_per_ccd * ccd_idx;
            let end = if ccd_idx == 3 {
                core_count
            } else {
                (cores_per_ccd * (ccd_idx + 1)).min(core_count)
            };
            for i in start..end {
                temps[i] = temp;
            }
        }
    }

    // Fallback to Tctl if no CCD temps found
    if temps.iter().all(|&t| t == 0.0) {
        if let Some(&tctl) = ccd_temps.get("Tctl") {
            temps.fill(tctl);
        }
    }
}

/// Read AMD k10temp/zenpower temperatures from hwmon path.
#[cfg(target_os = "linux")]
fn read_amd_temps(path: &std::path::Path, temps: &mut [f32]) -> bool {
    use std::collections::HashMap;
    use std::fs;

    let mut ccd_temps: HashMap<String, f32> = HashMap::new();

    // Read temperature sensor labels and values.
    for i in 1..=10 {
        let label_path = path.join(format!("temp{i}_label"));
        let input_path = path.join(format!("temp{i}_input"));

        if let (Ok(label), Ok(input)) = (fs::read_to_string(&label_path), fs::read_to_string(&input_path)) {
            let label = label.trim().to_string();
            if let Ok(millidegrees) = input.trim().parse::<i64>() {
                ccd_temps.insert(label, millidegrees as f32 / 1000.0);
            }
        }
    }

    if ccd_temps.is_empty() {
        return false;
    }

    map_ccd_temps_to_cores(&ccd_temps, temps);
    true
}

/// Read Intel coretemp temperatures from hwmon path.
#[cfg(target_os = "linux")]
fn read_intel_temps(path: &std::path::Path, temps: &mut [f32]) -> bool {
    use std::fs;

    // Intel: temp2_input = Core 0, temp3_input = Core 1, etc.
    for (i, temp) in temps.iter_mut().enumerate() {
        let temp_file = path.join(format!("temp{}_input", i + 2));
        if let Ok(temp_str) = fs::read_to_string(&temp_file) {
            if let Ok(millidegrees) = temp_str.trim().parse::<i64>() {
                *temp = millidegrees as f32 / 1000.0;
            }
        }
    }

    // Use package temperature when per-core temps unavailable.
    if temps.iter().all(|&t| t == 0.0) {
        let temp_file = path.join("temp1_input");
        if let Ok(temp_str) = fs::read_to_string(&temp_file) {
            if let Ok(millidegrees) = temp_str.trim().parse::<i64>() {
                temps.fill(millidegrees as f32 / 1000.0);
                return true;
            }
        }
        return false;
    }
    true
}

/// Try to read CPU temperatures from a single hwmon device.
#[cfg(target_os = "linux")]
fn try_read_hwmon_temps(path: &std::path::Path, temps: &mut [f32]) -> bool {
    use std::fs;

    let Ok(name) = fs::read_to_string(path.join("name")) else {
        return false;
    };

    match name.trim() {
        "k10temp" | "zenpower" => read_amd_temps(path, temps),
        "coretemp" => read_intel_temps(path, temps),
        _ => false,
    }
}

/// Read per-core CPU temperatures from /sys/class/hwmon (Linux only).
/// Returns temperatures in °C, or zeros if unavailable.
#[cfg(target_os = "linux")]
fn read_core_temperatures(core_count: usize) -> Vec<f32> {
    use std::fs;
    use std::path::Path;

    let mut temps = vec![0.0f32; core_count];
    let hwmon_dir = Path::new("/sys/class/hwmon");

    let Ok(entries) = fs::read_dir(hwmon_dir) else {
        return temps;
    };

    for entry in entries.flatten() {
        if try_read_hwmon_temps(&entry.path(), &mut temps) {
            return temps;
        }
    }

    temps
}

#[cfg(not(target_os = "linux"))]
fn read_core_temperatures(core_count: usize) -> Vec<f32> {
    // On non-Linux systems, return zeros
    vec![0.0f32; core_count]
}

use super::analyzers::{
    AnalyzerRegistry, ConnectionsData, DiskEntropyData, DiskIoData, FileAnalyzerData, PsiData,
    SensorHealthData, TreemapData,
};
use crate::{AsyncCollector, Snapshot};

/// Metrics snapshot sent from background collector to main thread.
/// Contains only the data needed for rendering - no heavy objects.
#[derive(Clone)]
pub struct MetricsSnapshot {
    // CPU metrics
    pub cpu_avg: f64,
    pub per_core_percent: Vec<f64>,
    pub per_core_freq: Vec<u64>, // MHz - SPEC-024 async update requirement
    pub per_core_temp: Vec<f32>, // °C - SPEC-024 async update requirement
    pub load_avg: sysinfo::LoadAvg,

    // Memory metrics
    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_available: u64,
    pub mem_cached: u64,
    pub swap_total: u64,
    pub swap_used: u64,

    // Network metrics (bytes since boot)
    pub net_rx: u64,
    pub net_tx: u64,

    // GPU metrics
    pub gpu_info: Option<GpuInfo>,

    // Process list (extracted from sysinfo::System)
    pub processes: Vec<ProcessInfo>,

    // Disk info (extracted from sysinfo::Disks)
    pub disk_info: Vec<DiskInfo>,

    // Network interfaces (extracted from sysinfo::Networks)
    pub network_info: Vec<NetworkInfo>,

    // Analyzer results
    pub psi_data: Option<PsiData>,
    pub connections_data: Option<ConnectionsData>,
    pub treemap_data: Option<TreemapData>,
    pub sensor_health_data: Option<SensorHealthData>,
    pub disk_io_data: Option<DiskIoData>,
    pub disk_entropy_data: Option<DiskEntropyData>,
    pub file_analyzer_data: Option<FileAnalyzerData>,
}

/// Lightweight process info for rendering
#[derive(Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory: u64,
    pub user: String,
    pub cmd: String,
}

/// Lightweight disk info for rendering
#[derive(Clone)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
    pub file_system: String,
}

/// Lightweight network interface info for rendering
#[derive(Clone)]
pub struct NetworkInfo {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
}

impl Snapshot for MetricsSnapshot {
    fn empty() -> Self {
        Self {
            cpu_avg: 0.0,
            per_core_percent: Vec::new(),
            per_core_freq: Vec::new(),
            per_core_temp: Vec::new(),
            load_avg: sysinfo::LoadAvg {
                one: 0.0,
                five: 0.0,
                fifteen: 0.0,
            },
            mem_total: 0,
            mem_used: 0,
            mem_available: 0,
            mem_cached: 0,
            swap_total: 0,
            swap_used: 0,
            net_rx: 0,
            net_tx: 0,
            gpu_info: None,
            processes: Vec::new(),
            disk_info: Vec::new(),
            network_info: Vec::new(),
            psi_data: None,
            connections_data: None,
            treemap_data: None,
            sensor_health_data: None,
            disk_io_data: None,
            disk_entropy_data: None,
            file_analyzer_data: None,
        }
    }
}

/// Background metrics collector that owns all heavy I/O objects.
/// Runs in a background thread and produces MetricsSnapshots.
pub struct MetricsCollector {
    system: System,
    disks: Disks,
    networks: Networks,
    analyzers: AnalyzerRegistry,
    deterministic: bool,
    frame_id: u64,
}

impl MetricsCollector {
    /// Create a new collector with initialized system objects.
    pub fn new(deterministic: bool) -> Self {
        let mut system = System::new();

        // Initial CPU sample (need 2 for delta calculation)
        system.refresh_cpu_specifics(CpuRefreshKind::everything());

        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        let analyzers = AnalyzerRegistry::new();

        Self {
            system,
            disks,
            networks,
            analyzers,
            deterministic,
            frame_id: 0,
        }
    }

    /// Check if PSI is available
    pub fn has_psi(&self) -> bool {
        self.analyzers.psi.is_some()
    }

    /// Check if GPU monitoring is available
    pub fn has_gpu(&self) -> bool {
        self.analyzers.gpu_procs.is_some()
    }

    /// Check if sensor monitoring is available
    pub fn has_sensors(&self) -> bool {
        self.analyzers.sensor_health.is_some()
    }

    /// Check if connection monitoring is available
    pub fn has_connections(&self) -> bool {
        self.analyzers.connections.is_some()
    }

    /// Check if treemap is available
    pub fn has_treemap(&self) -> bool {
        self.analyzers.treemap.is_some()
    }
}

impl AsyncCollector for MetricsCollector {
    type Snapshot = MetricsSnapshot;

    fn collect(&mut self) -> MetricsSnapshot {
        self.frame_id += 1;

        if self.deterministic {
            return MetricsSnapshot::empty();
        }

        // CPU refresh
        self.system
            .refresh_cpu_specifics(CpuRefreshKind::everything());

        let cpu_total: f32 = self
            .system
            .cpus()
            .iter()
            .map(sysinfo::Cpu::cpu_usage)
            .sum::<f32>()
            / self.system.cpus().len().max(1) as f32;

        let per_core_percent: Vec<f64> = self
            .system
            .cpus()
            .iter()
            .map(|c| c.cpu_usage() as f64)
            .collect();

        // SPEC-024: Extract per-core frequency for async updates
        let per_core_freq: Vec<u64> = self
            .system
            .cpus()
            .iter()
            .map(sysinfo::Cpu::frequency)
            .collect();

        // SPEC-024: Extract per-core temperature (from hwmon if available)
        let per_core_temp: Vec<f32> = read_core_temperatures(self.system.cpus().len());

        let load_avg = System::load_average();

        // Memory refresh
        self.system
            .refresh_memory_specifics(MemoryRefreshKind::everything());

        let mem_total = self.system.total_memory();
        let mem_used = self.system.used_memory();
        let mem_available = self.system.available_memory();
        let mem_cached = read_cached_memory();
        let swap_total = self.system.total_swap();
        let swap_used = self.system.used_swap();

        // Process refresh - incremental for performance
        let process_count = self.system.processes().len();
        let needs_initial = process_count == 0;
        let needs_periodic = self.frame_id > 0 && self.frame_id % 60 == 0;

        if needs_initial || needs_periodic {
            self.system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing()
                    .with_cpu()
                    .with_memory()
                    .with_user(sysinfo::UpdateKind::OnlyIfNotSet),
            );
        } else if self.frame_id > 0 {
            let top_pids: Vec<_> = self
                .system
                .processes()
                .iter()
                .filter(|(_, p)| p.cpu_usage() > 0.1)
                .take(50)
                .map(|(pid, _)| *pid)
                .collect();

            if !top_pids.is_empty() {
                self.system.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&top_pids),
                    true,
                    ProcessRefreshKind::nothing()
                        .with_cpu()
                        .with_memory()
                        .with_user(sysinfo::UpdateKind::OnlyIfNotSet),
                );
            }
        }

        // Extract process info
        let processes: Vec<ProcessInfo> = self
            .system
            .processes()
            .iter()
            .map(|(pid, proc)| ProcessInfo {
                pid: pid.as_u32(),
                name: proc.name().to_string_lossy().to_string(),
                cpu_usage: proc.cpu_usage(),
                memory: proc.memory(),
                user: proc.user_id().map(|u| u.to_string()).unwrap_or_default(),
                cmd: proc
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" "),
            })
            .collect();

        // Disk refresh
        self.disks.refresh(true);
        let disk_info: Vec<DiskInfo> = self
            .disks
            .iter()
            .map(|d| DiskInfo {
                name: d.name().to_string_lossy().to_string(),
                mount_point: d.mount_point().to_string_lossy().to_string(),
                total_space: d.total_space(),
                available_space: d.available_space(),
                file_system: d.file_system().to_string_lossy().to_string(),
            })
            .collect();

        // Network refresh
        self.networks.refresh(true);
        let (net_rx, net_tx) = self
            .networks
            .iter()
            .fold((0u64, 0u64), |acc, (_name, data)| {
                (acc.0 + data.received(), acc.1 + data.transmitted())
            });

        let network_info: Vec<NetworkInfo> = self
            .networks
            .iter()
            .map(|(name, data)| NetworkInfo {
                name: name.clone(),
                received: data.received(),
                transmitted: data.transmitted(),
            })
            .collect();

        // GPU info (may call nvidia-smi)
        let gpu_info = read_gpu_info();

        // Analyzer data
        self.analyzers.collect_all();
        let psi_data = self.analyzers.psi.as_ref().map(|p| p.data().clone());
        let connections_data = self
            .analyzers
            .connections
            .as_ref()
            .map(|c| c.data().clone());
        let treemap_data = self.analyzers.treemap.as_ref().map(|t| t.data().clone());
        let sensor_health_data = self
            .analyzers
            .sensor_health
            .as_ref()
            .map(|s| s.data().clone());
        let disk_io_data = self.analyzers.disk_io.as_ref().map(|d| d.data().clone());
        let disk_entropy_data = self
            .analyzers
            .disk_entropy
            .as_ref()
            .map(|d| d.data().clone());
        let file_analyzer_data = self
            .analyzers
            .file_analyzer
            .as_ref()
            .map(|f| f.data().clone());

        MetricsSnapshot {
            cpu_avg: cpu_total as f64 / 100.0,
            per_core_percent,
            per_core_freq,
            per_core_temp,
            load_avg,
            mem_total,
            mem_used,
            mem_available,
            mem_cached,
            swap_total,
            swap_used,
            net_rx,
            net_tx,
            gpu_info,
            processes,
            disk_info,
            network_info,
            psi_data,
            connections_data,
            treemap_data,
            sensor_health_data,
            disk_io_data,
            disk_entropy_data,
            file_analyzer_data,
        }
    }
}

/// Ring buffer for history (matches ttop's `ring_buffer.rs`)
pub struct RingBuffer<T> {
    data: Vec<T>,
    capacity: usize,
}

impl<T: Clone> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: T) {
        if self.data.len() >= self.capacity {
            self.data.remove(0);
        }
        self.data.push(value);
    }

    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn last(&self) -> Option<&T> {
        self.data.last()
    }
}

/// Process sort column (matches ttop's state.rs)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSortColumn {
    Pid,
    User,
    Cpu,
    Mem,
    Command,
}

impl ProcessSortColumn {
    /// Number of columns for navigation bounds
    pub const COUNT: usize = 5;

    pub fn next(self) -> Self {
        match self {
            Self::Pid => Self::User,
            Self::User => Self::Cpu,
            Self::Cpu => Self::Mem,
            Self::Mem => Self::Command,
            Self::Command => Self::Pid,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Pid => Self::Command,
            Self::User => Self::Pid,
            Self::Cpu => Self::User,
            Self::Mem => Self::Cpu,
            Self::Command => Self::Mem,
        }
    }

    /// Convert column index to enum variant
    pub fn from_index(idx: usize) -> Self {
        match idx % Self::COUNT {
            0 => Self::Pid,
            1 => Self::User,
            2 => Self::Cpu,
            3 => Self::Mem,
            _ => Self::Command,
        }
    }

    /// Convert enum variant to column index
    pub fn to_index(self) -> usize {
        match self {
            Self::Pid => 0,
            Self::User => 1,
            Self::Cpu => 2,
            Self::Mem => 3,
            Self::Command => 4,
        }
    }

    /// Get column header with optional sort indicator
    pub fn header(self, is_sorted: bool, descending: bool) -> String {
        let base = match self {
            Self::Pid => "PID",
            Self::User => "USER",
            Self::Cpu => "CPU%",
            Self::Mem => "MEM%",
            Self::Command => "COMMAND",
        };
        if is_sorted {
            format!("{}{}", base, if descending { "▼" } else { "▲" })
        } else {
            base.to_string()
        }
    }
}

/// Panel visibility (matches ttop's app.rs - all 14 panels)
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct PanelVisibility {
    // Core panels (P0)
    pub cpu: bool,
    pub memory: bool,
    pub disk: bool,
    pub network: bool,
    pub process: bool,
    // Hardware panels (P1)
    pub gpu: bool,
    pub sensors: bool,
    pub psi: bool,
    pub connections: bool,
    // Optional panels (P2)
    pub battery: bool,
    pub sensors_compact: bool,
    pub system: bool,
    // Advanced panels (P3)
    pub treemap: bool,
    pub files: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            // Core panels - always visible by default
            cpu: true,
            memory: true,
            disk: true,
            network: true,
            process: true,
            // Hardware panels - visible if hardware available
            gpu: false,     // TODO: detect GPU
            sensors: false, // TODO: detect sensors
            psi: false,     // TODO: detect PSI support
            connections: false,
            // Optional panels - hidden by default
            battery: false, // TODO: detect battery
            sensors_compact: false,
            system: false,
            // Advanced panels - hidden by default
            treemap: false,
            files: false,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    // System collectors
    pub system: System,
    pub disks: Disks,
    pub networks: Networks,
    /// User lookup table for resolving UID to username
    pub users: Users,

    // Analyzers (detailed metrics from /proc, /sys)
    pub analyzers: AnalyzerRegistry,

    // History buffers (normalized 0-1)
    pub cpu_history: RingBuffer<f64>,
    pub mem_history: RingBuffer<f64>,
    pub net_rx_history: RingBuffer<f64>,
    pub net_tx_history: RingBuffer<f64>,
    /// Per-interface network history (bytes/s per interface)
    pub net_iface_history: std::collections::HashMap<String, (RingBuffer<f64>, RingBuffer<f64>)>,
    /// Selected network interface index for Tab cycling (PMAT-GAP-031 - ttop parity)
    pub selected_interface_index: usize,
    /// GPU utilization history (0-100%)
    pub gpu_history: RingBuffer<f64>,
    /// VRAM usage history (0-100%)
    pub vram_history: RingBuffer<f64>,
    /// Cached GPU info (updated during `update()`)
    pub gpu_info: Option<GpuInfo>,

    // Per-core CPU data
    pub per_core_percent: Vec<f64>,
    /// Per-core frequency in MHz (SPEC-024 async update requirement)
    pub per_core_freq: Vec<u64>,
    /// Per-core temperature in °C (SPEC-024 async update requirement)
    pub per_core_temp: Vec<f32>,

    // Memory values
    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_available: u64,
    pub mem_cached: u64,
    pub swap_total: u64,
    pub swap_used: u64,

    // UI state
    pub panels: PanelVisibility,
    pub process_selected: usize,
    pub process_scroll_offset: usize,
    pub sort_column: ProcessSortColumn,
    pub sort_descending: bool,
    pub filter: String,
    pub show_filter_input: bool,
    pub show_help: bool,
    pub running: bool,

    // Signal handling (SPEC-024 Appendix G.6 P0 - ttop parity)
    /// Pending signal confirmation: (pid, process_name, signal_type)
    pub pending_signal: Option<(u32, String, SignalType)>,
    /// Last signal result: (success, message, timestamp) - auto-clears after 3s (PMAT-GAP-033)
    pub signal_result: Option<(bool, String, std::time::Instant)>,

    // Panel navigation and explode (SPEC-024 v5.0 Features D, E)
    /// Currently focused panel (receives keyboard input)
    pub focused_panel: Option<PanelType>,
    /// Exploded (fullscreen) panel, if any
    pub exploded_panel: Option<PanelType>,
    /// Selected column index for DataFrame navigation (0-based, left-to-right)
    pub selected_column: usize,
    /// Files panel view mode (PMAT-GAP-034 - ttop parity)
    pub files_view_mode: FilesViewMode,
    /// Last focused panel before collapse (PMAT-GAP-035 - ttop parity)
    /// Used to restore focus when panel is shown again
    pub collapse_memory: Option<PanelType>,

    // Configuration (SPEC-024 v5.0 Feature A)
    pub config: PtopConfig,

    // Frame timing
    pub frame_id: u64,
    pub avg_frame_time_us: u64,
    pub show_fps: bool,

    // Deterministic mode for pixel-perfect testing
    pub deterministic: bool,
    /// Fixed uptime in seconds (used in deterministic mode)
    pub fixed_uptime: u64,

    // Cached system info (O(1) render - no I/O in render path)
    /// Cached load average (updated in collect_metrics)
    pub load_avg: sysinfo::LoadAvg,
    /// Cached hostname (read once at startup)
    pub hostname: String,
    /// Cached kernel version (read once at startup)
    pub kernel_version: String,
    /// Cached container detection (read once at startup)
    pub in_container: bool,

    // Display rules (SPEC-024 Appendix F)
    /// System capabilities (detected at startup)
    pub system_capabilities: crate::widgets::SystemCapabilities,

    // Snapshot data from background collector (CB-INPUT-006)
    /// Process list from last snapshot
    pub snapshot_processes: Vec<ProcessInfo>,
    /// Disk info from last snapshot
    pub snapshot_disks: Vec<DiskInfo>,
    /// Network info from last snapshot
    pub snapshot_networks: Vec<NetworkInfo>,
    /// PSI data from last snapshot
    pub snapshot_psi: Option<PsiData>,
    /// Connections data from last snapshot
    pub snapshot_connections: Option<ConnectionsData>,
    /// Treemap data from last snapshot
    pub snapshot_treemap: Option<TreemapData>,
    /// Sensor health data from last snapshot
    pub snapshot_sensor_health: Option<SensorHealthData>,
    /// Disk I/O data from last snapshot
    pub snapshot_disk_io: Option<DiskIoData>,
    /// Disk entropy data from last snapshot
    pub snapshot_disk_entropy: Option<DiskEntropyData>,
    /// File analyzer data from last snapshot
    pub snapshot_file_analyzer: Option<FileAnalyzerData>,
}

impl App {
    /// Create new App with collectors initialized
    ///
    /// # Arguments
    /// * `deterministic` - If true, uses fixed mock data for pixel-perfect testing
    /// Create a new App with default configuration
    pub fn new(deterministic: bool) -> Self {
        Self::with_config(deterministic, PtopConfig::load())
    }

    /// Create a new App with a custom configuration
    pub fn with_config(deterministic: bool, config: PtopConfig) -> Self {
        Self::with_config_options(deterministic, config, false)
    }

    /// Create a new App with lightweight initialization (faster startup for headless mode)
    pub fn with_config_lightweight(deterministic: bool, config: PtopConfig) -> Self {
        Self::with_config_options(deterministic, config, true)
    }

    /// Internal constructor with options
    fn with_config_options(deterministic: bool, config: PtopConfig, lightweight: bool) -> Self {
        let mut system = System::new();

        // Initial refresh (need 2 samples for CPU delta)
        // Use 50ms instead of 100ms for faster startup while still getting valid CPU readings
        system.refresh_cpu_specifics(CpuRefreshKind::everything());
        std::thread::sleep(Duration::from_millis(50));
        system.refresh_cpu_specifics(CpuRefreshKind::everything());
        system.refresh_memory_specifics(MemoryRefreshKind::everything());

        // Skip heavy process refresh in lightweight mode (for headless/render-once)
        // Process CPU% also needs 2 samples for delta calculation (same as core CPU%)
        if !lightweight {
            system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::everything()
                    .with_cpu()
                    .with_memory()
                    .with_user(sysinfo::UpdateKind::OnlyIfNotSet),
            );
            // Second refresh after delay to get valid CPU percentages
            std::thread::sleep(Duration::from_millis(50));
            system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing().with_cpu(),
            );
        }

        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        // ttop deterministic mode uses 48 cores with all zeros
        let core_count = if deterministic {
            48
        } else {
            system.cpus().len()
        };

        // Initialize analyzers and detect available features
        let analyzers = AnalyzerRegistry::new();

        // Auto-detect panel visibility based on available analyzers
        // SPEC-024: Match ttop layout - prefer Sensors over PSI on desktop
        let mut panels = PanelVisibility::default();

        // Only show sensors panel if there are actual sensor readings
        // Sensors takes priority over PSI in ttop layout
        let has_sensors = analyzers
            .sensor_health
            .as_ref()
            .is_some_and(|sh| !sh.data().sensors.is_empty());
        if has_sensors {
            panels.sensors = true;
            // Don't show PSI if sensors available (ttop style)
        } else if analyzers.psi.is_some() {
            // Fallback: show PSI only if no sensors
            panels.psi = true;
        }

        if analyzers.gpu_procs.is_some() {
            panels.gpu = true;
        }
        if analyzers.connections.is_some() {
            panels.connections = true;
        }
        if analyzers.treemap.is_some() {
            panels.files = true;
        }

        // In deterministic mode, match ttop's panel layout exactly
        if deterministic {
            panels = PanelVisibility {
                cpu: true,
                memory: true,
                disk: true,
                network: true,
                process: true,
                gpu: true,
                sensors: true,
                psi: false, // ttop shows Containers, not PSI
                connections: true,
                battery: false,
                sensors_compact: false,
                system: false,
                treemap: false,
                files: true,
            };
        }

        let users = Users::new_with_refreshed_list();

        let mut app = Self {
            system,
            disks,
            networks,
            users,
            analyzers,
            cpu_history: RingBuffer::new(60),
            mem_history: RingBuffer::new(60),
            net_rx_history: RingBuffer::new(60),
            net_tx_history: RingBuffer::new(60),
            net_iface_history: std::collections::HashMap::new(),
            selected_interface_index: 0, // PMAT-GAP-031: default to first interface
            gpu_history: RingBuffer::new(60),
            vram_history: RingBuffer::new(60),
            gpu_info: None,
            per_core_percent: vec![0.0; core_count],
            per_core_freq: vec![0; core_count], // SPEC-024 async update
            // Initialize temperatures immediately for non-deterministic mode
            per_core_temp: if deterministic {
                vec![0.0; core_count]
            } else {
                read_core_temperatures(core_count)
            },
            mem_total: 0,
            mem_used: 0,
            mem_available: 0,
            mem_cached: 0,
            swap_total: 0,
            swap_used: 0,
            panels,
            process_selected: 0,
            process_scroll_offset: 0,
            sort_column: ProcessSortColumn::Cpu,
            sort_descending: true,
            filter: String::new(),
            show_filter_input: false,
            show_help: false,
            running: true,
            // Signal handling (SPEC-024 Appendix G.6 P0)
            pending_signal: None,
            signal_result: None,
            // Panel navigation (SPEC-024 v5.0 Feature D)
            focused_panel: Some(PanelType::Cpu), // Start with CPU focused
            exploded_panel: None,
            selected_column: 0, // Start with first column (PID)
            files_view_mode: FilesViewMode::default(), // PMAT-GAP-034: size view default
            collapse_memory: None, // PMAT-GAP-035: no collapsed focused panel
            config,
            frame_id: 0,
            avg_frame_time_us: 0,
            show_fps: false,
            deterministic,
            // ttop deterministic mode: 0 uptime
            fixed_uptime: 0,
            // Cached system info (read once, O(1) render)
            // Initialize load_avg immediately for non-deterministic mode
            load_avg: if deterministic {
                sysinfo::LoadAvg {
                    one: 0.0,
                    five: 0.0,
                    fifteen: 0.0,
                }
            } else {
                System::load_average()
            },
            hostname: read_hostname(),
            kernel_version: read_kernel_version(),
            in_container: detect_container(),
            // Display rules (SPEC-024 Appendix F)
            system_capabilities: crate::widgets::SystemCapabilities::detect(),
            // Snapshot data (CB-INPUT-006)
            snapshot_processes: Vec::new(),
            snapshot_disks: Vec::new(),
            snapshot_networks: Vec::new(),
            snapshot_psi: None,
            snapshot_connections: None,
            snapshot_treemap: None,
            snapshot_sensor_health: None,
            snapshot_disk_io: None,
            snapshot_disk_entropy: None,
            snapshot_file_analyzer: None,
        };

        // In deterministic mode, populate with fixed data
        if deterministic {
            app.init_deterministic_data();
        }

        app
    }

    /// Initialize fixed data for deterministic mode
    /// Matches ttop's deterministic mode exactly: all zeros
    fn init_deterministic_data(&mut self) {
        // ttop deterministic: 48 cores all at 0%
        self.per_core_percent = vec![0.0; 48];

        // ttop deterministic: all memory values are 0
        self.mem_total = 0;
        self.mem_used = 0;
        self.mem_available = 0;
        self.mem_cached = 0;
        self.swap_total = 0;
        self.swap_used = 0;

        // ttop deterministic: fixed uptime (5 days, 3 hours, 47 minutes)
        self.fixed_uptime = 5 * 86400 + 3 * 3600 + 47 * 60;

        // ttop deterministic: empty history (all zeros)
        for _ in 0..60 {
            self.cpu_history.push(0.0);
            self.mem_history.push(0.0);
            self.net_rx_history.push(0.0);
            self.net_tx_history.push(0.0);
        }
    }

    /// Get PSI data if available
    pub fn psi_data(&self) -> Option<&PsiData> {
        // Use snapshot data for CB-INPUT-006 async pattern consistency
        self.snapshot_psi.as_ref()
    }

    /// Collect metrics from all sources
    pub fn collect_metrics(&mut self) {
        self.frame_id += 1;
        // Provability: frame_id is monotonically increasing
        debug_assert!(self.frame_id > 0, "frame_id must be positive after increment");

        // Check for config hot reload (SPEC-024 v5.2.0 Feature A)
        // Only check every 10 frames to reduce filesystem overhead
        if self.frame_id % 10 == 0 {
            if let Some(new_config) = self.config.check_reload() {
                eprintln!("[ptop] config reloaded");
                self.config = new_config;
            }
        }

        // In deterministic mode, skip real data collection
        if self.deterministic {
            return;
        }

        // Update cached load average (O(1) render - no I/O in render path)
        self.load_avg = System::load_average();

        // CPU
        self.system
            .refresh_cpu_specifics(CpuRefreshKind::everything());

        let cpu_total: f32 = self
            .system
            .cpus()
            .iter()
            .map(sysinfo::Cpu::cpu_usage)
            .sum::<f32>()
            / self.system.cpus().len().max(1) as f32;
        self.cpu_history.push(cpu_total as f64 / 100.0);

        // Per-core
        self.per_core_percent = self
            .system
            .cpus()
            .iter()
            .map(|c| c.cpu_usage() as f64)
            .collect();
        // Provability: per_core_percent length matches CPU count
        debug_assert_eq!(
            self.per_core_percent.len(),
            self.system.cpus().len(),
            "per_core_percent must have one entry per CPU"
        );

        // SPEC-024: Per-core frequency and temperature for render-once mode
        // (async mode uses MetricsSnapshot, but sync mode needs direct update)
        self.per_core_freq = self
            .system
            .cpus()
            .iter()
            .map(sysinfo::Cpu::frequency)
            .collect();
        self.per_core_temp = read_core_temperatures(self.system.cpus().len());

        // Memory
        self.system
            .refresh_memory_specifics(MemoryRefreshKind::everything());

        self.mem_total = self.system.total_memory();
        self.mem_used = self.system.used_memory();
        self.mem_available = self.system.available_memory();
        // Read cached memory directly from /proc/meminfo (sysinfo doesn't expose this)
        self.mem_cached = read_cached_memory();
        self.swap_total = self.system.total_swap();
        self.swap_used = self.system.used_swap();

        if self.mem_total > 0 {
            self.mem_history
                .push(self.mem_used as f64 / self.mem_total as f64);
        }

        // Processes - O(1) OPTIMIZATION: Skip refresh if we already have process data
        // Full scan only on frame 0 or every 60 frames (~60 seconds at 1fps)
        // We already get initial process data in with_config(), so frame 0 can skip too
        // IMPORTANT: First few frames need full refresh to calculate CPU delta (sysinfo requirement)
        let process_count = self.system.processes().len();
        let needs_initial_data = process_count == 0;
        let needs_periodic_refresh = self.frame_id > 0 && self.frame_id % 60 == 0;
        let needs_delta_calculation = self.frame_id <= 3; // First 3 frames need full refresh for CPU %

        if needs_initial_data || needs_periodic_refresh || needs_delta_calculation {
            // Full refresh to discover new processes
            self.system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing()
                    .with_cpu()
                    .with_memory()
                    .with_user(sysinfo::UpdateKind::OnlyIfNotSet),
            );
        } else if self.frame_id > 0 {
            // Incremental: only refresh top 50 processes by CPU (O(1) cached view)
            // Pre-collect PIDs to avoid iterator invalidation
            let top_pids: Vec<_> = self
                .system
                .processes()
                .iter()
                .filter(|(_, p)| p.cpu_usage() > 0.1)
                .take(50)
                .map(|(pid, _)| *pid)
                .collect();

            if !top_pids.is_empty() {
                self.system.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&top_pids),
                    true,
                    ProcessRefreshKind::nothing()
                        .with_cpu()
                        .with_memory()
                        .with_user(sysinfo::UpdateKind::OnlyIfNotSet),
                );
            }
        }
        // frame_id == 0: use cached data from with_config() initialization

        // Disk
        self.disks.refresh(true);

        // Network
        self.networks.refresh(true);

        let (rx, tx) = self
            .networks
            .iter()
            .fold((0u64, 0u64), |acc, (_name, data)| {
                (acc.0 + data.received(), acc.1 + data.transmitted())
            });
        // Normalize (assume max 1GB/s)
        self.net_rx_history
            .push((rx as f64 / 1_000_000_000.0).min(1.0));
        self.net_tx_history
            .push((tx as f64 / 1_000_000_000.0).min(1.0));

        // GPU (SPEC-024 D012: track real GPU history)
        // Skip in deterministic mode (nvidia-smi is non-deterministic)
        if !self.deterministic {
            self.gpu_info = read_gpu_info();
            if let Some(ref gpu) = self.gpu_info {
                // Push utilization (0-100%)
                self.gpu_history.push(gpu.utilization.unwrap_or(0) as f64);
                // Push VRAM percentage (0-100%)
                let vram_pct = match (gpu.vram_used, gpu.vram_total) {
                    (Some(used), Some(total)) if total > 0 => (used as f64 / total as f64) * 100.0,
                    _ => 0.0,
                };
                self.vram_history.push(vram_pct);
            }
        }

        // Collect analyzer data (PSI, etc.)
        self.analyzers.collect_all();

        // Copy analyzer data to snapshot fields for render access (sync mode parity with async mode)
        // This ensures render code can use the same snapshot_* fields in both modes.
        self.snapshot_psi = self.analyzers.psi.as_ref().map(|p| p.data().clone());
        self.snapshot_connections = self.analyzers.connections.as_ref().map(|c| c.data().clone());
        self.snapshot_treemap = self.analyzers.treemap.as_ref().map(|t| t.data().clone());
        self.snapshot_sensor_health = self
            .analyzers
            .sensor_health
            .as_ref()
            .map(|s| s.data().clone());
        self.snapshot_disk_io = self.analyzers.disk_io.as_ref().map(|d| d.data().clone());
        self.snapshot_disk_entropy = self
            .analyzers
            .disk_entropy
            .as_ref()
            .map(|d| d.data().clone());
        self.snapshot_file_analyzer = self
            .analyzers
            .file_analyzer
            .as_ref()
            .map(|f| f.data().clone());
    }

    /// Update frame timing stats
    pub fn update_frame_stats(&mut self, frame_times: &[Duration]) {
        if frame_times.is_empty() {
            return;
        }
        let total: u128 = frame_times.iter().map(std::time::Duration::as_micros).sum();
        self.avg_frame_time_us = (total / frame_times.len() as u128) as u64;
    }

    /// Apply a metrics snapshot from the background collector.
    /// This is O(1) - just copies/swaps data, no I/O.
    pub fn apply_snapshot(&mut self, snapshot: MetricsSnapshot) {
        self.frame_id += 1;

        // Update CPU data (SPEC-024 async update requirement)
        self.per_core_percent = snapshot.per_core_percent;
        self.per_core_freq = snapshot.per_core_freq;
        self.per_core_temp = snapshot.per_core_temp;
        self.load_avg = snapshot.load_avg;
        self.cpu_history.push(snapshot.cpu_avg);

        // Update memory data
        self.mem_total = snapshot.mem_total;
        self.mem_used = snapshot.mem_used;
        self.mem_available = snapshot.mem_available;
        self.mem_cached = snapshot.mem_cached;
        self.swap_total = snapshot.swap_total;
        self.swap_used = snapshot.swap_used;
        if self.mem_total > 0 {
            self.mem_history
                .push(self.mem_used as f64 / self.mem_total as f64);
        }

        // Update network history
        // Normalize (assume max 1GB/s)
        self.net_rx_history
            .push((snapshot.net_rx as f64 / 1_000_000_000.0).min(1.0));
        self.net_tx_history
            .push((snapshot.net_tx as f64 / 1_000_000_000.0).min(1.0));

        // Update GPU data
        self.gpu_info = snapshot.gpu_info.clone();
        if let Some(ref gpu) = snapshot.gpu_info {
            self.gpu_history.push(gpu.utilization.unwrap_or(0) as f64);
            let vram_pct = match (gpu.vram_used, gpu.vram_total) {
                (Some(used), Some(total)) if total > 0 => (used as f64 / total as f64) * 100.0,
                _ => 0.0,
            };
            self.vram_history.push(vram_pct);
        }

        // Store snapshot data for rendering (processes, disks, networks)
        self.snapshot_processes = snapshot.processes;
        self.snapshot_disks = snapshot.disk_info;

        // Update per-interface history before storing snapshot
        for net in &snapshot.network_info {
            let entry = self
                .net_iface_history
                .entry(net.name.clone())
                .or_insert_with(|| (RingBuffer::new(60), RingBuffer::new(60)));
            entry.0.push(net.received as f64);
            entry.1.push(net.transmitted as f64);
        }
        self.snapshot_networks = snapshot.network_info;
        self.snapshot_psi = snapshot.psi_data;
        self.snapshot_connections = snapshot.connections_data;
        self.snapshot_treemap = snapshot.treemap_data;
        self.snapshot_sensor_health = snapshot.sensor_health_data;
        self.snapshot_disk_io = snapshot.disk_io_data;
        self.snapshot_disk_entropy = snapshot.disk_entropy_data;
        self.snapshot_file_analyzer = snapshot.file_analyzer_data;
    }

    /// Build data availability context for display rules evaluation
    ///
    /// SPEC-024 Appendix F: Declarative display rules require knowing
    /// what data is available to determine panel visibility.
    pub fn data_availability(&self) -> crate::widgets::DataAvailability {
        crate::widgets::DataAvailability {
            psi_available: self.snapshot_psi.as_ref().is_some_and(|psi| {
                psi.available
                    && (psi.cpu.some.avg10 > 0.01
                        || psi.io.some.avg10 > 0.01
                        || psi.memory.some.avg10 > 0.01)
            }),
            sensors_available: self.snapshot_sensor_health.is_some(),
            sensor_count: self
                .snapshot_sensor_health
                .as_ref()
                .map_or(0, |s| s.sensors.len()),
            gpu_available: self.gpu_info.is_some(),
            battery_available: false, // TODO: Add battery snapshot
            treemap_ready: self
                .snapshot_treemap
                .as_ref()
                .is_some_and(|t| !t.top_items.is_empty()),
            connections_available: self.snapshot_connections.is_some(),
            connection_count: self
                .snapshot_connections
                .as_ref()
                .map_or(0, |c| c.connections.len()),
        }
    }

    /// Evaluate display rules for a panel
    ///
    /// SPEC-024 Appendix F: Returns DisplayAction based on system capabilities
    /// and current data availability.
    pub fn evaluate_panel_display(&self, panel: PanelType) -> crate::widgets::DisplayAction {
        use crate::widgets::{
            BatteryDisplayRules, DisplayContext, DisplayRules, DisplayTerminalSize,
            GpuDisplayRules, PsiDisplayRules, SensorsDisplayRules,
        };

        let ctx = DisplayContext {
            system: &self.system_capabilities,
            terminal: DisplayTerminalSize {
                width: 0,
                height: 0,
            }, // Terminal size set by UI
            data: self.data_availability(),
        };

        match panel {
            PanelType::Psi => PsiDisplayRules.evaluate(&ctx),
            PanelType::Sensors => SensorsDisplayRules.evaluate(&ctx),
            PanelType::Gpu => GpuDisplayRules.evaluate(&ctx),
            PanelType::Battery => BatteryDisplayRules.evaluate(&ctx),
            _ => crate::widgets::DisplayAction::Show,
        }
    }

    /// Handle keyboard input. Returns true if app should quit.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        // Dispatch to mode-specific handlers
        if self.show_help {
            return self.handle_help_mode_key(code, modifiers);
        }
        if self.pending_signal.is_some() {
            return self.handle_signal_confirmation_key(code, modifiers);
        }
        if self.exploded_panel.is_some() {
            return self.handle_exploded_mode_key(code, modifiers);
        }
        if self.show_filter_input {
            return self.handle_filter_input_key(code);
        }
        self.handle_normal_mode_key(code, modifiers)
    }

    /// Handle keys in help overlay mode. Returns true if app should quit.
    fn handle_help_mode_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Esc | KeyCode::Char('?' | 'h') | KeyCode::F(1) => {
                self.show_help = false;
            }
            KeyCode::Char('q') => return true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,
            _ => {} // Swallow all other inputs
        }
        false
    }

    /// Handle keys in signal confirmation mode. Returns true if app should quit.
    fn handle_signal_confirmation_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Char('Y') | KeyCode::Enter => self.confirm_signal(),
            KeyCode::Char('n' | 'N') | KeyCode::Esc => self.cancel_signal(),
            KeyCode::Char('q') => return true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,
            KeyCode::Char('x') => self.request_signal(SignalType::Term),
            KeyCode::Char('K') => self.request_signal(SignalType::Kill),
            KeyCode::Char('H') => self.request_signal(SignalType::Hup),
            KeyCode::Char('i') => self.request_signal(SignalType::Int),
            KeyCode::Char('p') => self.request_signal(SignalType::Stop),
            _ => {} // Swallow other inputs
        }
        false
    }

    /// Handle keys in exploded mode. Returns true if app should quit.
    fn handle_exploded_mode_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Esc | KeyCode::Char('z') => self.exploded_panel = None,
            KeyCode::Char('q') => return true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,
            KeyCode::Left | KeyCode::Char('h') => self.navigate_column_left(),
            KeyCode::Right | KeyCode::Char('l') => self.navigate_column_right(),
            KeyCode::Enter | KeyCode::Char(' ') => self.sort_by_selected_column(),
            KeyCode::Up | KeyCode::Char('k') => self.navigate_process(-1),
            KeyCode::Down | KeyCode::Char('j') => self.navigate_process(1),
            KeyCode::PageUp => self.navigate_process(-10),
            KeyCode::PageDown => self.navigate_process(10),
            KeyCode::Home | KeyCode::Char('g') => self.process_selected = 0,
            KeyCode::End | KeyCode::Char('G') => self.select_last_process(),
            KeyCode::Char('c') => self.quick_sort(ProcessSortColumn::Cpu, true),
            KeyCode::Char('m') => self.quick_sort(ProcessSortColumn::Mem, true),
            KeyCode::Char('p') => self.quick_sort(ProcessSortColumn::Pid, false),
            KeyCode::Char('n') => self.quick_sort(ProcessSortColumn::Command, false),
            KeyCode::Char('r') => self.sort_descending = !self.sort_descending,
            KeyCode::Char('/' | 'f') => self.show_filter_input = true,
            _ => {} // Swallow other inputs
        }
        false
    }

    /// Handle keys in filter input mode. Returns true if app should quit.
    fn handle_filter_input_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc => {
                self.show_filter_input = false;
                self.filter.clear();
            }
            KeyCode::Enter => self.show_filter_input = false,
            KeyCode::Backspace => { self.filter.pop(); }
            KeyCode::Char(c) => self.filter.push(c),
            _ => {}
        }
        false
    }

    /// Handle keys in normal mode. Returns true if app should quit.
    #[allow(clippy::match_same_arms)]
    fn handle_normal_mode_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,
            KeyCode::Enter | KeyCode::Char('z') => {
                if let Some(panel) = self.focused_panel {
                    self.exploded_panel = Some(panel);
                }
            }
            KeyCode::Tab if !modifiers.contains(KeyModifiers::SHIFT) => {
                if self.focused_panel == Some(PanelType::Network) {
                    self.cycle_interface();
                } else {
                    self.navigate_panel_forward();
                }
            }
            KeyCode::BackTab => self.navigate_panel_backward(),
            KeyCode::Char('l') if !self.show_filter_input => self.navigate_panel_forward(),
            KeyCode::Char('H') => self.navigate_panel_backward(),
            KeyCode::Char('?' | 'h') | KeyCode::F(1) => {
                self.show_help = !self.show_help;
            }
            KeyCode::Char('1') => self.toggle_panel(PanelType::Cpu),
            KeyCode::Char('2') => self.toggle_panel(PanelType::Memory),
            KeyCode::Char('3') => self.toggle_panel(PanelType::Disk),
            KeyCode::Char('4') => self.toggle_panel(PanelType::Network),
            KeyCode::Char('5') => self.toggle_panel(PanelType::Process),
            KeyCode::Char('6') => self.toggle_panel(PanelType::Gpu),
            KeyCode::Char('7') => self.toggle_panel(PanelType::Sensors),
            KeyCode::Char('8') => self.toggle_panel(PanelType::Connections),
            KeyCode::Char('9') => self.toggle_panel(PanelType::Psi),
            KeyCode::Char('v') if self.focused_panel == Some(PanelType::Files) => {
                self.cycle_files_view_mode();
            }
            KeyCode::Down | KeyCode::Char('j') => self.navigate_process(1),
            KeyCode::Up | KeyCode::Char('k') => self.navigate_process(-1),
            KeyCode::PageDown => self.navigate_process(10),
            KeyCode::PageUp => self.navigate_process(-10),
            KeyCode::Home | KeyCode::Char('g') => self.process_selected = 0,
            KeyCode::End | KeyCode::Char('G') => self.select_last_process(),
            KeyCode::Char('c') => {
                self.sort_column = ProcessSortColumn::Cpu;
                self.sort_descending = true;
            }
            KeyCode::Char('m') => {
                self.sort_column = ProcessSortColumn::Mem;
                self.sort_descending = true;
            }
            KeyCode::Char('p') => {
                self.sort_column = ProcessSortColumn::Pid;
                self.sort_descending = false;
            }
            KeyCode::Char('s') => self.sort_column = self.sort_column.next(),
            KeyCode::Char('r') => self.sort_descending = !self.sort_descending,
            KeyCode::Char('/' | 'f') => self.show_filter_input = true,
            KeyCode::Delete => self.filter.clear(),
            KeyCode::Char('0') => self.panels = PanelVisibility::default(),
            KeyCode::Char('x') => self.request_signal(SignalType::Term),
            KeyCode::Char('X') => self.request_signal(SignalType::Kill),
            _ => {}
        }
        false
    }

    /// Navigate column selection left with wrap-around.
    fn navigate_column_left(&mut self) {
        if self.selected_column > 0 {
            self.selected_column -= 1;
        } else {
            self.selected_column = ProcessSortColumn::COUNT - 1;
        }
    }

    /// Navigate column selection right with wrap-around.
    fn navigate_column_right(&mut self) {
        self.selected_column = (self.selected_column + 1) % ProcessSortColumn::COUNT;
    }

    /// Sort by currently selected column.
    fn sort_by_selected_column(&mut self) {
        let new_col = ProcessSortColumn::from_index(self.selected_column);
        if self.sort_column == new_col {
            self.sort_descending = !self.sort_descending;
        } else {
            self.sort_column = new_col;
            self.sort_descending = matches!(new_col, ProcessSortColumn::Cpu | ProcessSortColumn::Mem);
        }
    }

    /// Quick sort by column with specified direction.
    fn quick_sort(&mut self, column: ProcessSortColumn, descending: bool) {
        self.sort_column = column;
        self.selected_column = column.to_index();
        self.sort_descending = descending;
    }

    /// Select last process in list.
    fn select_last_process(&mut self) {
        let count = self.process_count();
        if count > 0 {
            self.process_selected = count - 1;
        }
    }

    /// Navigate to next visible panel (SPEC-024 v5.0 Feature D)
    fn navigate_panel_forward(&mut self) {
        let visible = self.visible_panels();
        if visible.is_empty() {
            return;
        }

        let current_idx = self
            .focused_panel
            .and_then(|p| visible.iter().position(|&v| v == p))
            .unwrap_or(0);

        let next_idx = (current_idx + 1) % visible.len();
        self.focused_panel = Some(visible[next_idx]);
    }

    /// Navigate to previous visible panel (SPEC-024 v5.0 Feature D)
    fn navigate_panel_backward(&mut self) {
        let visible = self.visible_panels();
        if visible.is_empty() {
            return;
        }

        let current_idx = self
            .focused_panel
            .and_then(|p| visible.iter().position(|&v| v == p))
            .unwrap_or(0);

        let prev_idx = if current_idx == 0 {
            visible.len() - 1
        } else {
            current_idx - 1
        };
        self.focused_panel = Some(visible[prev_idx]);
    }

    /// Get list of currently visible panels in order
    pub fn visible_panels(&self) -> Vec<PanelType> {
        let mut visible = Vec::new();

        if self.panels.cpu {
            visible.push(PanelType::Cpu);
        }
        if self.panels.memory {
            visible.push(PanelType::Memory);
        }
        if self.panels.disk {
            visible.push(PanelType::Disk);
        }
        if self.panels.network {
            visible.push(PanelType::Network);
        }
        if self.panels.process {
            visible.push(PanelType::Process);
        }
        if self.panels.gpu {
            visible.push(PanelType::Gpu);
        }
        if self.panels.sensors {
            visible.push(PanelType::Sensors);
        }
        if self.panels.connections {
            visible.push(PanelType::Connections);
        }
        if self.panels.psi {
            visible.push(PanelType::Psi);
        }
        if self.panels.files {
            visible.push(PanelType::Files);
        }

        visible
    }

    /// Check if a panel is currently focused
    pub fn is_panel_focused(&self, panel: PanelType) -> bool {
        self.focused_panel == Some(panel)
    }

    /// Get detail level for a panel based on its current height
    /// Reference: SPEC-024 Section 17.3
    pub fn detail_level_for_panel(&self, _panel: PanelType, height: u16) -> DetailLevel {
        DetailLevel::for_height(height)
    }

    #[allow(clippy::cast_possible_wrap)]
    fn navigate_process(&mut self, delta: isize) {
        let count = self.process_count();
        if count == 0 {
            return;
        }

        let current = self.process_selected as isize;
        let new = (current + delta).clamp(0, (count - 1) as isize) as usize;
        self.process_selected = new;
    }

    /// Get filtered process count
    pub fn process_count(&self) -> usize {
        // ttop deterministic mode: 0 processes
        if self.deterministic {
            return 0;
        }
        self.system
            .processes()
            .values()
            .filter(|p| {
                if self.filter.is_empty() {
                    true
                } else {
                    let name = p.name().to_string_lossy().to_lowercase();
                    name.contains(&self.filter.to_lowercase())
                }
            })
            .count()
    }

    // ========== Signal Handling (SPEC-024 Appendix G.6 P0) ==========

    /// Get the currently selected process (pid, name) if any
    fn get_selected_process(&self) -> Option<(u32, String)> {
        let procs = self.sorted_processes();
        if self.process_selected < procs.len() {
            let p = procs[self.process_selected];
            Some((p.pid().as_u32(), p.name().to_string_lossy().to_string()))
        } else {
            None
        }
    }

    /// Request to send a signal to the selected process
    /// Opens confirmation dialog
    pub fn request_signal(&mut self, signal: SignalType) {
        if let Some((pid, name)) = self.get_selected_process() {
            self.pending_signal = Some((pid, name, signal));
        }
    }

    /// Confirm and send the pending signal
    pub fn confirm_signal(&mut self) {
        if let Some((pid, _name, signal)) = self.pending_signal.take() {
            let (success, message) = self.send_signal(pid, signal);
            self.signal_result = Some((success, message, std::time::Instant::now()));
        }
    }

    /// Cancel pending signal
    pub fn cancel_signal(&mut self) {
        self.pending_signal = None;
    }

    /// Clear old signal result after 3 seconds (PMAT-GAP-033 - ttop parity)
    pub fn clear_old_signal_result(&mut self) {
        if let Some((_, _, timestamp)) = &self.signal_result {
            if timestamp.elapsed() > std::time::Duration::from_secs(3) {
                self.signal_result = None;
            }
        }
    }

    /// Cycle to the next network interface (PMAT-GAP-031 - ttop parity)
    ///
    /// Tab key cycles through available interfaces in order.
    /// Wraps around to 0 when reaching the end.
    pub fn cycle_interface(&mut self) {
        let iface_count = self.snapshot_networks.len();
        if iface_count == 0 {
            self.selected_interface_index = 0;
            return;
        }
        self.selected_interface_index = (self.selected_interface_index + 1) % iface_count;
    }

    /// Get the name of the currently selected interface (PMAT-GAP-031)
    #[must_use]
    pub fn selected_interface_name(&self) -> Option<&str> {
        self.snapshot_networks
            .get(self.selected_interface_index)
            .map(|info| info.name.as_str())
    }

    /// Get the data for the currently selected interface (PMAT-GAP-031)
    #[must_use]
    pub fn selected_interface_data(&self) -> Option<&NetworkInfo> {
        self.snapshot_networks.get(self.selected_interface_index)
    }

    /// Cycle to the next files view mode (PMAT-GAP-034 - ttop parity)
    ///
    /// 'v' key cycles: Size -> Tree -> Flat -> Size
    pub fn cycle_files_view_mode(&mut self) {
        self.files_view_mode = self.files_view_mode.next();
    }

    /// Toggle a panel's visibility with collapse memory (PMAT-GAP-035 - ttop parity)
    ///
    /// When hiding a focused panel:
    /// 1. Store it in collapse_memory
    /// 2. Move focus to first visible panel
    ///
    /// When showing a panel that was previously focused (in collapse_memory):
    /// 1. Restore focus to that panel
    /// 2. Clear collapse_memory
    pub fn toggle_panel(&mut self, panel: PanelType) {
        let is_visible = self.is_panel_visible(panel);

        if is_visible {
            // Hiding the panel
            if self.focused_panel == Some(panel) {
                // Store in collapse_memory
                self.collapse_memory = Some(panel);
                // Move focus to first visible (excluding this one)
                self.set_panel_visible(panel, false);
                let visible = self.visible_panels();
                self.focused_panel = visible.first().copied();
            } else {
                self.set_panel_visible(panel, false);
            }
        } else {
            // Showing the panel
            self.set_panel_visible(panel, true);
            // If this panel was stored in collapse_memory, restore focus
            if self.collapse_memory == Some(panel) {
                self.focused_panel = Some(panel);
                self.collapse_memory = None;
            }
        }
    }

    /// Check if a panel is currently visible
    fn is_panel_visible(&self, panel: PanelType) -> bool {
        match panel {
            PanelType::Cpu => self.panels.cpu,
            PanelType::Memory => self.panels.memory,
            PanelType::Disk => self.panels.disk,
            PanelType::Network => self.panels.network,
            PanelType::Process => self.panels.process,
            PanelType::Gpu => self.panels.gpu,
            PanelType::Sensors => self.panels.sensors,
            PanelType::Connections => self.panels.connections,
            PanelType::Psi => self.panels.psi,
            PanelType::Battery => self.panels.battery,
            PanelType::Files => self.panels.files,
            PanelType::Containers => false, // Not implemented
        }
    }

    /// Set a panel's visibility
    fn set_panel_visible(&mut self, panel: PanelType, visible: bool) {
        match panel {
            PanelType::Cpu => self.panels.cpu = visible,
            PanelType::Memory => self.panels.memory = visible,
            PanelType::Disk => self.panels.disk = visible,
            PanelType::Network => self.panels.network = visible,
            PanelType::Process => self.panels.process = visible,
            PanelType::Gpu => self.panels.gpu = visible,
            PanelType::Sensors => self.panels.sensors = visible,
            PanelType::Connections => self.panels.connections = visible,
            PanelType::Psi => self.panels.psi = visible,
            PanelType::Battery => self.panels.battery = visible,
            PanelType::Files => self.panels.files = visible,
            PanelType::Containers => {} // Not implemented
        }
    }

    /// Send a signal to a process using the system `kill` command
    #[cfg(unix)]
    fn send_signal(&self, pid: u32, signal: SignalType) -> (bool, String) {
        use std::process::Command;

        // Use the system `kill` command for safe signal sending
        let output = Command::new("kill")
            .arg(format!("-{}", signal.number()))
            .arg(pid.to_string())
            .output();

        match output {
            Ok(result) if result.status.success() => {
                (true, format!("Sent SIG{} to PID {}", signal.name(), pid))
            }
            Ok(result) => {
                let stderr = String::from_utf8_lossy(&result.stderr);
                (false, format!(
                    "Failed to send SIG{} to {}: {}",
                    signal.name(),
                    pid,
                    stderr.trim()
                ))
            }
            Err(e) => (false, format!("Failed to send SIG{} to {}: {}", signal.name(), pid, e)),
        }
    }

    #[cfg(not(unix))]
    fn send_signal(&self, pid: u32, signal: SignalType) -> (bool, String) {
        (false, format!(
            "Signal {} not supported on this platform (PID {})",
            signal.name(),
            pid
        ))
    }

    /// Get sorted and filtered processes
    pub fn sorted_processes(&self) -> Vec<&sysinfo::Process> {
        // ttop deterministic mode: empty process list
        if self.deterministic {
            return Vec::new();
        }
        let mut procs: Vec<_> = self
            .system
            .processes()
            .values()
            .filter(|p| {
                if self.filter.is_empty() {
                    true
                } else {
                    let name = p.name().to_string_lossy().to_lowercase();
                    name.contains(&self.filter.to_lowercase())
                }
            })
            .collect();

        procs.sort_by(|a, b| {
            let cmp = match self.sort_column {
                ProcessSortColumn::Pid => a.pid().as_u32().cmp(&b.pid().as_u32()),
                ProcessSortColumn::User => {
                    let ua = a.user_id().map(|u| u.to_string()).unwrap_or_default();
                    let ub = b.user_id().map(|u| u.to_string()).unwrap_or_default();
                    ua.cmp(&ub)
                }
                ProcessSortColumn::Cpu => a
                    .cpu_usage()
                    .partial_cmp(&b.cpu_usage())
                    .unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortColumn::Mem => a.memory().cmp(&b.memory()),
                ProcessSortColumn::Command => {
                    let na = a.name().to_string_lossy();
                    let nb = b.name().to_string_lossy();
                    na.cmp(&nb)
                }
            };
            if self.sort_descending {
                cmp.reverse()
            } else {
                cmp
            }
        });

        procs
    }

    /// Get Disk I/O data if available
    pub fn disk_io_data(&self) -> Option<&super::analyzers::DiskIoData> {
        // Use snapshot data for CB-INPUT-006 async pattern consistency
        self.snapshot_disk_io.as_ref()
    }

    /// Get system uptime in seconds
    pub fn uptime(&self) -> u64 {
        if self.deterministic {
            self.fixed_uptime
        } else {
            System::uptime()
        }
    }
}

// ============================================================================
// Cached System Info Helpers (read once at startup, O(1) render)
// ============================================================================

/// Read hostname from /etc/hostname (called once at startup)
fn read_hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Read kernel version from /proc/version (called once at startup)
fn read_kernel_version() -> String {
    std::fs::read_to_string("/proc/version")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|_| "Linux".to_string())
}

/// Detect if running in a container (called once at startup)
fn detect_container() -> bool {
    std::path::Path::new("/.dockerenv").exists()
        || std::fs::read_to_string("/proc/1/cgroup")
            .map(|s| s.contains("docker") || s.contains("containerd"))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // RingBuffer tests
    #[test]
    fn test_ring_buffer() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.as_slice(), &[1, 2, 3]);

        buf.push(4);
        assert_eq!(buf.as_slice(), &[2, 3, 4]);

        assert_eq!(buf.last(), Some(&4));
    }

    #[test]
    fn test_ring_buffer_empty() {
        let buf: RingBuffer<i32> = RingBuffer::new(5);
        assert!(buf.as_slice().is_empty());
        assert_eq!(buf.last(), None);
    }

    #[test]
    fn test_ring_buffer_single_element() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(1);
        buf.push(42);
        assert_eq!(buf.as_slice(), &[42]);

        buf.push(100);
        assert_eq!(buf.as_slice(), &[100]);
    }

    #[test]
    fn test_ring_buffer_many_pushes() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(3);
        for i in 0..100 {
            buf.push(i);
        }
        assert_eq!(buf.as_slice(), &[97, 98, 99]);
    }

    // ProcessSortColumn tests
    #[test]
    fn test_process_sort_column_next() {
        assert_eq!(ProcessSortColumn::Pid.next(), ProcessSortColumn::User);
        assert_eq!(ProcessSortColumn::User.next(), ProcessSortColumn::Cpu);
        assert_eq!(ProcessSortColumn::Cpu.next(), ProcessSortColumn::Mem);
        assert_eq!(ProcessSortColumn::Mem.next(), ProcessSortColumn::Command);
        assert_eq!(ProcessSortColumn::Command.next(), ProcessSortColumn::Pid);
    }

    #[test]
    fn test_process_sort_column_prev() {
        assert_eq!(ProcessSortColumn::Pid.prev(), ProcessSortColumn::Command);
        assert_eq!(ProcessSortColumn::User.prev(), ProcessSortColumn::Pid);
        assert_eq!(ProcessSortColumn::Cpu.prev(), ProcessSortColumn::User);
        assert_eq!(ProcessSortColumn::Mem.prev(), ProcessSortColumn::Cpu);
        assert_eq!(ProcessSortColumn::Command.prev(), ProcessSortColumn::Mem);
    }

    #[test]
    fn test_process_sort_column_count() {
        assert_eq!(ProcessSortColumn::COUNT, 5);
    }

    #[test]
    fn test_process_sort_column_next_cycle() {
        let mut col = ProcessSortColumn::Pid;
        for _ in 0..5 {
            col = col.next();
        }
        assert_eq!(col, ProcessSortColumn::Pid); // Full cycle
    }

    #[test]
    fn test_process_sort_column_prev_cycle() {
        let mut col = ProcessSortColumn::Pid;
        for _ in 0..5 {
            col = col.prev();
        }
        assert_eq!(col, ProcessSortColumn::Pid); // Full cycle
    }

    // PanelVisibility tests
    #[test]
    fn test_panel_visibility_default() {
        let panels = PanelVisibility::default();
        assert!(panels.cpu);
        assert!(panels.memory);
        assert!(panels.disk);
        assert!(panels.network);
        assert!(panels.process);
        assert!(!panels.gpu);
        assert!(!panels.sensors);
        assert!(!panels.psi);
        assert!(!panels.connections);
        assert!(!panels.battery);
        assert!(!panels.sensors_compact);
        assert!(!panels.system);
        assert!(!panels.treemap);
        assert!(!panels.files);
    }

    #[test]
    fn test_panel_visibility_all_fields() {
        let panels = PanelVisibility {
            cpu: true,
            memory: true,
            disk: true,
            network: true,
            process: true,
            gpu: true,
            sensors: true,
            psi: true,
            connections: true,
            battery: true,
            sensors_compact: true,
            system: true,
            treemap: true,
            files: true,
        };
        assert!(panels.cpu && panels.gpu && panels.treemap && panels.files);
    }

    // App tests
    #[test]
    fn test_app_normal_mode() {
        let app = App::new(false);
        assert!(!app.deterministic);
        assert!(!app.show_fps);
    }

    #[test]
    fn test_app_deterministic_mode() {
        let app = App::new(true);
        assert!(app.deterministic);

        // Check fixed values - ttop deterministic mode uses 48 cores, all zeros
        assert_eq!(app.per_core_percent.len(), 48);
        // ttop deterministic: all memory values are 0
        assert_eq!(app.mem_total, 0);
        assert_eq!(app.mem_used, 0);
        assert_eq!(app.swap_total, 0);

        // Check fixed uptime (5 days, 3 hours, 47 minutes)
        assert_eq!(app.uptime(), 5 * 86400 + 3 * 3600 + 47 * 60);

        // Check history is pre-populated with 60 zeros
        assert_eq!(app.cpu_history.as_slice().len(), 60);
        assert_eq!(app.mem_history.as_slice().len(), 60);
    }

    #[test]
    fn test_deterministic_mode_collect_metrics_noop() {
        let mut app = App::new(true);
        let initial_frame_id = app.frame_id;
        let initial_cpu_history_len = app.cpu_history.as_slice().len();

        // Collect should only increment frame_id, not change data
        app.collect_metrics();

        assert_eq!(app.frame_id, initial_frame_id + 1);
        // History should NOT grow (deterministic mode skips collection)
        assert_eq!(app.cpu_history.as_slice().len(), initial_cpu_history_len);
    }

    #[test]
    fn test_app_process_count_deterministic() {
        let app = App::new(true);
        // Deterministic mode returns 0 processes
        assert_eq!(app.process_count(), 0);
    }

    #[test]
    fn test_app_sorted_processes_deterministic() {
        let app = App::new(true);
        // Deterministic mode returns empty process list
        assert!(app.sorted_processes().is_empty());
    }

    #[test]
    fn test_app_focus_panel() {
        let mut app = App::new(true);
        // Default is CPU focused
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));

        app.focused_panel = Some(PanelType::Memory);
        assert_eq!(app.focused_panel, Some(PanelType::Memory));

        app.focused_panel = None;
        assert!(app.focused_panel.is_none());
    }

    #[test]
    fn test_app_is_panel_focused() {
        let mut app = App::new(true);
        // Default is CPU focused
        assert!(app.is_panel_focused(PanelType::Cpu));
        assert!(!app.is_panel_focused(PanelType::Memory));

        app.focused_panel = Some(PanelType::Memory);
        assert!(!app.is_panel_focused(PanelType::Cpu));
        assert!(app.is_panel_focused(PanelType::Memory));
    }

    #[test]
    fn test_app_sort_column_toggle() {
        let mut app = App::new(true);
        assert_eq!(app.sort_column, ProcessSortColumn::Cpu);

        app.sort_column = app.sort_column.next();
        assert_eq!(app.sort_column, ProcessSortColumn::Mem);
    }

    #[test]
    fn test_app_sort_descending_toggle() {
        let mut app = App::new(true);
        // Default is descending (highest first)
        assert!(app.sort_descending);

        app.sort_descending = false;
        assert!(!app.sort_descending);
    }

    #[test]
    fn test_app_filter_field_assignment() {
        // NOTE: This only tests the filter FIELD, not actual process filtering.
        // For actual filtering tests, see falsification_tests.rs:
        // - falsify_filter_does_not_reduce_count
        // - falsify_filter_does_not_match_known_process
        let mut app = App::new(true);
        assert!(app.filter.is_empty());

        app.filter = "test".to_string();
        assert_eq!(app.filter, "test");
    }

    #[test]
    fn test_app_update_frame_stats() {
        let mut app = App::new(true);
        app.update_frame_stats(&[
            Duration::from_micros(1000),
            Duration::from_micros(2000),
            Duration::from_micros(3000),
        ]);
        assert_eq!(app.avg_frame_time_us, 2000);
    }

    #[test]
    fn test_app_update_frame_stats_empty() {
        let mut app = App::new(true);
        app.avg_frame_time_us = 1234;
        app.update_frame_stats(&[]);
        // Should not change when empty
        assert_eq!(app.avg_frame_time_us, 1234);
    }

    #[test]
    fn test_app_request_signal_deterministic_noop() {
        // NOTE: This test only verifies no-op behavior in deterministic mode (no processes).
        // For actual signal request testing, see falsification_tests.rs:
        // - falsify_request_signal_sets_pending
        let mut app = App::new(true);
        // Deterministic mode has no processes, so this should be a no-op
        app.request_signal(SignalType::Term);
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_app_cancel_signal() {
        let mut app = App::new(true);
        app.pending_signal = Some((123, "test".to_string(), SignalType::Term));
        app.cancel_signal();
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_app_data_availability_deterministic() {
        let app = App::new(true);
        let avail = app.data_availability();
        // Deterministic mode has no optional data
        assert!(!avail.psi_available);
        assert!(!avail.gpu_available);
        assert!(!avail.treemap_ready);
    }

    #[test]
    fn test_app_apply_snapshot() {
        let mut app = App::new(true);
        let initial_frame = app.frame_id;

        let snapshot = MetricsSnapshot {
            per_core_percent: vec![25.0; 4],
            per_core_freq: vec![2000; 4],
            per_core_temp: vec![50.0; 4],
            cpu_avg: 25.0,
            load_avg: sysinfo::LoadAvg {
                one: 1.0,
                five: 0.5,
                fifteen: 0.25,
            },
            mem_total: 16 * 1024 * 1024 * 1024,
            mem_used: 8 * 1024 * 1024 * 1024,
            mem_available: 8 * 1024 * 1024 * 1024,
            mem_cached: 2 * 1024 * 1024 * 1024,
            swap_total: 4 * 1024 * 1024 * 1024,
            swap_used: 0,
            net_rx: 1000,
            net_tx: 500,
            gpu_info: None,
            processes: vec![],
            disk_info: vec![],
            network_info: vec![],
            psi_data: None,
            connections_data: None,
            treemap_data: None,
            sensor_health_data: None,
            disk_io_data: None,
            disk_entropy_data: None,
            file_analyzer_data: None,
        };

        app.apply_snapshot(snapshot);

        assert_eq!(app.frame_id, initial_frame + 1);
        assert_eq!(app.per_core_percent.len(), 4);
        assert_eq!(app.mem_total, 16 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_detail_level_for_panel() {
        let app = App::new(true);
        let level = app.detail_level_for_panel(PanelType::Cpu, 20);
        // Height 20 should give some detail level
        assert!(!matches!(level, DetailLevel::Minimal));
    }

    // =========================================================================
    // MetricsSnapshot TESTS
    // =========================================================================

    #[test]
    fn test_metrics_snapshot_empty() {
        let snap = MetricsSnapshot::empty();
        assert!((snap.cpu_avg - 0.0).abs() < f64::EPSILON);
        assert!(snap.per_core_percent.is_empty());
        assert!(snap.per_core_freq.is_empty());
        assert!(snap.per_core_temp.is_empty());
        assert_eq!(snap.mem_total, 0);
        assert_eq!(snap.mem_used, 0);
        assert_eq!(snap.swap_total, 0);
        assert_eq!(snap.net_rx, 0);
        assert_eq!(snap.net_tx, 0);
        assert!(snap.gpu_info.is_none());
        assert!(snap.processes.is_empty());
        assert!(snap.disk_info.is_empty());
        assert!(snap.network_info.is_empty());
        assert!(snap.psi_data.is_none());
        assert!(snap.connections_data.is_none());
        assert!(snap.treemap_data.is_none());
    }

    #[test]
    fn test_metrics_snapshot_clone() {
        let snap = MetricsSnapshot::empty();
        let snap2 = snap.clone();
        assert_eq!(snap2.mem_total, 0);
    }

    #[test]
    fn test_metrics_snapshot_with_values() {
        let snap = MetricsSnapshot {
            cpu_avg: 50.0,
            per_core_percent: vec![25.0, 50.0, 75.0, 100.0],
            per_core_freq: vec![3000, 3200, 3400, 3600],
            per_core_temp: vec![40.0, 45.0, 50.0, 55.0],
            load_avg: sysinfo::LoadAvg {
                one: 1.5,
                five: 1.0,
                fifteen: 0.5,
            },
            mem_total: 16 * 1024 * 1024 * 1024,
            mem_used: 8 * 1024 * 1024 * 1024,
            mem_available: 8 * 1024 * 1024 * 1024,
            mem_cached: 2 * 1024 * 1024 * 1024,
            swap_total: 4 * 1024 * 1024 * 1024,
            swap_used: 1024 * 1024 * 1024,
            net_rx: 1_000_000,
            net_tx: 500_000,
            gpu_info: None,
            processes: vec![],
            disk_info: vec![],
            network_info: vec![],
            psi_data: None,
            connections_data: None,
            treemap_data: None,
            sensor_health_data: None,
            disk_io_data: None,
            disk_entropy_data: None,
            file_analyzer_data: None,
        };
        assert!((snap.cpu_avg - 50.0).abs() < f64::EPSILON);
        assert_eq!(snap.per_core_percent.len(), 4);
        assert_eq!(snap.per_core_freq[3], 3600);
        assert!((snap.per_core_temp[0] - 40.0).abs() < f32::EPSILON);
    }

    // =========================================================================
    // ProcessInfo TESTS
    // =========================================================================

    #[test]
    fn test_process_info_clone() {
        let info = ProcessInfo {
            pid: 1234,
            name: "test".to_string(),
            cpu_usage: 25.5,
            memory: 1024 * 1024,
            user: "root".to_string(),
            cmd: "/usr/bin/test".to_string(),
        };
        let info2 = info.clone();
        assert_eq!(info2.pid, 1234);
        assert_eq!(info2.name, "test");
        assert!((info2.cpu_usage - 25.5).abs() < f32::EPSILON);
    }

    // =========================================================================
    // DiskInfo TESTS
    // =========================================================================

    #[test]
    fn test_disk_info_clone() {
        let info = DiskInfo {
            name: "sda1".to_string(),
            mount_point: "/".to_string(),
            total_space: 500 * 1024 * 1024 * 1024,
            available_space: 200 * 1024 * 1024 * 1024,
            file_system: "ext4".to_string(),
        };
        let info2 = info.clone();
        assert_eq!(info2.name, "sda1");
        assert_eq!(info2.mount_point, "/");
        assert_eq!(info2.file_system, "ext4");
    }

    // =========================================================================
    // NetworkInfo TESTS
    // =========================================================================

    #[test]
    fn test_network_info_clone() {
        let info = NetworkInfo {
            name: "eth0".to_string(),
            received: 1_000_000,
            transmitted: 500_000,
        };
        let info2 = info.clone();
        assert_eq!(info2.name, "eth0");
        assert_eq!(info2.received, 1_000_000);
        assert_eq!(info2.transmitted, 500_000);
    }

    // =========================================================================
    // MetricsCollector TESTS
    // =========================================================================

    #[test]
    fn test_metrics_collector_new_deterministic() {
        let collector = MetricsCollector::new(true);
        assert!(collector.deterministic);
        assert_eq!(collector.frame_id, 0);
    }

    #[test]
    fn test_metrics_collector_has_psi_returns_bool() {
        // NOTE: has_psi() returns true if /proc/pressure/cpu exists on host.
        // Deterministic mode still detects real system capabilities.
        // For actual PSI falsification, see falsification_tests.rs.
        let collector = MetricsCollector::new(true);
        let has_psi: bool = collector.has_psi();
        // Just verify it returns a bool and doesn't panic
        let _: bool = has_psi; // type check
    }

    #[test]
    fn test_metrics_collector_has_gpu_returns_bool() {
        // NOTE: has_gpu() returns true if GPU detected on host.
        // For actual GPU falsification, see falsification_tests.rs.
        let collector = MetricsCollector::new(true);
        let has_gpu: bool = collector.has_gpu();
        let _: bool = has_gpu; // type check
    }

    #[test]
    fn test_metrics_collector_has_sensors_returns_bool() {
        // NOTE: has_sensors() returns true if hwmon detected on host.
        let collector = MetricsCollector::new(true);
        let has_sensors: bool = collector.has_sensors();
        let _: bool = has_sensors; // type check
    }

    #[test]
    fn test_metrics_collector_has_connections_returns_bool() {
        // NOTE: has_connections() returns true if /proc/net/tcp readable.
        let collector = MetricsCollector::new(true);
        let has_connections: bool = collector.has_connections();
        let _: bool = has_connections; // type check
    }

    #[test]
    fn test_metrics_collector_has_treemap_returns_bool() {
        // NOTE: has_treemap() returns true if treemap analyzer available.
        let collector = MetricsCollector::new(true);
        let has_treemap: bool = collector.has_treemap();
        let _: bool = has_treemap; // type check
    }

    // =========================================================================
    // RingBuffer ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn test_ring_buffer_len() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(5);
        assert_eq!(buf.as_slice().len(), 0);
        buf.push(1);
        assert_eq!(buf.as_slice().len(), 1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.as_slice().len(), 3);
    }

    #[test]
    fn test_ring_buffer_capacity_maintained() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(3);
        for i in 0..10 {
            buf.push(i);
            assert!(buf.as_slice().len() <= 3);
        }
    }

    #[test]
    fn test_ring_buffer_with_floats() {
        let mut buf: RingBuffer<f64> = RingBuffer::new(3);
        buf.push(1.5);
        buf.push(2.5);
        buf.push(3.5);
        assert!((buf.as_slice()[0] - 1.5).abs() < f64::EPSILON);
        assert!((buf.last().unwrap() - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ring_buffer_with_strings() {
        let mut buf: RingBuffer<String> = RingBuffer::new(2);
        buf.push("hello".to_string());
        buf.push("world".to_string());
        assert_eq!(buf.as_slice(), &["hello", "world"]);

        buf.push("rust".to_string());
        assert_eq!(buf.as_slice(), &["world", "rust"]);
    }

    // =========================================================================
    // App ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn test_app_uptime() {
        let app = App::new(true);
        // Deterministic mode has fixed uptime (5d 3h 47m)
        let uptime = app.uptime();
        assert_eq!(uptime, 5 * 86400 + 3 * 3600 + 47 * 60);
    }

    #[test]
    fn test_app_frame_id_starts_at_zero() {
        let app = App::new(true);
        assert_eq!(app.frame_id, 0);
    }

    #[test]
    fn test_app_show_filter_input() {
        let mut app = App::new(true);
        assert!(!app.show_filter_input);
        app.show_filter_input = true;
        assert!(app.show_filter_input);
    }

    #[test]
    fn test_app_show_help() {
        let mut app = App::new(true);
        assert!(!app.show_help);
        app.show_help = true;
        assert!(app.show_help);
    }

    #[test]
    fn test_app_show_fps() {
        let mut app = App::new(true);
        assert!(!app.show_fps);
        app.show_fps = true;
        assert!(app.show_fps);
    }

    #[test]
    fn test_app_cpu_history_initial() {
        let app = App::new(true);
        // Deterministic mode pre-fills with 60 zeros
        assert_eq!(app.cpu_history.as_slice().len(), 60);
        for &val in app.cpu_history.as_slice() {
            assert!((val - 0.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_app_mem_history_initial() {
        let app = App::new(true);
        // Deterministic mode pre-fills with 60 zeros
        assert_eq!(app.mem_history.as_slice().len(), 60);
    }

    #[test]
    fn test_app_panels_visibility() {
        let app = App::new(true);
        assert!(app.panels.cpu);
        assert!(app.panels.memory);
        assert!(app.panels.disk);
        assert!(app.panels.network);
        assert!(app.panels.process);
    }

    #[test]
    fn test_app_core_count_deterministic() {
        let app = App::new(true);
        // Deterministic mode has 48 cores
        assert_eq!(app.per_core_percent.len(), 48);
    }

    #[test]
    fn test_app_with_config() {
        let config = PtopConfig::default();
        let app = App::with_config(true, config);
        assert!(app.deterministic);
    }

    #[test]
    fn test_app_with_config_lightweight() {
        let config = PtopConfig::default();
        let app = App::with_config_lightweight(true, config);
        assert!(app.deterministic);
    }

    #[test]
    fn test_app_multiple_collect_metrics() {
        let mut app = App::new(true);
        for _ in 0..5 {
            app.collect_metrics();
        }
        assert_eq!(app.frame_id, 5);
    }

    #[test]
    fn test_app_data_availability_fields() {
        let app = App::new(true);
        let avail = app.data_availability();
        // Just verify all fields exist
        let _psi = avail.psi_available;
        let _gpu = avail.gpu_available;
        let _treemap = avail.treemap_ready;
    }

    #[test]
    fn test_app_process_selected() {
        let mut app = App::new(true);
        assert_eq!(app.process_selected, 0);
        app.process_selected = 5;
        assert_eq!(app.process_selected, 5);
    }

    #[test]
    fn test_app_process_scroll_offset() {
        let mut app = App::new(true);
        assert_eq!(app.process_scroll_offset, 0);
        app.process_scroll_offset = 10;
        assert_eq!(app.process_scroll_offset, 10);
    }

    #[test]
    fn test_panel_visibility_fields() {
        let panels = PanelVisibility::default();
        // Test field access for all fields
        let _ = panels.cpu;
        let _ = panels.memory;
        let _ = panels.disk;
        let _ = panels.network;
        let _ = panels.process;
        let _ = panels.gpu;
        let _ = panels.sensors;
        let _ = panels.psi;
        let _ = panels.connections;
        let _ = panels.battery;
        let _ = panels.sensors_compact;
        let _ = panels.system;
        let _ = panels.treemap;
        let _ = panels.files;
    }

    #[test]
    fn test_app_net_history() {
        let app = App::new(true);
        // Check network history exists
        let _ = app.net_rx_history.as_slice().len();
        let _ = app.net_tx_history.as_slice().len();
    }

    #[test]
    fn test_process_sort_column_label() {
        // Test that the column enum has proper variants
        let col = ProcessSortColumn::Pid;
        assert!(matches!(col, ProcessSortColumn::Pid));

        let col = ProcessSortColumn::User;
        assert!(matches!(col, ProcessSortColumn::User));

        let col = ProcessSortColumn::Cpu;
        assert!(matches!(col, ProcessSortColumn::Cpu));

        let col = ProcessSortColumn::Mem;
        assert!(matches!(col, ProcessSortColumn::Mem));

        let col = ProcessSortColumn::Command;
        assert!(matches!(col, ProcessSortColumn::Command));
    }

    #[test]
    fn test_app_load_avg_deterministic() {
        let app = App::new(true);
        // Deterministic mode has zero load average
        assert!((app.load_avg.one - 0.0).abs() < f64::EPSILON);
        assert!((app.load_avg.five - 0.0).abs() < f64::EPSILON);
        assert!((app.load_avg.fifteen - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_app_snapshot_disks() {
        let app = App::new(true);
        // Check snapshot_disks field exists
        let _ = app.snapshot_disks.len();
    }

    #[test]
    fn test_app_snapshot_networks() {
        let app = App::new(true);
        // Check snapshot_networks field exists
        let _ = app.snapshot_networks.len();
    }

    #[test]
    fn test_app_snapshot_processes() {
        let app = App::new(true);
        // Check snapshot_processes field exists
        let _ = app.snapshot_processes.len();
    }

    #[test]
    fn test_app_hostname() {
        let app = App::new(true);
        // Hostname field should exist
        let _ = app.hostname.len();
    }

    #[test]
    fn test_app_kernel_version() {
        let app = App::new(true);
        // Kernel version field should exist
        let _ = app.kernel_version.len();
    }

    #[test]
    fn test_app_in_container() {
        let app = App::new(true);
        // Just verify field exists
        let _ = app.in_container;
    }

    #[test]
    fn test_app_running_state() {
        let mut app = App::new(true);
        assert!(app.running);
        app.running = false;
        assert!(!app.running);
    }

    #[test]
    fn test_app_exploded_panel() {
        let mut app = App::new(true);
        assert!(app.exploded_panel.is_none());
        app.exploded_panel = Some(PanelType::Cpu);
        assert!(app.exploded_panel.is_some());
    }

    #[test]
    fn test_app_selected_column() {
        let mut app = App::new(true);
        assert_eq!(app.selected_column, 0);
        app.selected_column = 3;
        assert_eq!(app.selected_column, 3);
    }

    // =========================================================================
    // ProcessSortColumn ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn test_process_sort_column_from_index() {
        assert_eq!(ProcessSortColumn::from_index(0), ProcessSortColumn::Pid);
        assert_eq!(ProcessSortColumn::from_index(1), ProcessSortColumn::User);
        assert_eq!(ProcessSortColumn::from_index(2), ProcessSortColumn::Cpu);
        assert_eq!(ProcessSortColumn::from_index(3), ProcessSortColumn::Mem);
        assert_eq!(ProcessSortColumn::from_index(4), ProcessSortColumn::Command);
        // Wrap around
        assert_eq!(ProcessSortColumn::from_index(5), ProcessSortColumn::Pid);
        assert_eq!(ProcessSortColumn::from_index(10), ProcessSortColumn::Pid);
    }

    #[test]
    fn test_process_sort_column_to_index() {
        assert_eq!(ProcessSortColumn::Pid.to_index(), 0);
        assert_eq!(ProcessSortColumn::User.to_index(), 1);
        assert_eq!(ProcessSortColumn::Cpu.to_index(), 2);
        assert_eq!(ProcessSortColumn::Mem.to_index(), 3);
        assert_eq!(ProcessSortColumn::Command.to_index(), 4);
    }

    #[test]
    fn test_process_sort_column_header_not_sorted() {
        assert_eq!(ProcessSortColumn::Pid.header(false, true), "PID");
        assert_eq!(ProcessSortColumn::User.header(false, true), "USER");
        assert_eq!(ProcessSortColumn::Cpu.header(false, true), "CPU%");
        assert_eq!(ProcessSortColumn::Mem.header(false, true), "MEM%");
        assert_eq!(ProcessSortColumn::Command.header(false, true), "COMMAND");
    }

    #[test]
    fn test_process_sort_column_header_sorted_desc() {
        assert_eq!(ProcessSortColumn::Pid.header(true, true), "PID▼");
        assert_eq!(ProcessSortColumn::Cpu.header(true, true), "CPU%▼");
    }

    #[test]
    fn test_process_sort_column_header_sorted_asc() {
        assert_eq!(ProcessSortColumn::Pid.header(true, false), "PID▲");
        assert_eq!(ProcessSortColumn::Command.header(true, false), "COMMAND▲");
    }

    // =========================================================================
    // handle_key() TESTS
    // =========================================================================

    #[test]
    fn test_handle_key_quit_q() {
        let mut app = App::new(true);
        assert!(app.handle_key(KeyCode::Char('q'), KeyModifiers::empty()));
    }

    #[test]
    fn test_handle_key_quit_ctrl_c() {
        let mut app = App::new(true);
        assert!(app.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    }

    #[test]
    fn test_handle_key_escape_quits() {
        let mut app = App::new(true);
        assert!(app.handle_key(KeyCode::Esc, KeyModifiers::empty()));
    }

    #[test]
    fn test_handle_key_help_toggle() {
        let mut app = App::new(true);
        assert!(!app.show_help);

        // '?' toggles help
        app.handle_key(KeyCode::Char('?'), KeyModifiers::empty());
        assert!(app.show_help);

        // '?' again toggles off
        app.handle_key(KeyCode::Char('?'), KeyModifiers::empty());
        assert!(!app.show_help);
    }

    #[test]
    fn test_handle_key_help_f1() {
        let mut app = App::new(true);
        app.handle_key(KeyCode::F(1), KeyModifiers::empty());
        assert!(app.show_help);
    }

    #[test]
    fn test_handle_key_h_toggles_help() {
        let mut app = App::new(true);
        app.handle_key(KeyCode::Char('h'), KeyModifiers::empty());
        assert!(app.show_help);
    }

    #[test]
    fn test_handle_key_in_help_mode_esc_closes() {
        let mut app = App::new(true);
        app.show_help = true;

        app.handle_key(KeyCode::Esc, KeyModifiers::empty());
        assert!(!app.show_help);
    }

    #[test]
    fn test_handle_key_in_help_mode_q_quits() {
        let mut app = App::new(true);
        app.show_help = true;

        assert!(app.handle_key(KeyCode::Char('q'), KeyModifiers::empty()));
    }

    #[test]
    fn test_handle_key_in_help_mode_swallows_other() {
        let mut app = App::new(true);
        app.show_help = true;

        // Random key should be swallowed, not quit
        assert!(!app.handle_key(KeyCode::Char('x'), KeyModifiers::empty()));
        assert!(app.show_help); // Still in help
    }

    #[test]
    fn test_handle_key_panel_toggles() {
        let mut app = App::new(true);

        // In deterministic mode: CPU, Memory, Disk, Network, Process, GPU, Sensors, Connections, Files are on
        // PSI is off

        // Toggle CPU off
        assert!(app.panels.cpu);
        app.handle_key(KeyCode::Char('1'), KeyModifiers::empty());
        assert!(!app.panels.cpu);

        // Toggle memory off
        assert!(app.panels.memory);
        app.handle_key(KeyCode::Char('2'), KeyModifiers::empty());
        assert!(!app.panels.memory);

        // Toggle disk off
        assert!(app.panels.disk);
        app.handle_key(KeyCode::Char('3'), KeyModifiers::empty());
        assert!(!app.panels.disk);

        // Toggle network off
        assert!(app.panels.network);
        app.handle_key(KeyCode::Char('4'), KeyModifiers::empty());
        assert!(!app.panels.network);

        // Toggle process off
        assert!(app.panels.process);
        app.handle_key(KeyCode::Char('5'), KeyModifiers::empty());
        assert!(!app.panels.process);

        // Toggle GPU off (it's on in deterministic mode)
        assert!(app.panels.gpu);
        app.handle_key(KeyCode::Char('6'), KeyModifiers::empty());
        assert!(!app.panels.gpu);

        // Toggle sensors off (it's on in deterministic mode)
        assert!(app.panels.sensors);
        app.handle_key(KeyCode::Char('7'), KeyModifiers::empty());
        assert!(!app.panels.sensors);

        // Toggle connections off (it's on in deterministic mode)
        assert!(app.panels.connections);
        app.handle_key(KeyCode::Char('8'), KeyModifiers::empty());
        assert!(!app.panels.connections);

        // Toggle PSI on (it's off in deterministic mode)
        assert!(!app.panels.psi);
        app.handle_key(KeyCode::Char('9'), KeyModifiers::empty());
        assert!(app.panels.psi);
    }

    #[test]
    fn test_handle_key_reset_panels() {
        let mut app = App::new(true);
        app.panels.cpu = false;
        app.panels.gpu = true;

        // '0' resets to defaults
        app.handle_key(KeyCode::Char('0'), KeyModifiers::empty());

        assert!(app.panels.cpu);
        assert!(!app.panels.gpu);
    }

    #[test]
    fn test_handle_key_sort_keys() {
        let mut app = App::new(true);

        // 'c' sorts by CPU
        app.handle_key(KeyCode::Char('c'), KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Cpu);
        assert!(app.sort_descending);

        // 'm' sorts by Memory
        app.handle_key(KeyCode::Char('m'), KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Mem);
        assert!(app.sort_descending);

        // 'p' sorts by PID
        app.handle_key(KeyCode::Char('p'), KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Pid);
        assert!(!app.sort_descending);

        // 'r' reverses sort
        app.handle_key(KeyCode::Char('r'), KeyModifiers::empty());
        assert!(app.sort_descending);

        // 's' cycles to next column
        app.sort_column = ProcessSortColumn::Cpu;
        app.handle_key(KeyCode::Char('s'), KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Mem);
    }

    #[test]
    fn test_handle_key_filter_mode() {
        let mut app = App::new(true);

        // '/' enters filter mode
        app.handle_key(KeyCode::Char('/'), KeyModifiers::empty());
        assert!(app.show_filter_input);

        // Type some characters
        app.handle_key(KeyCode::Char('t'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('e'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('s'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('t'), KeyModifiers::empty());
        assert_eq!(app.filter, "test");

        // Backspace removes character
        app.handle_key(KeyCode::Backspace, KeyModifiers::empty());
        assert_eq!(app.filter, "tes");

        // Enter exits filter mode
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert!(!app.show_filter_input);
        assert_eq!(app.filter, "tes"); // Filter remains
    }

    #[test]
    fn test_handle_key_filter_escape() {
        let mut app = App::new(true);
        app.show_filter_input = true;
        app.filter = "test".to_string();

        // Esc clears filter and exits
        app.handle_key(KeyCode::Esc, KeyModifiers::empty());
        assert!(!app.show_filter_input);
        assert!(app.filter.is_empty());
    }

    #[test]
    fn test_handle_key_delete_clears_filter() {
        let mut app = App::new(true);
        app.filter = "test".to_string();

        app.handle_key(KeyCode::Delete, KeyModifiers::empty());
        assert!(app.filter.is_empty());
    }

    #[test]
    fn test_handle_key_explode_panel() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        // Enter explodes focused panel
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert_eq!(app.exploded_panel, Some(PanelType::Cpu));

        // Esc collapses
        app.handle_key(KeyCode::Esc, KeyModifiers::empty());
        assert!(app.exploded_panel.is_none());
    }

    #[test]
    fn test_handle_key_explode_z() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Memory);

        // 'z' explodes focused panel
        app.handle_key(KeyCode::Char('z'), KeyModifiers::empty());
        assert_eq!(app.exploded_panel, Some(PanelType::Memory));

        // 'z' again collapses
        app.handle_key(KeyCode::Char('z'), KeyModifiers::empty());
        assert!(app.exploded_panel.is_none());
    }

    #[test]
    fn test_handle_key_tab_navigation() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        // Tab navigates forward
        app.handle_key(KeyCode::Tab, KeyModifiers::empty());
        assert_eq!(app.focused_panel, Some(PanelType::Memory));

        // BackTab navigates backward
        app.handle_key(KeyCode::BackTab, KeyModifiers::empty());
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
    }

    #[test]
    fn test_handle_key_process_navigation() {
        let mut app = App::new(true);

        // In deterministic mode, no processes, so navigation is noop
        app.handle_key(KeyCode::Down, KeyModifiers::empty());
        assert_eq!(app.process_selected, 0);

        app.handle_key(KeyCode::Up, KeyModifiers::empty());
        assert_eq!(app.process_selected, 0);
    }

    #[test]
    fn test_handle_key_signal_request_deterministic_noop() {
        // NOTE: This test only verifies no-op behavior in deterministic mode (no processes).
        // For actual 'x' key signal request testing, see falsification_tests.rs:
        // - falsify_x_key_creates_pending_signal
        let mut app = App::new(true);

        // In deterministic mode, no selected process, so request does nothing
        app.handle_key(KeyCode::Char('x'), KeyModifiers::empty());
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_handle_key_in_signal_confirmation() {
        let mut app = App::new(true);
        app.pending_signal = Some((1234, "test".to_string(), SignalType::Term));

        // 'n' cancels
        app.handle_key(KeyCode::Char('n'), KeyModifiers::empty());
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_handle_key_in_signal_confirmation_esc() {
        let mut app = App::new(true);
        app.pending_signal = Some((1234, "test".to_string(), SignalType::Term));

        // Esc cancels
        app.handle_key(KeyCode::Esc, KeyModifiers::empty());
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_handle_key_in_signal_confirmation_q_quits() {
        let mut app = App::new(true);
        app.pending_signal = Some((1234, "test".to_string(), SignalType::Term));

        assert!(app.handle_key(KeyCode::Char('q'), KeyModifiers::empty()));
    }

    #[test]
    fn test_handle_key_in_exploded_column_navigation() {
        let mut app = App::new(true);
        app.exploded_panel = Some(PanelType::Process);
        app.selected_column = 2;

        // Left moves column left
        app.handle_key(KeyCode::Left, KeyModifiers::empty());
        assert_eq!(app.selected_column, 1);

        // Right moves column right
        app.handle_key(KeyCode::Right, KeyModifiers::empty());
        assert_eq!(app.selected_column, 2);

        // At 0, left wraps to last
        app.selected_column = 0;
        app.handle_key(KeyCode::Left, KeyModifiers::empty());
        assert_eq!(app.selected_column, ProcessSortColumn::COUNT - 1);
    }

    #[test]
    fn test_handle_key_in_exploded_sort_toggle() {
        let mut app = App::new(true);
        app.exploded_panel = Some(PanelType::Process);
        app.sort_column = ProcessSortColumn::Cpu;
        app.selected_column = 2; // CPU column
        app.sort_descending = true;

        // Enter on same column toggles direction
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert!(!app.sort_descending);

        // Enter again toggles back
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert!(app.sort_descending);
    }

    #[test]
    fn test_handle_key_in_exploded_sort_new_column() {
        let mut app = App::new(true);
        app.exploded_panel = Some(PanelType::Process);
        app.sort_column = ProcessSortColumn::Cpu;
        app.selected_column = 0; // PID column
        app.sort_descending = true;

        // Enter on different column changes sort
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Pid);
        // PID is not numeric for descending default
        assert!(!app.sort_descending);
    }

    #[test]
    fn test_handle_key_in_exploded_quit() {
        let mut app = App::new(true);
        app.exploded_panel = Some(PanelType::Process);

        assert!(app.handle_key(KeyCode::Char('q'), KeyModifiers::empty()));
    }

    // =========================================================================
    // visible_panels() TESTS
    // =========================================================================

    #[test]
    fn test_visible_panels_default() {
        let app = App::new(true);
        let visible = app.visible_panels();

        // In deterministic mode: CPU, Memory, Disk, Network, Process, GPU, Sensors, Connections, Files
        assert_eq!(visible.len(), 9);
        assert!(visible.contains(&PanelType::Cpu));
        assert!(visible.contains(&PanelType::Memory));
        assert!(visible.contains(&PanelType::Disk));
        assert!(visible.contains(&PanelType::Network));
        assert!(visible.contains(&PanelType::Process));
        assert!(visible.contains(&PanelType::Gpu));
        assert!(visible.contains(&PanelType::Sensors));
        assert!(visible.contains(&PanelType::Connections));
        assert!(visible.contains(&PanelType::Files));
    }

    #[test]
    fn test_visible_panels_with_psi() {
        let mut app = App::new(true);
        app.panels.psi = true;

        let visible = app.visible_panels();
        assert_eq!(visible.len(), 10);
        assert!(visible.contains(&PanelType::Psi));
    }

    #[test]
    fn test_visible_panels_order() {
        let app = App::new(true);
        let visible = app.visible_panels();

        // Order should be: CPU, Memory, Disk, Network, Process, GPU, Sensors, Connections, Files
        assert_eq!(visible[0], PanelType::Cpu);
        assert_eq!(visible[1], PanelType::Memory);
        assert_eq!(visible[2], PanelType::Disk);
        assert_eq!(visible[3], PanelType::Network);
        assert_eq!(visible[4], PanelType::Process);
        assert_eq!(visible[5], PanelType::Gpu);
        assert_eq!(visible[6], PanelType::Sensors);
        assert_eq!(visible[7], PanelType::Connections);
        assert_eq!(visible[8], PanelType::Files);
    }

    #[test]
    fn test_visible_panels_empty() {
        let mut app = App::new(true);
        // Turn off all panels
        app.panels.cpu = false;
        app.panels.memory = false;
        app.panels.disk = false;
        app.panels.network = false;
        app.panels.process = false;
        app.panels.gpu = false;
        app.panels.sensors = false;
        app.panels.connections = false;
        app.panels.files = false;

        let visible = app.visible_panels();
        assert!(visible.is_empty());
    }

    // =========================================================================
    // navigate_panel_*() TESTS
    // =========================================================================

    #[test]
    fn test_navigate_panel_forward() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        app.navigate_panel_forward();
        assert_eq!(app.focused_panel, Some(PanelType::Memory));

        app.navigate_panel_forward();
        assert_eq!(app.focused_panel, Some(PanelType::Disk));
    }

    #[test]
    fn test_navigate_panel_forward_wraps() {
        let mut app = App::new(true);
        // In deterministic mode, Files is the last visible panel
        app.focused_panel = Some(PanelType::Files);

        // After Files (last), wraps to CPU
        app.navigate_panel_forward();
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
    }

    #[test]
    fn test_navigate_panel_backward() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Memory);

        app.navigate_panel_backward();
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
    }

    #[test]
    fn test_navigate_panel_backward_wraps() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        // Before CPU (first), wraps to Files (last in deterministic mode)
        app.navigate_panel_backward();
        assert_eq!(app.focused_panel, Some(PanelType::Files));
    }

    #[test]
    fn test_navigate_panel_empty_is_noop() {
        let mut app = App::new(true);
        // Turn off all panels
        app.panels.cpu = false;
        app.panels.memory = false;
        app.panels.disk = false;
        app.panels.network = false;
        app.panels.process = false;
        app.panels.gpu = false;
        app.panels.sensors = false;
        app.panels.connections = false;
        app.panels.files = false;
        app.focused_panel = None;

        app.navigate_panel_forward();
        assert!(app.focused_panel.is_none());

        app.navigate_panel_backward();
        assert!(app.focused_panel.is_none());
    }

    // =========================================================================
    // navigate_process() TESTS
    // =========================================================================

    #[test]
    fn test_navigate_process_down() {
        let mut app = App::new(true);
        // Deterministic mode has 0 processes, so navigation is bounded
        app.navigate_process(1);
        assert_eq!(app.process_selected, 0);
    }

    #[test]
    fn test_navigate_process_up() {
        let mut app = App::new(true);
        app.process_selected = 5;
        // With 0 processes, navigate is a no-op (early return)
        app.navigate_process(-1);
        // process_selected unchanged since count is 0
        assert_eq!(app.process_selected, 5);
    }

    // =========================================================================
    // evaluate_panel_display() TESTS
    // =========================================================================

    #[test]
    fn test_evaluate_panel_display_cpu() {
        let app = App::new(true);
        let action = app.evaluate_panel_display(PanelType::Cpu);
        // CPU should always show
        assert!(matches!(action, crate::widgets::DisplayAction::Show));
    }

    #[test]
    fn test_evaluate_panel_display_psi() {
        let app = App::new(true);
        let action = app.evaluate_panel_display(PanelType::Psi);
        // PSI not available in deterministic mode
        // Should be HideNoData or similar
        let _ = action; // Just verify it doesn't panic
    }

    #[test]
    fn test_data_availability_with_connections() {
        let mut app = App::new(true);
        app.snapshot_connections = Some(ConnectionsData {
            connections: vec![],
            state_counts: std::collections::HashMap::new(),
            count_history: vec![],
        });

        let avail = app.data_availability();
        assert!(avail.connections_available);
        assert_eq!(avail.connection_count, 0);
    }

    #[test]
    fn test_data_availability_with_treemap() {
        let mut app = App::new(true);
        app.snapshot_treemap = Some(TreemapData {
            root_path: std::path::PathBuf::from("/"),
            root: None,
            top_items: vec![],
            total_size: 0,
            total_files: 0,
            total_dirs: 0,
            depth: 0,
            last_scan: None,
            scan_duration: std::time::Duration::from_secs(0),
        });

        let avail = app.data_availability();
        // Empty top_items means not ready
        assert!(!avail.treemap_ready);
    }

    // =========================================================================
    // Signal handling ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn test_confirm_signal_with_no_pending() {
        let mut app = App::new(true);
        assert!(app.pending_signal.is_none());

        app.confirm_signal();
        // Should be no-op with no pending signal
        assert!(app.signal_result.is_none());
    }

    #[test]
    fn test_signal_type_name_and_number() {
        assert_eq!(SignalType::Term.name(), "TERM");
        assert_eq!(SignalType::Term.number(), 15);

        assert_eq!(SignalType::Kill.name(), "KILL");
        assert_eq!(SignalType::Kill.number(), 9);

        assert_eq!(SignalType::Hup.name(), "HUP");
        assert_eq!(SignalType::Hup.number(), 1);

        assert_eq!(SignalType::Int.name(), "INT");
        assert_eq!(SignalType::Int.number(), 2);

        assert_eq!(SignalType::Stop.name(), "STOP");
        assert_eq!(SignalType::Stop.number(), 19);
    }

    // =========================================================================
    // Signal result auto-clear tests (PMAT-GAP-033)
    // =========================================================================

    #[test]
    fn test_clear_old_signal_result_none() {
        let mut app = App::new(true);
        app.signal_result = None;
        app.clear_old_signal_result();
        assert!(app.signal_result.is_none());
    }

    #[test]
    fn test_clear_old_signal_result_recent() {
        let mut app = App::new(true);
        app.signal_result = Some((true, "test".to_string(), std::time::Instant::now()));
        app.clear_old_signal_result();
        assert!(app.signal_result.is_some()); // Not old enough to clear
    }

    #[test]
    fn test_signal_result_tuple_structure() {
        let mut app = App::new(true);
        let now = std::time::Instant::now();
        app.signal_result = Some((true, "Success message".to_string(), now));

        if let Some((success, message, timestamp)) = &app.signal_result {
            assert!(*success);
            assert_eq!(message, "Success message");
            assert!(timestamp.elapsed().as_secs() < 1);
        } else {
            panic!("Expected signal_result to be Some");
        }
    }

    #[test]
    fn test_signal_result_failure() {
        let mut app = App::new(true);
        let now = std::time::Instant::now();
        app.signal_result = Some((false, "Failed to send signal".to_string(), now));

        if let Some((success, message, _timestamp)) = &app.signal_result {
            assert!(!*success);
            assert!(message.contains("Failed"));
        } else {
            panic!("Expected signal_result to be Some");
        }
    }

    // =========================================================================
    // PMAT-GAP-031: Network interface cycling tests (ttop parity)
    // =========================================================================

    #[test]
    fn test_selected_interface_index_field_exists() {
        let app = App::new(true);
        // Field must exist and default to 0
        assert_eq!(app.selected_interface_index, 0);
    }

    #[test]
    fn test_cycle_interface_no_interfaces() {
        let mut app = App::new(true);
        // No interfaces available - should stay at 0
        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 0);
    }

    #[test]
    fn test_cycle_interface_wraps_around() {
        let mut app = App::new(true);
        // Simulate 3 interfaces
        app.snapshot_networks = vec![
            NetworkInfo {
                name: "eth0".to_string(),
                received: 0,
                transmitted: 0,
            },
            NetworkInfo {
                name: "wlan0".to_string(),
                received: 0,
                transmitted: 0,
            },
            NetworkInfo {
                name: "lo".to_string(),
                received: 0,
                transmitted: 0,
            },
        ];

        assert_eq!(app.selected_interface_index, 0);
        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 1);
        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 2);
        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 0); // Wraps around
    }

    #[test]
    fn test_selected_interface_name() {
        let mut app = App::new(true);
        app.snapshot_networks = vec![
            NetworkInfo {
                name: "eth0".to_string(),
                received: 100,
                transmitted: 200,
            },
            NetworkInfo {
                name: "wlan0".to_string(),
                received: 50,
                transmitted: 25,
            },
        ];

        assert_eq!(app.selected_interface_name(), Some("eth0"));
        app.selected_interface_index = 1;
        assert_eq!(app.selected_interface_name(), Some("wlan0"));
        app.selected_interface_index = 2; // Out of bounds
        assert_eq!(app.selected_interface_name(), None);
    }

    #[test]
    fn test_selected_interface_data() {
        let mut app = App::new(true);
        app.snapshot_networks = vec![NetworkInfo {
            name: "eth0".to_string(),
            received: 1000,
            transmitted: 500,
        }];

        let data = app.selected_interface_data();
        assert!(data.is_some());
        let info = data.unwrap();
        assert_eq!(info.name, "eth0");
        assert_eq!(info.received, 1000);
    }

    #[test]
    fn test_cycle_interface_single_interface() {
        let mut app = App::new(true);
        app.snapshot_networks = vec![NetworkInfo {
            name: "lo".to_string(),
            received: 0,
            transmitted: 0,
        }];

        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 0); // Stays at 0 (wraps from 1 to 0)
    }

    #[test]
    fn test_tab_cycles_interface_when_network_focused() {
        let mut app = App::new(true);
        app.snapshot_networks = vec![
            NetworkInfo {
                name: "eth0".to_string(),
                received: 0,
                transmitted: 0,
            },
            NetworkInfo {
                name: "wlan0".to_string(),
                received: 0,
                transmitted: 0,
            },
        ];
        app.focused_panel = Some(PanelType::Network);

        assert_eq!(app.selected_interface_index, 0);
        app.handle_key(KeyCode::Tab, KeyModifiers::empty());
        assert_eq!(app.selected_interface_index, 1);
        app.handle_key(KeyCode::Tab, KeyModifiers::empty());
        assert_eq!(app.selected_interface_index, 0); // Wraps
    }

    #[test]
    fn test_tab_navigates_panels_when_not_network_focused() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.memory = true;

        app.handle_key(KeyCode::Tab, KeyModifiers::empty());
        // Should navigate to next panel, not cycle interfaces
        assert_ne!(app.focused_panel, Some(PanelType::Cpu));
        assert_eq!(app.selected_interface_index, 0); // Unchanged
    }

    // =========================================================================
    // PMAT-GAP-034: Files view mode toggle tests (ttop parity)
    // =========================================================================

    #[test]
    fn test_files_view_mode_field_exists() {
        let app = App::new(true);
        // Field must exist and default to Size
        assert_eq!(app.files_view_mode, FilesViewMode::Size);
    }

    #[test]
    fn test_files_view_mode_next_cycle() {
        assert_eq!(FilesViewMode::Size.next(), FilesViewMode::Tree);
        assert_eq!(FilesViewMode::Tree.next(), FilesViewMode::Flat);
        assert_eq!(FilesViewMode::Flat.next(), FilesViewMode::Size);
    }

    #[test]
    fn test_files_view_mode_names() {
        assert_eq!(FilesViewMode::Tree.name(), "tree");
        assert_eq!(FilesViewMode::Flat.name(), "flat");
        assert_eq!(FilesViewMode::Size.name(), "size");
    }

    #[test]
    fn test_cycle_files_view_mode() {
        let mut app = App::new(true);
        assert_eq!(app.files_view_mode, FilesViewMode::Size);

        app.cycle_files_view_mode();
        assert_eq!(app.files_view_mode, FilesViewMode::Tree);

        app.cycle_files_view_mode();
        assert_eq!(app.files_view_mode, FilesViewMode::Flat);

        app.cycle_files_view_mode();
        assert_eq!(app.files_view_mode, FilesViewMode::Size);
    }

    #[test]
    fn test_v_key_cycles_view_mode_when_files_focused() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Files);
        app.panels.files = true;

        assert_eq!(app.files_view_mode, FilesViewMode::Size);
        app.handle_key(KeyCode::Char('v'), KeyModifiers::empty());
        assert_eq!(app.files_view_mode, FilesViewMode::Tree);
        app.handle_key(KeyCode::Char('v'), KeyModifiers::empty());
        assert_eq!(app.files_view_mode, FilesViewMode::Flat);
        app.handle_key(KeyCode::Char('v'), KeyModifiers::empty());
        assert_eq!(app.files_view_mode, FilesViewMode::Size);
    }

    #[test]
    fn test_v_key_does_nothing_when_files_not_focused() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        assert_eq!(app.files_view_mode, FilesViewMode::Size);
        app.handle_key(KeyCode::Char('v'), KeyModifiers::empty());
        assert_eq!(app.files_view_mode, FilesViewMode::Size); // Unchanged
    }

    // =========================================================================
    // PMAT-GAP-035: Panel collapse memory tests (ttop parity)
    // =========================================================================

    #[test]
    fn test_collapse_memory_field_exists() {
        let app = App::new(true);
        // Field must exist and default to None
        assert!(app.collapse_memory.is_none());
    }

    #[test]
    fn test_toggle_panel_hides_focused_stores_memory() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.cpu = true;
        app.panels.memory = true;

        // Toggle off CPU while focused
        app.toggle_panel(PanelType::Cpu);

        // CPU should be hidden
        assert!(!app.panels.cpu);
        // Focus should move to first visible (Memory)
        assert_eq!(app.focused_panel, Some(PanelType::Memory));
        // Collapse memory should store CPU
        assert_eq!(app.collapse_memory, Some(PanelType::Cpu));
    }

    #[test]
    fn test_toggle_panel_restore_focus_from_memory() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.cpu = true;
        app.panels.memory = true;

        // Hide CPU (stores in memory)
        app.toggle_panel(PanelType::Cpu);
        assert_eq!(app.collapse_memory, Some(PanelType::Cpu));
        assert_eq!(app.focused_panel, Some(PanelType::Memory));

        // Show CPU again (should restore focus)
        app.toggle_panel(PanelType::Cpu);
        assert!(app.panels.cpu);
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
        assert!(app.collapse_memory.is_none()); // Memory cleared
    }

    #[test]
    fn test_toggle_panel_no_memory_when_not_focused() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.cpu = true;
        app.panels.memory = true;

        // Toggle off Memory (not focused)
        app.toggle_panel(PanelType::Memory);

        // Memory should be hidden
        assert!(!app.panels.memory);
        // Focus should stay on CPU
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
        // Collapse memory should be empty (Memory wasn't focused)
        assert!(app.collapse_memory.is_none());
    }

    #[test]
    fn test_toggle_panel_key_binding_with_memory() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.cpu = true;
        app.panels.memory = true;

        // Press '1' to hide CPU
        app.handle_key(KeyCode::Char('1'), KeyModifiers::empty());
        assert!(!app.panels.cpu);
        assert_eq!(app.collapse_memory, Some(PanelType::Cpu));

        // Press '1' again to show CPU and restore focus
        app.handle_key(KeyCode::Char('1'), KeyModifiers::empty());
        assert!(app.panels.cpu);
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
        assert!(app.collapse_memory.is_none());
    }

    #[test]
    fn test_is_panel_visible() {
        let mut app = App::new(true);
        app.panels.cpu = true;
        app.panels.memory = false;

        assert!(app.is_panel_visible(PanelType::Cpu));
        assert!(!app.is_panel_visible(PanelType::Memory));
    }

    #[test]
    fn test_set_panel_visible() {
        let mut app = App::new(true);
        app.panels.cpu = true;

        app.set_panel_visible(PanelType::Cpu, false);
        assert!(!app.panels.cpu);

        app.set_panel_visible(PanelType::Cpu, true);
        assert!(app.panels.cpu);
    }
}
