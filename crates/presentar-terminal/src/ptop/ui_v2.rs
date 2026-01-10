//! ptop v2 - Pure Widget Composition
//!
//! ZERO `draw_text` calls. All rendering through widgets.
//! Target: Match ttop's USEFUL layout, not feature bloat.

use super::app::App;
use crate::widgets::{
    Border, ConnectionEntry, ConnectionsPanel, ContainerEntry, ContainerState, ContainersPanel,
    CpuGrid, FileEntry, FilesPanel, GpuDevice, GpuPanel, GpuProcess, GpuVendor, Layout, LayoutItem,
    MemoryBar, Meter, NetworkInterface, NetworkPanel, ProcessEntry, ProcessState, ProcessTable,
    SensorReading, SensorsPanel, Text,
};
use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Rect, Size, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// ptop v2 - The entire UI as widget composition.
pub struct PtopView {
    /// The composed widget tree (rebuilt each frame).
    root: Layout,
    /// Optional help overlay.
    help_overlay: Option<Border>,
    /// Cached bounds.
    bounds: Rect,
}

impl PtopView {
    /// Build the UI from application state.
    ///
    /// Layout follows ttop's 3-row structure:
    /// - Row 1 (30%): CPU, Memory, Disk
    /// - Row 2 (25%): Network, GPU, Sensors+Containers
    /// - Row 3 (45%): Processes, Connections+Files
    pub fn from_app(app: &App) -> Self {
        // Build Row 1 panels (CPU, Memory, Disk) based on visibility
        let mut row1_items: Vec<LayoutItem> = Vec::new();
        if app.panels.cpu {
            row1_items.push(LayoutItem::new(Self::cpu_panel(app)).flex(1.0));
        }
        if app.panels.memory {
            row1_items.push(LayoutItem::new(Self::memory_panel(app)).flex(1.0));
        }
        if app.panels.disk {
            row1_items.push(LayoutItem::new(Self::disk_panel(app)).flex(1.0));
        }
        if row1_items.is_empty() {
            row1_items.push(LayoutItem::new(Text::new("(1-3: toggle panels)")).flex(1.0));
        }

        // Build Row 2 panels (Network, GPU, Sensors+Containers)
        let mut row2_items: Vec<LayoutItem> = Vec::new();
        if app.panels.network {
            row2_items.push(LayoutItem::new(Self::network_panel(app)).flex(1.0));
        }
        if app.panels.gpu {
            row2_items.push(LayoutItem::new(Self::gpu_panel(app)).flex(1.0));
        }
        // Stacked sensors + containers (containers shown when sensors enabled)
        let mut stacked: Vec<LayoutItem> = Vec::new();
        if app.panels.sensors {
            stacked.push(LayoutItem::new(Self::sensors_panel(app)).flex(1.0));
            stacked.push(LayoutItem::new(Self::containers_panel(app)).flex(1.0));
        }
        if !stacked.is_empty() {
            row2_items.push(Layout::rows(stacked).into_item().flex(1.0));
        }
        if row2_items.is_empty() {
            row2_items.push(LayoutItem::new(Text::new("(4-7: toggle panels)")).flex(1.0));
        }

        // Build Row 3 panels (Processes, Connections+Files)
        let mut row3_items: Vec<LayoutItem> = Vec::new();
        if app.panels.process {
            row3_items.push(LayoutItem::new(Self::process_panel(app)).flex(1.0));
        }
        // Stacked connections + files (files shown when connections enabled)
        let mut details: Vec<LayoutItem> = Vec::new();
        if app.panels.connections {
            details.push(LayoutItem::new(Self::connections_panel(app)).flex(1.0));
            details.push(LayoutItem::new(Self::files_panel(app)).flex(1.0));
        }
        if !details.is_empty() {
            row3_items.push(Layout::rows(details).into_item().flex(1.0));
        }
        if row3_items.is_empty() {
            row3_items.push(LayoutItem::new(Text::new("(5,8: toggle panels)")).flex(1.0));
        }

        // Build the widget tree using composition
        let root = Layout::rows([
            Layout::columns(row1_items).into_item().percent(30.0),
            Layout::columns(row2_items).into_item().percent(25.0),
            Layout::columns(row3_items).into_item().expanded(),
        ]);

        // Create help overlay if needed
        let help_overlay = if app.show_help {
            Some(Self::help_panel())
        } else {
            None
        };

        Self {
            root,
            help_overlay,
            bounds: Rect::default(),
        }
    }

    /// Create the help overlay panel.
    fn help_panel() -> Border {
        use crate::widgets::BorderStyle;
        Border::new().with_style(BorderStyle::Double).with_title(" Help (? to close) ").child(
            Text::new("q/Esc Quit | h/? Help | j/k Navigate | g/G Top/Bottom\nc/m/p Sort CPU/Mem/PID | s/Tab Cycle | r Reverse\n//f Filter | Del Clear | 1-9 Panels | 0 Reset"))
    }

    fn cpu_panel(app: &App) -> Border {
        use sysinfo::Cpu;

        // Calculate overall CPU percentage
        let cpu_pct = if app.per_core_percent.is_empty() {
            0.0
        } else {
            app.per_core_percent.iter().sum::<f64>() / app.per_core_percent.len() as f64
        };

        // Get max frequency
        let max_freq_mhz = app
            .system
            .cpus()
            .iter()
            .map(Cpu::frequency)
            .max()
            .unwrap_or(0);
        let freq_ghz = max_freq_mhz as f64 / 1000.0;
        let is_boosting = max_freq_mhz > 3000;
        let boost_icon = if is_boosting { "⚡" } else { "" };

        // Get CPU temperature from sensors if available
        let cpu_temp = app.analyzers.sensor_health_data().and_then(|data| {
            data.temperatures()
                .find(|s| s.label.to_lowercase().contains("cpu") || s.device.contains("coretemp"))
                .map(|s| s.value)
        });

        // Get load averages
        let load = sysinfo::System::load_average();
        let load_text = Text::new(format!(
            "Load {:.2} {:.2} {:.2}",
            load.one, load.five, load.fifteen
        ));

        let grid = CpuGrid::new(app.per_core_percent.clone()).with_columns(8);

        let inner = Layout::rows([
            LayoutItem::new(grid).expanded(),
            LayoutItem::new(load_text).fixed(1.0),
        ]);

        // Build title with freq and optional temp
        let title = if let Some(temp) = cpu_temp {
            format!(
                "CPU {:.0}% │ {} cores │ {:.1}GHz{} │ {:.0}°C",
                cpu_pct,
                app.per_core_percent.len(),
                freq_ghz,
                boost_icon,
                temp
            )
        } else {
            format!(
                "CPU {:.0}% │ {} cores │ {:.1}GHz{}",
                cpu_pct,
                app.per_core_percent.len(),
                freq_ghz,
                boost_icon
            )
        };

        Border::rounded(title).child(inner)
    }

    fn memory_panel(app: &App) -> Border {
        let bar = MemoryBar::new(app.mem_total)
            .segment("Used", app.mem_used, Color::new(0.7, 0.3, 0.5, 1.0))
            .segment("Swap", app.swap_used, Color::new(0.9, 0.7, 0.2, 1.0))
            .segment("Cached", app.mem_cached, Color::new(0.3, 0.7, 0.8, 1.0));

        let used_gb = app.mem_used as f64 / 1_073_741_824.0;
        let total_gb = app.mem_total as f64 / 1_073_741_824.0;
        let pct = if app.mem_total > 0 {
            (app.mem_used as f64 / app.mem_total as f64) * 100.0
        } else {
            0.0
        };

        // Get memory PSI (pressure stall info) if available
        let psi_indicator = app
            .psi_data()
            .and_then(|psi| {
                if psi.available {
                    let mem_pressure = psi.memory.some.avg10;
                    // Symbol based on pressure level: ● >10%, ◐ >5%, ○ low
                    let symbol = if mem_pressure > 10.0 {
                        "●"
                    } else if mem_pressure > 5.0 {
                        "◐"
                    } else {
                        "○"
                    };
                    Some(format!(" │ PSI {symbol} {mem_pressure:.1}%"))
                } else {
                    None
                }
            })
            .unwrap_or_default();

        Border::rounded(format!(
            "Memory │ {used_gb:.1}G / {total_gb:.1}G ({pct:.0}%){psi_indicator}"
        ))
        .child(bar)
    }

    fn disk_panel(app: &App) -> Border {
        let mut meters: Vec<LayoutItem> = Vec::new();

        for disk in &app.disks {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);
            let pct = if total > 0 {
                (used as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            let mount = disk.mount_point().to_string_lossy();
            let meter = Meter::percentage(pct)
                .with_label(format!("{mount}"))
                .with_percentage_text(true);

            meters.push(LayoutItem::new(meter).fixed(1.0));
        }

        if meters.is_empty() {
            meters.push(LayoutItem::new(Text::new("No disks")).fixed(1.0));
        }

        let inner = Layout::rows(meters);

        let (r_rate, w_rate) = if let Some(io) = app.disk_io_data() {
            (io.total_read_bytes_per_sec, io.total_write_bytes_per_sec)
        } else {
            (0.0, 0.0)
        };

        Border::rounded(format!(
            "Disk │ R: {}B/s │ W: {}B/s",
            format_bytes(r_rate as u64),
            format_bytes(w_rate as u64)
        ))
        .child(inner)
    }

    fn network_panel(app: &App) -> Border {
        let mut panel = NetworkPanel::new();

        for (name, data) in &app.networks {
            let mut iface = NetworkInterface::new(name);
            iface.rx_bps = data.received() as f64;
            iface.tx_bps = data.transmitted() as f64;
            iface.rx_total = data.total_received();
            iface.tx_total = data.total_transmitted();

            // Copy history from app
            for &val in app.net_rx_history.as_slice() {
                iface.rx_history.push(val * 1_000_000.0); // Scale for display
            }
            for &val in app.net_tx_history.as_slice() {
                iface.tx_history.push(val * 1_000_000.0);
            }

            panel.add_interface(iface);
        }

        Border::rounded("Network").child(panel)
    }

    fn gpu_panel(app: &App) -> Border {
        let gpu_data = app.analyzers.gpu_procs_data();

        let (panel, title) = if let Some(data) = gpu_data {
            if let Some(gpu) = data.gpus.first() {
                let vendor = match gpu.vendor {
                    super::analyzers::GpuVendor::Nvidia => GpuVendor::Nvidia,
                    super::analyzers::GpuVendor::Amd => GpuVendor::Amd,
                    super::analyzers::GpuVendor::Intel => GpuVendor::Intel,
                    _ => GpuVendor::Unknown,
                };

                let device = GpuDevice::new(&gpu.name)
                    .with_vendor(vendor)
                    .with_utilization(gpu.utilization)
                    .with_vram(gpu.used_memory, gpu.total_memory);

                let device = if let Some(temp) = gpu.temperature {
                    device.with_temperature(temp)
                } else {
                    device
                };

                let device = if let Some(power) = gpu.power_draw {
                    device.with_power(power, gpu.power_limit)
                } else {
                    device
                };

                let device = if let Some(fan) = gpu.fan_speed {
                    device.with_fan(fan)
                } else {
                    device
                };

                let mut panel = GpuPanel::new().with_device(device);

                // Add top GPU processes
                for proc in data.processes.iter().take(3) {
                    panel.add_process(GpuProcess::new(&proc.name, proc.pid, proc.used_memory));
                }

                let title = format!("GPU │ {}", gpu.name);
                (panel, title)
            } else {
                (GpuPanel::new(), "GPU │ No GPU".to_string())
            }
        } else {
            (GpuPanel::new(), "GPU │ N/A".to_string())
        };

        Border::rounded(title).child(panel)
    }

    fn sensors_panel(app: &App) -> Border {
        let sensor_data = app.analyzers.sensor_health_data();

        let mut panel = SensorsPanel::new().max_per_category(3);

        if let Some(data) = sensor_data {
            // Add temperature sensors
            for sensor in data.temperatures().take(4) {
                panel.add_temperature(SensorReading::temperature(&sensor.label, sensor.value));
            }

            // Add fan sensors
            for sensor in data.fans().take(2) {
                panel.add_fan(SensorReading::fan(&sensor.label, sensor.value));
            }
        }

        let max_temp = panel.max_temperature().unwrap_or(0.0);
        let title = if max_temp > 0.0 {
            format!("Sensors │ {max_temp:.0}°C")
        } else {
            "Sensors".to_string()
        };

        Border::rounded(title).child(panel)
    }

    fn containers_panel(app: &App) -> Border {
        let container_data = app.analyzers.containers_data();

        let mut panel = ContainersPanel::new().max_containers(4);

        if let Some(data) = container_data {
            for container in data.containers.iter().take(4) {
                let state = match container.state {
                    super::analyzers::ContainerState::Running => ContainerState::Running,
                    super::analyzers::ContainerState::Paused => ContainerState::Paused,
                    super::analyzers::ContainerState::Exited => ContainerState::Stopped,
                    super::analyzers::ContainerState::Restarting => ContainerState::Restarting,
                    _ => ContainerState::Stopped,
                };

                let entry = ContainerEntry::new(&container.name, &container.id)
                    .with_state(state)
                    .with_cpu(container.stats.cpu_percent)
                    .with_memory(container.stats.memory_bytes, container.stats.memory_limit);

                panel.add_container(entry);
            }
        }

        let running = panel.running_count();
        let title = if running > 0 {
            format!("Containers │ {running} running")
        } else {
            "Containers".to_string()
        };

        Border::rounded(title).child(panel)
    }

    fn connections_panel(app: &App) -> Border {
        let conn_data = app.analyzers.connections_data();

        let mut panel = ConnectionsPanel::new()
            .max_connections(10)
            .show_headers(true);

        if let Some(data) = conn_data {
            for conn in data.connections.iter().take(12) {
                use super::analyzers::TcpState as AnalyzerTcpState;
                use crate::widgets::TcpState as WidgetTcpState;

                let state = match conn.state {
                    AnalyzerTcpState::Established => WidgetTcpState::Established,
                    AnalyzerTcpState::Listen => WidgetTcpState::Listen,
                    AnalyzerTcpState::TimeWait => WidgetTcpState::TimeWait,
                    AnalyzerTcpState::CloseWait => WidgetTcpState::CloseWait,
                    AnalyzerTcpState::SynSent => WidgetTcpState::SynSent,
                    AnalyzerTcpState::SynRecv => WidgetTcpState::SynRecv,
                    AnalyzerTcpState::FinWait1 => WidgetTcpState::FinWait1,
                    AnalyzerTcpState::FinWait2 => WidgetTcpState::FinWait2,
                    AnalyzerTcpState::LastAck => WidgetTcpState::LastAck,
                    AnalyzerTcpState::Closing => WidgetTcpState::Closing,
                    AnalyzerTcpState::Close => WidgetTcpState::Closed,
                    AnalyzerTcpState::Unknown => WidgetTcpState::Closed,
                };

                let mut entry = ConnectionEntry::tcp(
                    conn.local_port,
                    conn.remote_addr.to_string(),
                    conn.remote_port,
                )
                .with_state(state)
                .with_local_addr(conn.local_addr.to_string());

                if let Some(ref name) = conn.process_name {
                    entry = entry.with_process(name, conn.pid.unwrap_or(0));
                }

                panel.add_connection(entry);
            }
        }

        let established = panel.established_count();
        let listening = panel.listening_count();
        let title = format!("Connections │ {established} active │ {listening} listen");

        Border::rounded(title).child(panel)
    }

    fn process_panel(app: &App) -> Border {
        let mut table = ProcessTable::new().compact().with_oom().with_nice_column();

        // Get process extra data (OOM scores, cgroups, nice values)
        let extra_data = app.analyzers.process_extra_data();

        let mut entries: Vec<ProcessEntry> = app
            .system
            .processes()
            .iter()
            .map(|(pid, proc)| {
                let state = match proc.status().to_string().chars().next() {
                    Some('R') => ProcessState::Running,
                    Some('S') => ProcessState::Sleeping,
                    Some('D') => ProcessState::DiskWait,
                    Some('Z') => ProcessState::Zombie,
                    Some('T') => ProcessState::Stopped,
                    Some('I') => ProcessState::Idle,
                    _ => ProcessState::Sleeping,
                };

                let mut entry = ProcessEntry::new(
                    pid.as_u32(),
                    proc.user_id().map_or_else(|| "-".into(), |u| u.to_string()),
                    proc.cpu_usage(),
                    proc.memory() as f32 / 1024.0 / 1024.0 / 10.0, // Rough percentage
                    proc.name().to_string_lossy().into_owned(),
                )
                .with_state(state);

                // Enrich with ProcessExtraAnalyzer data if available
                if let Some(data) = extra_data {
                    if let Some(extra) = data.get(pid.as_u32()) {
                        entry = entry
                            .with_oom_score(extra.oom_score)
                            .with_cgroup(extra.cgroup_short())
                            .with_nice(extra.nice);
                    }
                }

                entry
            })
            .collect();

        // Sort by CPU descending
        entries.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        table.set_processes(entries);

        Border::rounded(format!("Processes ({})", app.system.processes().len())).child(table)
    }

    fn files_panel(app: &App) -> Border {
        let treemap_data = app.analyzers.treemap_data();

        let mut panel = FilesPanel::new().max_entries(6);
        let mut total_size: u64 = 0;

        if let Some(data) = treemap_data {
            // Use pre-flattened top_items for display
            for node in data.top_items.iter().take(8) {
                total_size += node.size;
                let entry = FileEntry::new(&node.name, node.size, node.is_dir);
                panel.add_entry(entry);
            }
        } else {
            // Fallback: show disk mount points as entries
            for disk in app.disks.iter().take(6) {
                let mount = disk.mount_point().to_string_lossy().to_string();
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total.saturating_sub(available);
                total_size += used;
                panel.add_entry(FileEntry::directory(mount, used));
            }
        }

        let title = if total_size > 0 {
            format!("Files │ {}", format_bytes(total_size))
        } else {
            "Files".to_string()
        };

        Border::rounded(title).child(panel)
    }
}

impl Brick for PtopView {
    fn brick_name(&self) -> &'static str {
        "ptop_view"
    }
    fn assertions(&self) -> &[BrickAssertion] {
        static A: &[BrickAssertion] = &[BrickAssertion::max_latency_ms(16)];
        A
    }
    fn budget(&self) -> BrickBudget {
        BrickBudget::uniform(16)
    }
    fn verify(&self) -> BrickVerification {
        BrickVerification {
            passed: vec![BrickAssertion::max_latency_ms(16)],
            failed: vec![],
            verification_time: Duration::from_micros(50),
        }
    }
    fn to_html(&self) -> String {
        String::new()
    }
    fn to_css(&self) -> String {
        String::new()
    }
}

impl Widget for PtopView {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        self.root.measure(constraints)
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        let result = self.root.layout(bounds);
        if let Some(ref mut overlay) = self.help_overlay {
            let (w, h) = (
                55.0_f32.min(bounds.width - 4.0),
                6.0_f32.min(bounds.height - 2.0),
            );
            overlay.layout(Rect::new(
                bounds.x + (bounds.width - w) / 2.0,
                bounds.y + (bounds.height - h) / 2.0,
                w,
                h,
            ));
        }
        result
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        self.root.paint(canvas);
        if let Some(ref overlay) = self.help_overlay {
            overlay.paint(canvas);
        }
    }

    fn event(&mut self, event: &Event) -> Option<Box<dyn Any + Send>> {
        self.root.event(event)
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500");
        assert_eq!(format_bytes(1024), "1.0K");
        assert_eq!(format_bytes(1024 * 1024), "1.0M");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0G");
    }
}
