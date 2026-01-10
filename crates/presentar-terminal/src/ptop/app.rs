//! Application state and data collectors for ptop.
//!
//! Mirrors ttop's app.rs - maintains system state and history.

use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Duration;

use sysinfo::{
    CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind, ProcessesToUpdate,
    System,
};

use super::analyzers::{AnalyzerRegistry, PsiData};

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
    pub fn next(self) -> Self {
        match self {
            Self::Pid => Self::User,
            Self::User => Self::Cpu,
            Self::Cpu => Self::Mem,
            Self::Mem => Self::Command,
            Self::Command => Self::Pid,
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

/// Disk I/O rates (bytes per second)
#[derive(Debug, Default, Clone, Copy)]
pub struct DiskIoRates {
    /// Read bytes per second
    pub read_bytes_per_sec: f64,
    /// Write bytes per second
    pub write_bytes_per_sec: f64,
}

/// Main application state (mirrors ttop's App struct)
#[allow(clippy::struct_excessive_bools)]
pub struct App {
    // System collectors
    pub system: System,
    pub disks: Disks,
    pub networks: Networks,

    // Analyzers (detailed metrics from /proc, /sys)
    pub analyzers: AnalyzerRegistry,

    // History buffers (normalized 0-1)
    pub cpu_history: RingBuffer<f64>,
    pub mem_history: RingBuffer<f64>,
    pub net_rx_history: RingBuffer<f64>,
    pub net_tx_history: RingBuffer<f64>,

    // Per-core CPU percentages
    pub per_core_percent: Vec<f64>,

    // Memory values
    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_available: u64,
    pub mem_cached: u64,
    pub swap_total: u64,
    pub swap_used: u64,

    // Disk I/O rates
    pub disk_io_rates: DiskIoRates,
    prev_disk_read_bytes: u64,
    prev_disk_write_bytes: u64,

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

    // Frame timing
    pub frame_id: u64,
    pub avg_frame_time_us: u64,
    pub show_fps: bool,
}

impl App {
    /// Create new App with collectors initialized
    pub fn new(show_fps: bool) -> Self {
        let mut system = System::new();

        // Initial refresh (need 2 samples for CPU delta)
        // Use 50ms instead of 100ms for faster startup while still getting valid CPU readings
        system.refresh_cpu_specifics(CpuRefreshKind::everything());
        std::thread::sleep(Duration::from_millis(50));
        system.refresh_cpu_specifics(CpuRefreshKind::everything());
        system.refresh_memory_specifics(MemoryRefreshKind::everything());
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything()
                .with_cpu()
                .with_memory()
                .with_user(sysinfo::UpdateKind::OnlyIfNotSet),
        );

        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        let core_count = system.cpus().len();

        // Initialize analyzers and detect available features
        let analyzers = AnalyzerRegistry::new();

        // Auto-detect panel visibility based on available analyzers
        let mut panels = PanelVisibility::default();
        if analyzers.psi.is_some() {
            panels.psi = true;
        }

        Self {
            system,
            disks,
            networks,
            analyzers,
            cpu_history: RingBuffer::new(60),
            mem_history: RingBuffer::new(60),
            net_rx_history: RingBuffer::new(60),
            net_tx_history: RingBuffer::new(60),
            per_core_percent: vec![0.0; core_count],
            mem_total: 0,
            mem_used: 0,
            mem_available: 0,
            mem_cached: 0,
            swap_total: 0,
            swap_used: 0,
            disk_io_rates: DiskIoRates::default(),
            prev_disk_read_bytes: 0,
            prev_disk_write_bytes: 0,
            panels,
            process_selected: 0,
            process_scroll_offset: 0,
            sort_column: ProcessSortColumn::Cpu,
            sort_descending: true,
            filter: String::new(),
            show_filter_input: false,
            show_help: false,
            running: true,
            frame_id: 0,
            avg_frame_time_us: 0,
            show_fps,
        }
    }

    /// Get PSI data if available
    pub fn psi_data(&self) -> Option<&PsiData> {
        self.analyzers.psi_data()
    }

    /// Collect metrics from all sources
    pub fn collect_metrics(&mut self) {
        self.frame_id += 1;

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

        // Memory
        self.system
            .refresh_memory_specifics(MemoryRefreshKind::everything());

        self.mem_total = self.system.total_memory();
        self.mem_used = self.system.used_memory();
        self.mem_available = self.system.available_memory();
        self.mem_cached = self
            .mem_total
            .saturating_sub(self.mem_used + self.mem_available);
        self.swap_total = self.system.total_swap();
        self.swap_used = self.system.used_swap();

        if self.mem_total > 0 {
            self.mem_history
                .push(self.mem_used as f64 / self.mem_total as f64);
        }

        // Processes
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything()
                .with_cpu()
                .with_memory()
                .with_user(sysinfo::UpdateKind::OnlyIfNotSet),
        );

        // Disk
        self.disks.refresh(true);

        // Disk I/O rates from /proc/diskstats (Linux only)
        self.collect_disk_io();

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

    /// Handle keyboard input. Returns true if app should quit.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
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
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,

            // Help
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = !self.show_help,

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

            // Process navigation
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
            KeyCode::Tab | KeyCode::Char('s') => {
                self.sort_column = self.sort_column.next();
            }
            KeyCode::Char('r') => self.sort_descending = !self.sort_descending,

            // Filter
            KeyCode::Char('/' | 'f') => {
                self.show_filter_input = true;
            }
            KeyCode::Delete => self.filter.clear(),

            // Reset / Help
            KeyCode::Char('0') => {
                self.panels = PanelVisibility::default();
            }
            KeyCode::Char('h') => self.show_help = !self.show_help,

            _ => {}
        }

        false
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

    /// Get system uptime in seconds
    pub fn uptime(&self) -> u64 {
        System::uptime()
    }

    /// Collect disk I/O statistics from /proc/diskstats (Linux only)
    fn collect_disk_io(&mut self) {
        #[cfg(target_os = "linux")]
        {
            use std::fs;

            // Read /proc/diskstats
            // Format: major minor name reads_completed reads_merged sectors_read time_reading
            //         writes_completed writes_merged sectors_written time_writing ...
            // Sector size is typically 512 bytes
            const SECTOR_SIZE: u64 = 512;

            let Ok(content) = fs::read_to_string("/proc/diskstats") else {
                return;
            };

            let mut total_read_sectors: u64 = 0;
            let mut total_write_sectors: u64 = 0;

            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 14 {
                    continue;
                }

                let name = parts[2];
                // Skip partitions (e.g., sda1, nvme0n1p1) - only count whole disks
                // This avoids double-counting
                if name.chars().last().map_or(false, |c| c.is_ascii_digit()) {
                    // Check if it's a partition (e.g., sda1, nvme0n1p1)
                    // Partitions usually end with a number after device name
                    // Skip nvme partitions (contain 'p' followed by digits)
                    if name.contains('p')
                        && name
                            .chars()
                            .rev()
                            .take_while(|c| c.is_ascii_digit())
                            .count()
                            > 0
                    {
                        continue;
                    }
                    // Skip traditional partitions (sda1, sdb2, etc.)
                    if name.starts_with("sd") || name.starts_with("hd") {
                        continue;
                    }
                }

                // Skip loop devices and ram devices
                if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("dm-") {
                    continue;
                }

                // sectors_read is field 5 (0-indexed: 5), sectors_written is field 9
                let read_sectors: u64 = parts[5].parse().unwrap_or(0);
                let write_sectors: u64 = parts[9].parse().unwrap_or(0);

                total_read_sectors += read_sectors;
                total_write_sectors += write_sectors;
            }

            let total_read_bytes = total_read_sectors * SECTOR_SIZE;
            let total_write_bytes = total_write_sectors * SECTOR_SIZE;

            // Calculate rates (assume 1 second interval between refreshes)
            if self.prev_disk_read_bytes > 0 {
                self.disk_io_rates.read_bytes_per_sec =
                    total_read_bytes.saturating_sub(self.prev_disk_read_bytes) as f64;
                self.disk_io_rates.write_bytes_per_sec =
                    total_write_bytes.saturating_sub(self.prev_disk_write_bytes) as f64;
            }

            self.prev_disk_read_bytes = total_read_bytes;
            self.prev_disk_write_bytes = total_write_bytes;
        }

        #[cfg(not(target_os = "linux"))]
        {
            // No disk I/O stats on non-Linux platforms
        }
    }
}
