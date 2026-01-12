//! Application state and data collectors for ptop.
//!
//! Mirrors ttop's app.rs - maintains system state and history.

use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Duration;

use sysinfo::{
    CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind, ProcessesToUpdate,
    System, Users,
};

use super::config::{DetailLevel, PanelType, PtopConfig};
use super::ui::{read_gpu_info, GpuInfo};

/// Read cached memory from /proc/meminfo (Linux only).
/// Returns bytes, or 0 if unavailable.
#[cfg(target_os = "linux")]
fn read_cached_memory() -> u64 {
    use std::fs;
    if let Ok(contents) = fs::read_to_string("/proc/meminfo") {
        for line in contents.lines() {
            // Look for "Cached:" line (not "SwapCached:")
            if line.starts_with("Cached:") && !line.starts_with("CachedSwap") {
                // Format: "Cached:          1234567 kB"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return kb * 1024; // Convert kB to bytes
                    }
                }
            }
        }
    }
    0
}

#[cfg(not(target_os = "linux"))]
fn read_cached_memory() -> u64 {
    // On non-Linux systems, return 0 (cached memory not available via /proc)
    0
}

/// Read per-core CPU temperatures from /sys/class/hwmon (Linux only).
/// Returns temperatures in °C, or zeros if unavailable.
#[cfg(target_os = "linux")]
fn read_core_temperatures(core_count: usize) -> Vec<f32> {
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    let mut temps = vec![0.0f32; core_count];

    let hwmon_dir = Path::new("/sys/class/hwmon");
    if !hwmon_dir.exists() {
        return temps;
    }

    // Find CPU temperature hwmon device
    let Ok(entries) = fs::read_dir(hwmon_dir) else {
        return temps;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name_path = path.join("name");
        let Ok(name) = fs::read_to_string(&name_path) else {
            continue;
        };
        let name = name.trim();

        if name == "k10temp" || name == "zenpower" {
            // AMD: Read by label (Tccd1-4), map to core groups
            // k10temp layout: temp1=Tctl, temp3=Tccd1, temp4=Tccd2, temp5=Tccd3, temp6=Tccd4
            // NOTE: temp2 does NOT exist on k10temp!
            let mut ccd_temps: HashMap<String, f32> = HashMap::new();

            // Discover all temp sensors by reading labels
            for i in 1..=10 {
                let label_path = path.join(format!("temp{i}_label"));
                let input_path = path.join(format!("temp{i}_input"));

                if let (Ok(label), Ok(input)) = (
                    fs::read_to_string(&label_path),
                    fs::read_to_string(&input_path),
                ) {
                    let label = label.trim().to_string();
                    if let Ok(millidegrees) = input.trim().parse::<i64>() {
                        ccd_temps.insert(label, millidegrees as f32 / 1000.0);
                    }
                }
            }

            // Map CCD temperatures to cores
            // AMD Threadripper 7960X: 24 cores, 4 CCDs, 6 cores per CCD (but 48 threads)
            // Tccd1 → cores 0-11, Tccd2 → cores 12-23, Tccd3 → cores 24-35, Tccd4 → cores 36-47
            let cores_per_ccd = core_count / 4;
            if let Some(&tccd1) = ccd_temps.get("Tccd1") {
                for i in 0..cores_per_ccd.min(core_count) {
                    temps[i] = tccd1;
                }
            }
            if let Some(&tccd2) = ccd_temps.get("Tccd2") {
                for i in cores_per_ccd..(cores_per_ccd * 2).min(core_count) {
                    temps[i] = tccd2;
                }
            }
            if let Some(&tccd3) = ccd_temps.get("Tccd3") {
                for i in (cores_per_ccd * 2)..(cores_per_ccd * 3).min(core_count) {
                    temps[i] = tccd3;
                }
            }
            if let Some(&tccd4) = ccd_temps.get("Tccd4") {
                for i in (cores_per_ccd * 3)..core_count {
                    temps[i] = tccd4;
                }
            }

            // Fallback to Tctl if no CCD temps found
            if temps.iter().all(|&t| t == 0.0) {
                if let Some(&tctl) = ccd_temps.get("Tctl") {
                    temps.fill(tctl);
                }
            }

            return temps;
        } else if name == "coretemp" {
            // Intel: temp2_input = Core 0, temp3_input = Core 1, etc.
            for i in 0..core_count {
                let temp_file = path.join(format!("temp{}_input", i + 2));
                if let Ok(temp_str) = fs::read_to_string(&temp_file) {
                    if let Ok(millidegrees) = temp_str.trim().parse::<i64>() {
                        temps[i] = millidegrees as f32 / 1000.0;
                    }
                }
            }

            // Fallback to package temp
            if temps.iter().all(|&t| t == 0.0) {
                let temp_file = path.join("temp1_input");
                if let Ok(temp_str) = fs::read_to_string(&temp_file) {
                    if let Ok(millidegrees) = temp_str.trim().parse::<i64>() {
                        temps.fill(millidegrees as f32 / 1000.0);
                    }
                }
            }

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

use super::analyzers::{AnalyzerRegistry, PsiData};
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

    // Panel navigation and explode (SPEC-024 v5.0 Features D, E)
    /// Currently focused panel (receives keyboard input)
    pub focused_panel: Option<PanelType>,
    /// Exploded (fullscreen) panel, if any
    pub exploded_panel: Option<PanelType>,
    /// Selected column index for DataFrame navigation (0-based, left-to-right)
    pub selected_column: usize,

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

    // Snapshot data from background collector (CB-INPUT-006)
    /// Process list from last snapshot
    pub snapshot_processes: Vec<ProcessInfo>,
    /// Disk info from last snapshot
    pub snapshot_disks: Vec<DiskInfo>,
    /// Network info from last snapshot
    pub snapshot_networks: Vec<NetworkInfo>,
    /// PSI data from last snapshot
    pub snapshot_psi: Option<PsiData>,
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
        let mut panels = PanelVisibility::default();
        if analyzers.psi.is_some() {
            panels.psi = true;
        }
        if analyzers.gpu_procs.is_some() {
            panels.gpu = true;
        }
        if analyzers.sensor_health.is_some() {
            panels.sensors = true;
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
            // Panel navigation (SPEC-024 v5.0 Feature D)
            focused_panel: Some(PanelType::Cpu), // Start with CPU focused
            exploded_panel: None,
            selected_column: 0, // Start with first column (PID)
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
            // Snapshot data (CB-INPUT-006)
            snapshot_processes: Vec::new(),
            snapshot_disks: Vec::new(),
            snapshot_networks: Vec::new(),
            snapshot_psi: None,
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
        self.analyzers.psi_data()
    }

    /// Collect metrics from all sources
    pub fn collect_metrics(&mut self) {
        self.frame_id += 1;

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
        self.snapshot_networks = snapshot.network_info;
        self.snapshot_psi = snapshot.psi_data;
    }

    /// Handle keyboard input. Returns true if app should quit.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        // Help overlay mode - block all inputs except close
        if self.show_help {
            match code {
                KeyCode::Esc | KeyCode::Char('?' | 'h') | KeyCode::F(1) => {
                    self.show_help = false;
                }
                KeyCode::Char('q') => return true,
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,
                _ => {} // Swallow all other inputs
            }
            return false;
        }

        // Exploded mode - DataFrame controls + Esc to collapse (SPEC-024 v5.0 Feature D)
        if self.exploded_panel.is_some() {
            match code {
                // Exit exploded mode
                KeyCode::Esc | KeyCode::Char('z') => {
                    self.exploded_panel = None;
                    return false;
                }
                KeyCode::Char('q') => return true,
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,

                // DataFrame column navigation (Left/Right or h/l)
                KeyCode::Left | KeyCode::Char('h') => {
                    if self.selected_column > 0 {
                        self.selected_column -= 1;
                    } else {
                        self.selected_column = ProcessSortColumn::COUNT - 1;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.selected_column = (self.selected_column + 1) % ProcessSortColumn::COUNT;
                }

                // Sort by selected column (Enter or Space)
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let new_col = ProcessSortColumn::from_index(self.selected_column);
                    if self.sort_column == new_col {
                        // Toggle direction if same column
                        self.sort_descending = !self.sort_descending;
                    } else {
                        // New column: default to descending for numeric, ascending for text
                        self.sort_column = new_col;
                        self.sort_descending =
                            matches!(new_col, ProcessSortColumn::Cpu | ProcessSortColumn::Mem);
                    }
                }

                // Row navigation (Up/Down or j/k)
                KeyCode::Up | KeyCode::Char('k') => self.navigate_process(-1),
                KeyCode::Down | KeyCode::Char('j') => self.navigate_process(1),
                KeyCode::PageUp => self.navigate_process(-10),
                KeyCode::PageDown => self.navigate_process(10),
                KeyCode::Home | KeyCode::Char('g') => self.process_selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    let count = self.process_count();
                    if count > 0 {
                        self.process_selected = count - 1;
                    }
                }

                // Quick sort keys
                KeyCode::Char('c') => {
                    self.sort_column = ProcessSortColumn::Cpu;
                    self.selected_column = ProcessSortColumn::Cpu.to_index();
                    self.sort_descending = true;
                }
                KeyCode::Char('m') => {
                    self.sort_column = ProcessSortColumn::Mem;
                    self.selected_column = ProcessSortColumn::Mem.to_index();
                    self.sort_descending = true;
                }
                KeyCode::Char('p') => {
                    self.sort_column = ProcessSortColumn::Pid;
                    self.selected_column = ProcessSortColumn::Pid.to_index();
                    self.sort_descending = false;
                }
                KeyCode::Char('n') => {
                    self.sort_column = ProcessSortColumn::Command;
                    self.selected_column = ProcessSortColumn::Command.to_index();
                    self.sort_descending = false;
                }
                KeyCode::Char('r') => self.sort_descending = !self.sort_descending,

                // Filter in exploded mode
                KeyCode::Char('/' | 'f') => {
                    self.show_filter_input = true;
                }

                _ => {} // Swallow other inputs
            }
            return false;
        }

        // Filter input mode
        if self.show_filter_input {
            match code {
                KeyCode::Esc => {
                    self.show_filter_input = false;
                    self.filter.clear();
                }
                KeyCode::Enter => {
                    self.show_filter_input = false;
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                }
                _ => {}
            }
            return false;
        }

        // Normal mode
        #[allow(clippy::match_same_arms)]
        match code {
            // Quit
            KeyCode::Char('q') => return true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,

            // Explode focused panel (SPEC-024 v5.0 Feature D)
            KeyCode::Enter | KeyCode::Char('z') => {
                if let Some(panel) = self.focused_panel {
                    self.exploded_panel = Some(panel);
                }
            }

            // Panel navigation - Tab cycles forward (SPEC-024 v5.0 Feature D)
            KeyCode::Tab if !modifiers.contains(KeyModifiers::SHIFT) => {
                self.navigate_panel_forward();
            }

            // Panel navigation - Shift+Tab cycles backward
            KeyCode::BackTab => {
                self.navigate_panel_backward();
            }

            // Vim-style panel navigation (hjkl)
            KeyCode::Char('l') if !self.show_filter_input => {
                self.navigate_panel_forward();
            }
            KeyCode::Char('H') => {
                self.navigate_panel_backward();
            }

            // Help
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = !self.show_help,
            KeyCode::Char('h') => self.show_help = !self.show_help,

            // Panel toggles (matches ttop keys)
            KeyCode::Char('1') => self.panels.cpu = !self.panels.cpu,
            KeyCode::Char('2') => self.panels.memory = !self.panels.memory,
            KeyCode::Char('3') => self.panels.disk = !self.panels.disk,
            KeyCode::Char('4') => self.panels.network = !self.panels.network,
            KeyCode::Char('5') => self.panels.process = !self.panels.process,
            KeyCode::Char('6') => self.panels.gpu = !self.panels.gpu,
            KeyCode::Char('7') => self.panels.sensors = !self.panels.sensors,
            KeyCode::Char('8') => self.panels.connections = !self.panels.connections,
            KeyCode::Char('9') => self.panels.psi = !self.panels.psi,

            // Process navigation (when Process panel focused)
            KeyCode::Down | KeyCode::Char('j') => self.navigate_process(1),
            KeyCode::Up | KeyCode::Char('k') => self.navigate_process(-1),
            KeyCode::PageDown => self.navigate_process(10),
            KeyCode::PageUp => self.navigate_process(-10),
            KeyCode::Home | KeyCode::Char('g') => self.process_selected = 0,
            KeyCode::End | KeyCode::Char('G') => {
                let count = self.process_count();
                if count > 0 {
                    self.process_selected = count - 1;
                }
            }

            // Sorting
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
            KeyCode::Char('s') => {
                self.sort_column = self.sort_column.next();
            }
            KeyCode::Char('r') => self.sort_descending = !self.sort_descending,

            // Filter
            KeyCode::Char('/' | 'f') => {
                self.show_filter_input = true;
            }
            KeyCode::Delete => self.filter.clear(),

            // Reset panels
            KeyCode::Char('0') => {
                self.panels = PanelVisibility::default();
            }

            // Escape in normal mode quits
            KeyCode::Esc => return true,

            _ => {}
        }

        false
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
        self.analyzers.disk_io_data()
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
    fn test_process_sort_column_next() {
        assert_eq!(ProcessSortColumn::Pid.next(), ProcessSortColumn::User);
        assert_eq!(ProcessSortColumn::User.next(), ProcessSortColumn::Cpu);
        assert_eq!(ProcessSortColumn::Command.next(), ProcessSortColumn::Pid);
    }

    #[test]
    fn test_panel_visibility_default() {
        let panels = PanelVisibility::default();
        assert!(panels.cpu);
        assert!(panels.memory);
        assert!(panels.process);
        assert!(!panels.gpu);
        assert!(!panels.treemap);
    }
}
