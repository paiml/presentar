//! `ProcessTable` widget for system process monitoring.
//!
//! Displays running processes with CPU/Memory usage in a ttop/btop style.
//! Reference: ttop/btop process displays.

use crate::theme::Gradient;
use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event, Key,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::fmt::Write as _;
use std::time::Duration;

/// Process state (from /proc/[pid]/stat)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessState {
    /// Running (R)
    Running,
    /// Sleeping (S)
    #[default]
    Sleeping,
    /// Disk sleep/waiting (D)
    DiskWait,
    /// Zombie (Z)
    Zombie,
    /// Stopped (T)
    Stopped,
    /// Idle (I)
    Idle,
}

impl ProcessState {
    /// Get the single-character representation
    #[must_use]
    pub fn char(&self) -> char {
        match self {
            Self::Running => 'R',
            Self::Sleeping => 'S',
            Self::DiskWait => 'D',
            Self::Zombie => 'Z',
            Self::Stopped => 'T',
            Self::Idle => 'I',
        }
    }

    /// Get the color for this state
    #[must_use]
    pub fn color(&self) -> Color {
        match self {
            Self::Running => Color::new(0.3, 0.9, 0.3, 1.0), // Green
            Self::Sleeping => Color::new(0.5, 0.5, 0.5, 1.0), // Gray
            Self::DiskWait => Color::new(1.0, 0.7, 0.2, 1.0), // Orange
            Self::Zombie => Color::new(1.0, 0.3, 0.3, 1.0),  // Red
            Self::Stopped => Color::new(0.9, 0.9, 0.3, 1.0), // Yellow
            Self::Idle => Color::new(0.4, 0.4, 0.4, 1.0),    // Dark gray
        }
    }
}

/// A running process entry.
#[derive(Debug, Clone)]
pub struct ProcessEntry {
    /// Process ID.
    pub pid: u32,
    /// User running the process.
    pub user: String,
    /// CPU usage percentage (0-100).
    pub cpu_percent: f32,
    /// Memory usage percentage (0-100).
    pub mem_percent: f32,
    /// Command name.
    pub command: String,
    /// Full command line (optional).
    pub cmdline: Option<String>,
    /// Process state.
    pub state: ProcessState,
    /// OOM score (0-1000, higher = more likely to be killed).
    pub oom_score: Option<i32>,
    /// cgroup path (short form).
    pub cgroup: Option<String>,
    /// Nice value (-20 to +19).
    pub nice: Option<i32>,
}

impl ProcessEntry {
    /// Create a new process entry.
    #[must_use]
    pub fn new(
        pid: u32,
        user: impl Into<String>,
        cpu: f32,
        mem: f32,
        command: impl Into<String>,
    ) -> Self {
        Self {
            pid,
            user: user.into(),
            cpu_percent: cpu,
            mem_percent: mem,
            command: command.into(),
            cmdline: None,
            state: ProcessState::default(),
            oom_score: None,
            cgroup: None,
            nice: None,
        }
    }

    /// Set full command line.
    #[must_use]
    pub fn with_cmdline(mut self, cmdline: impl Into<String>) -> Self {
        self.cmdline = Some(cmdline.into());
        self
    }

    /// Set process state.
    #[must_use]
    pub fn with_state(mut self, state: ProcessState) -> Self {
        self.state = state;
        self
    }

    /// Set OOM score (0-1000).
    #[must_use]
    pub fn with_oom_score(mut self, score: i32) -> Self {
        self.oom_score = Some(score);
        self
    }

    /// Set cgroup path.
    #[must_use]
    pub fn with_cgroup(mut self, cgroup: impl Into<String>) -> Self {
        self.cgroup = Some(cgroup.into());
        self
    }

    /// Set nice value.
    #[must_use]
    pub fn with_nice(mut self, nice: i32) -> Self {
        self.nice = Some(nice);
        self
    }
}

/// Sort column for process table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSort {
    Pid,
    User,
    Cpu,
    Memory,
    Command,
    Oom,
}

/// Process table widget with color-coded CPU/Memory bars.
#[derive(Debug, Clone)]
pub struct ProcessTable {
    /// Process entries.
    processes: Vec<ProcessEntry>,
    /// Selected row index.
    selected: usize,
    /// Scroll offset.
    scroll_offset: usize,
    /// Current sort column.
    sort_by: ProcessSort,
    /// Sort ascending.
    sort_ascending: bool,
    /// CPU gradient (low → high).
    cpu_gradient: Gradient,
    /// Memory gradient (low → high).
    mem_gradient: Gradient,
    /// Show command line instead of command name.
    show_cmdline: bool,
    /// Compact mode (fewer columns).
    compact: bool,
    /// Show OOM score column.
    show_oom: bool,
    /// Show nice value column.
    show_nice: bool,
    /// Cached bounds.
    bounds: Rect,
}

impl Default for ProcessTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessTable {
    /// Create a new process table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            sort_by: ProcessSort::Cpu,
            sort_ascending: false, // Default: highest CPU first
            cpu_gradient: Gradient::from_hex(&["#7aa2f7", "#e0af68", "#f7768e"]),
            mem_gradient: Gradient::from_hex(&["#9ece6a", "#e0af68", "#f7768e"]),
            show_cmdline: false,
            compact: false,
            show_oom: false,
            show_nice: false,
            bounds: Rect::default(),
        }
    }

    /// Set processes.
    pub fn set_processes(&mut self, processes: Vec<ProcessEntry>) {
        self.processes = processes;
        self.sort_processes();
        // Clamp selection
        if !self.processes.is_empty() && self.selected >= self.processes.len() {
            self.selected = self.processes.len() - 1;
        }
    }

    /// Add a process.
    pub fn add_process(&mut self, process: ProcessEntry) {
        self.processes.push(process);
    }

    /// Clear all processes.
    pub fn clear(&mut self) {
        self.processes.clear();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Set CPU gradient.
    #[must_use]
    pub fn with_cpu_gradient(mut self, gradient: Gradient) -> Self {
        self.cpu_gradient = gradient;
        self
    }

    /// Set memory gradient.
    #[must_use]
    pub fn with_mem_gradient(mut self, gradient: Gradient) -> Self {
        self.mem_gradient = gradient;
        self
    }

    /// Enable compact mode.
    #[must_use]
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Show full command line.
    #[must_use]
    pub fn with_cmdline(mut self) -> Self {
        self.show_cmdline = true;
        self
    }

    /// Show OOM score column.
    #[must_use]
    pub fn with_oom(mut self) -> Self {
        self.show_oom = true;
        self
    }

    /// Show nice value column.
    #[must_use]
    pub fn with_nice_column(mut self) -> Self {
        self.show_nice = true;
        self
    }

    /// Set sort column.
    pub fn sort_by(&mut self, column: ProcessSort) {
        if self.sort_by == column {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_by = column;
            // Default directions (CPU/Memory/OOM default to descending)
            self.sort_ascending = !matches!(
                column,
                ProcessSort::Cpu | ProcessSort::Memory | ProcessSort::Oom
            );
        }
        self.sort_processes();
    }

    /// Get current sort column.
    #[must_use]
    pub fn current_sort(&self) -> ProcessSort {
        self.sort_by
    }

    /// Get selected index.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Get selected process.
    #[must_use]
    pub fn selected_process(&self) -> Option<&ProcessEntry> {
        self.processes.get(self.selected)
    }

    /// Select a row.
    pub fn select(&mut self, row: usize) {
        if !self.processes.is_empty() {
            self.selected = row.min(self.processes.len() - 1);
            self.ensure_visible();
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if !self.processes.is_empty() && self.selected < self.processes.len() - 1 {
            self.selected += 1;
            self.ensure_visible();
        }
    }

    /// Get process count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.processes.len()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.processes.is_empty()
    }

    fn sort_processes(&mut self) {
        let ascending = self.sort_ascending;
        match self.sort_by {
            ProcessSort::Pid => {
                self.processes.sort_by(|a, b| {
                    if ascending {
                        a.pid.cmp(&b.pid)
                    } else {
                        b.pid.cmp(&a.pid)
                    }
                });
            }
            ProcessSort::User => {
                self.processes.sort_by(|a, b| {
                    if ascending {
                        a.user.cmp(&b.user)
                    } else {
                        b.user.cmp(&a.user)
                    }
                });
            }
            ProcessSort::Cpu => {
                self.processes.sort_by(|a, b| {
                    let cmp = a
                        .cpu_percent
                        .partial_cmp(&b.cpu_percent)
                        .unwrap_or(std::cmp::Ordering::Equal);
                    if ascending {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
            ProcessSort::Memory => {
                self.processes.sort_by(|a, b| {
                    let cmp = a
                        .mem_percent
                        .partial_cmp(&b.mem_percent)
                        .unwrap_or(std::cmp::Ordering::Equal);
                    if ascending {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
            ProcessSort::Command => {
                self.processes.sort_by(|a, b| {
                    if ascending {
                        a.command.cmp(&b.command)
                    } else {
                        b.command.cmp(&a.command)
                    }
                });
            }
            ProcessSort::Oom => {
                self.processes.sort_by(|a, b| {
                    let a_oom = a.oom_score.unwrap_or(0);
                    let b_oom = b.oom_score.unwrap_or(0);
                    if ascending {
                        a_oom.cmp(&b_oom)
                    } else {
                        b_oom.cmp(&a_oom)
                    }
                });
            }
        }
    }

    fn ensure_visible(&mut self) {
        let visible_rows = (self.bounds.height as usize).saturating_sub(2);
        if visible_rows == 0 {
            return;
        }

        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selected - visible_rows + 1;
        }
    }

    fn truncate(s: &str, width: usize) -> String {
        if s.len() <= width {
            format!("{s:width$}")
        } else if width > 3 {
            format!("{}...", &s[..width - 3])
        } else {
            s[..width.min(s.len())].to_string()
        }
    }
}

impl Brick for ProcessTable {
    fn brick_name(&self) -> &'static str {
        "process_table"
    }

    fn assertions(&self) -> &[BrickAssertion] {
        static ASSERTIONS: &[BrickAssertion] = &[BrickAssertion::max_latency_ms(16)];
        ASSERTIONS
    }

    fn budget(&self) -> BrickBudget {
        BrickBudget::uniform(16)
    }

    fn verify(&self) -> BrickVerification {
        let passed = if self.processes.is_empty() || self.selected < self.processes.len() {
            vec![BrickAssertion::max_latency_ms(16)]
        } else {
            vec![]
        };
        let failed = if !self.processes.is_empty() && self.selected >= self.processes.len() {
            vec![(
                BrickAssertion::max_latency_ms(16),
                format!(
                    "Selected {} >= process count {}",
                    self.selected,
                    self.processes.len()
                ),
            )]
        } else {
            vec![]
        };

        BrickVerification {
            passed,
            failed,
            verification_time: Duration::from_micros(10),
        }
    }

    fn to_html(&self) -> String {
        String::new()
    }

    fn to_css(&self) -> String {
        String::new()
    }
}

impl Widget for ProcessTable {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let min_width = if self.compact { 40.0 } else { 60.0 };
        let width = constraints.max_width.max(min_width);
        let height = (self.processes.len() + 2).min(30) as f32;
        constraints.constrain(Size::new(width, height.max(3.0)))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        self.ensure_visible();
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&self, canvas: &mut dyn Canvas) {
        let width = self.bounds.width as usize;
        let height = self.bounds.height as usize;
        if width == 0 || height == 0 {
            return;
        }

        // Column layout - compact mode: PID S [OOM] [NI] C% M% COMMAND
        let pid_w = 6;
        let state_w = if self.compact { 2 } else { 0 }; // State column in compact mode
        let oom_w = if self.show_oom { 4 } else { 0 }; // OOM column (3 digits + space)
        let nice_w = if self.show_nice { 4 } else { 0 }; // NI column (3 chars + space)
        let user_w = if self.compact { 0 } else { 8 };
        let cpu_w = 6;
        let mem_w = 6;
        let sep_w = if self.compact { 1 } else { 3 };
        let extra_cols = usize::from(self.show_oom) + usize::from(self.show_nice);
        let num_seps = if self.compact { 3 } else { 4 } + extra_cols;
        let fixed_w = pid_w + state_w + oom_w + nice_w + user_w + cpu_w + mem_w + sep_w * num_seps;
        let cmd_w = width.saturating_sub(fixed_w);

        // Header style
        let header_style = TextStyle {
            color: Color::new(0.0, 1.0, 1.0, 1.0),
            weight: presentar_core::FontWeight::Bold,
            ..Default::default()
        };

        // Draw header - ttop compact format: PID S [OOM] C% M% COMMAND
        let mut header = String::new();
        let _ = write!(header, "{:>pid_w$}", "PID");
        if self.compact {
            header.push(' ');
            let _ = write!(header, "{:>1}", "S");
        } else {
            header.push_str(" │ ");
            let _ = write!(header, "{:user_w$}", "USER");
        }
        if self.show_oom {
            header.push_str(if self.compact { " " } else { " │ " });
            let _ = write!(header, "{:>3}", "OOM");
        }
        if self.show_nice {
            header.push_str(if self.compact { " " } else { " │ " });
            let _ = write!(header, "{:>3}", "NI");
        }
        header.push_str(if self.compact { " " } else { " │ " });
        let _ = write!(
            header,
            "{:>cpu_w$}",
            if self.compact { "C%" } else { "CPU%" }
        );
        header.push_str(if self.compact { " " } else { " │ " });
        let _ = write!(
            header,
            "{:>mem_w$}",
            if self.compact { "M%" } else { "MEM%" }
        );
        header.push_str(if self.compact { " " } else { " │ " });
        let _ = write!(header, "{:cmd_w$}", "COMMAND");

        canvas.draw_text(
            &header,
            Point::new(self.bounds.x, self.bounds.y),
            &header_style,
        );

        // Draw separator
        if height > 1 {
            let sep: String = "─".repeat(width);
            canvas.draw_text(
                &sep,
                Point::new(self.bounds.x, self.bounds.y + 1.0),
                &TextStyle {
                    color: Color::new(0.3, 0.3, 0.4, 1.0),
                    ..Default::default()
                },
            );
        }

        // Draw rows
        let visible_rows = height.saturating_sub(2);
        let default_style = TextStyle {
            color: Color::new(0.8, 0.8, 0.8, 1.0),
            ..Default::default()
        };

        for (i, proc_idx) in (self.scroll_offset..self.processes.len())
            .take(visible_rows)
            .enumerate()
        {
            let proc = &self.processes[proc_idx];
            let y = self.bounds.y + 2.0 + i as f32;
            let is_selected = proc_idx == self.selected;

            // Selection background
            if is_selected {
                canvas.fill_rect(
                    Rect::new(self.bounds.x, y, self.bounds.width, 1.0),
                    Color::new(0.2, 0.2, 0.4, 0.5),
                );
            }

            let mut x = self.bounds.x;

            // PID
            let pid_str = format!("{:>pid_w$}", proc.pid);
            canvas.draw_text(&pid_str, Point::new(x, y), &default_style);
            x += pid_w as f32;

            // State (compact mode) or User (full mode)
            if self.compact {
                x += 1.0; // separator
                let state_char = proc.state.char().to_string();
                canvas.draw_text(
                    &state_char,
                    Point::new(x, y),
                    &TextStyle {
                        color: proc.state.color(),
                        ..Default::default()
                    },
                );
                x += 1.0;
            } else {
                x += 3.0; // separator
                let user_str = Self::truncate(&proc.user, user_w);
                canvas.draw_text(&user_str, Point::new(x, y), &default_style);
                x += user_w as f32;
            }

            // OOM score (if enabled)
            if self.show_oom {
                x += if self.compact { 1.0 } else { 3.0 };
                let oom = proc.oom_score.unwrap_or(0);
                // Color based on OOM risk: green < 200, yellow 200-500, red > 500
                let oom_color = if oom > 500 {
                    Color::new(1.0, 0.3, 0.3, 1.0) // Red - high risk
                } else if oom > 200 {
                    Color::new(1.0, 0.8, 0.2, 1.0) // Yellow - medium risk
                } else {
                    Color::new(0.5, 0.8, 0.5, 1.0) // Green - low risk
                };
                let oom_str = format!("{oom:>3}");
                canvas.draw_text(
                    &oom_str,
                    Point::new(x, y),
                    &TextStyle {
                        color: oom_color,
                        ..Default::default()
                    },
                );
                x += 3.0;
            }

            // Nice value (if enabled)
            if self.show_nice {
                x += if self.compact { 1.0 } else { 3.0 };
                let ni = proc.nice.unwrap_or(0);
                // Color: negative nice (high priority) = cyan, positive (low priority) = gray
                let ni_color = if ni < 0 {
                    Color::new(0.3, 0.9, 0.9, 1.0) // Cyan - high priority
                } else if ni > 0 {
                    Color::new(0.6, 0.6, 0.6, 1.0) // Gray - low priority
                } else {
                    Color::new(0.8, 0.8, 0.8, 1.0) // White - normal
                };
                let ni_str = format!("{ni:>3}");
                canvas.draw_text(
                    &ni_str,
                    Point::new(x, y),
                    &TextStyle {
                        color: ni_color,
                        ..Default::default()
                    },
                );
                x += 3.0;
            }

            // CPU
            x += if self.compact { 1.0 } else { 3.0 };
            let cpu_color = self.cpu_gradient.for_percent(proc.cpu_percent as f64);
            let cpu_str = format!("{:>5.1}%", proc.cpu_percent);
            canvas.draw_text(
                &cpu_str,
                Point::new(x, y),
                &TextStyle {
                    color: cpu_color,
                    ..Default::default()
                },
            );
            x += cpu_w as f32;

            // Memory
            x += if self.compact { 1.0 } else { 3.0 };
            let mem_color = self.mem_gradient.for_percent(proc.mem_percent as f64);
            let mem_str = format!("{:>5.1}%", proc.mem_percent);
            canvas.draw_text(
                &mem_str,
                Point::new(x, y),
                &TextStyle {
                    color: mem_color,
                    ..Default::default()
                },
            );
            x += mem_w as f32;

            // Command
            x += if self.compact { 1.0 } else { 3.0 };
            let cmd = if self.show_cmdline {
                proc.cmdline.as_deref().unwrap_or(&proc.command)
            } else {
                &proc.command
            };
            let cmd_str = Self::truncate(cmd, cmd_w);
            let cmd_style = if is_selected {
                TextStyle {
                    color: Color::new(1.0, 1.0, 1.0, 1.0),
                    ..Default::default()
                }
            } else {
                default_style.clone()
            };
            canvas.draw_text(&cmd_str, Point::new(x, y), &cmd_style);
        }

        // Show count in empty state
        if self.processes.is_empty() && height > 2 {
            canvas.draw_text(
                "No processes",
                Point::new(self.bounds.x + 1.0, self.bounds.y + 2.0),
                &TextStyle {
                    color: Color::new(0.5, 0.5, 0.5, 1.0),
                    ..Default::default()
                },
            );
        }
    }

    fn event(&mut self, event: &Event) -> Option<Box<dyn Any + Send>> {
        match event {
            Event::KeyDown { key } => {
                match key {
                    Key::Up | Key::K => self.select_prev(),
                    Key::Down | Key::J => self.select_next(),
                    Key::C => self.sort_by(ProcessSort::Cpu),
                    Key::M => self.sort_by(ProcessSort::Memory),
                    Key::P => self.sort_by(ProcessSort::Pid),
                    Key::N => self.sort_by(ProcessSort::Command),
                    Key::O => self.sort_by(ProcessSort::Oom),
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_processes() -> Vec<ProcessEntry> {
        vec![
            ProcessEntry::new(1, "root", 0.5, 0.1, "systemd"),
            ProcessEntry::new(1234, "noah", 25.0, 5.5, "firefox"),
            ProcessEntry::new(5678, "noah", 80.0, 12.3, "rustc"),
        ]
    }

    #[test]
    fn test_process_table_new() {
        let table = ProcessTable::new();
        assert!(table.is_empty());
    }

    #[test]
    fn test_process_table_set_processes() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn test_process_table_add_process() {
        let mut table = ProcessTable::new();
        table.add_process(ProcessEntry::new(1, "root", 0.0, 0.0, "init"));
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_process_table_clear() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.select(1);
        table.clear();
        assert!(table.is_empty());
        assert_eq!(table.selected(), 0);
    }

    #[test]
    fn test_process_table_selection() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        assert_eq!(table.selected(), 0);

        table.select_next();
        assert_eq!(table.selected(), 1);

        table.select_prev();
        assert_eq!(table.selected(), 0);
    }

    #[test]
    fn test_process_table_select_bounds() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());

        table.select(100);
        assert_eq!(table.selected(), 2);

        table.select_prev();
        table.select_prev();
        table.select_prev();
        assert_eq!(table.selected(), 0);
    }

    #[test]
    fn test_process_table_sort_cpu() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        // Default sort is CPU descending
        assert_eq!(table.processes[0].command, "rustc");
    }

    #[test]
    fn test_process_table_sort_toggle() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.sort_by(ProcessSort::Cpu); // Toggle to ascending
        assert_eq!(table.processes[0].command, "systemd");
    }

    #[test]
    fn test_process_table_sort_pid() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.sort_by(ProcessSort::Pid);
        assert_eq!(table.processes[0].pid, 1);
    }

    #[test]
    fn test_process_table_sort_memory() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.sort_by(ProcessSort::Memory);
        assert_eq!(table.processes[0].command, "rustc");
    }

    #[test]
    fn test_process_table_selected_process() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        let proc = table.selected_process().unwrap();
        assert_eq!(proc.command, "rustc"); // Highest CPU
    }

    #[test]
    fn test_process_entry_with_cmdline() {
        let proc = ProcessEntry::new(1, "root", 0.0, 0.0, "bash").with_cmdline("/bin/bash --login");
        assert_eq!(proc.cmdline.as_deref(), Some("/bin/bash --login"));
    }

    #[test]
    fn test_process_table_compact() {
        let table = ProcessTable::new().compact();
        assert!(table.compact);
    }

    #[test]
    fn test_process_table_with_cmdline() {
        let table = ProcessTable::new().with_cmdline();
        assert!(table.show_cmdline);
    }

    #[test]
    fn test_process_table_verify() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        assert!(table.verify().is_valid());
    }

    #[test]
    fn test_process_table_verify_invalid() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.selected = 100;
        assert!(!table.verify().is_valid());
    }

    #[test]
    fn test_process_table_measure() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        let size = table.measure(Constraints::new(0.0, 100.0, 0.0, 50.0));
        assert!(size.width >= 60.0);
        assert!(size.height >= 3.0);
    }

    #[test]
    fn test_process_table_layout() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        let result = table.layout(Rect::new(0.0, 0.0, 80.0, 20.0));
        assert_eq!(result.size.width, 80.0);
    }

    #[test]
    fn test_process_table_event_keys() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());

        table.event(&Event::KeyDown { key: Key::J });
        assert_eq!(table.selected(), 1);

        table.event(&Event::KeyDown { key: Key::K });
        assert_eq!(table.selected(), 0);

        table.event(&Event::KeyDown { key: Key::P });
        assert_eq!(table.current_sort(), ProcessSort::Pid);
    }

    #[test]
    fn test_process_table_brick_name() {
        let table = ProcessTable::new();
        assert_eq!(table.brick_name(), "process_table");
    }

    #[test]
    fn test_process_table_default() {
        let table = ProcessTable::default();
        assert!(table.is_empty());
    }

    #[test]
    fn test_process_table_children() {
        let table = ProcessTable::new();
        assert!(table.children().is_empty());
    }

    #[test]
    fn test_process_table_children_mut() {
        let mut table = ProcessTable::new();
        assert!(table.children_mut().is_empty());
    }

    #[test]
    fn test_process_table_truncate() {
        assert_eq!(ProcessTable::truncate("hello", 10), "hello     ");
        assert_eq!(ProcessTable::truncate("hello world", 8), "hello...");
        assert_eq!(ProcessTable::truncate("hi", 2), "hi");
    }

    #[test]
    fn test_process_table_type_id() {
        let table = ProcessTable::new();
        assert_eq!(Widget::type_id(&table), TypeId::of::<ProcessTable>());
    }

    #[test]
    fn test_process_table_to_html() {
        let table = ProcessTable::new();
        assert!(table.to_html().is_empty());
    }

    #[test]
    fn test_process_table_to_css() {
        let table = ProcessTable::new();
        assert!(table.to_css().is_empty());
    }

    #[test]
    fn test_process_table_sort_command() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.sort_by(ProcessSort::Command);
        assert_eq!(table.processes[0].command, "firefox");
    }

    #[test]
    fn test_process_table_sort_user() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.sort_by(ProcessSort::User);
        assert_eq!(table.processes[0].user, "noah");
    }

    #[test]
    fn test_process_table_sort_oom() {
        let mut table = ProcessTable::new();
        // Create processes with different OOM scores
        let entries = vec![
            ProcessEntry::new(1, "user", 10.0, 5.0, "low_oom").with_oom_score(100),
            ProcessEntry::new(2, "user", 10.0, 5.0, "high_oom").with_oom_score(800),
            ProcessEntry::new(3, "user", 10.0, 5.0, "med_oom").with_oom_score(400),
        ];
        table.set_processes(entries);

        // Sort by OOM (default descending - highest first)
        table.sort_by(ProcessSort::Oom);

        // Verify order: high (800) -> med (400) -> low (100)
        assert_eq!(table.processes[0].command, "high_oom");
        assert_eq!(table.processes[1].command, "med_oom");
        assert_eq!(table.processes[2].command, "low_oom");
    }

    #[test]
    fn test_process_table_sort_oom_toggle_ascending() {
        let mut table = ProcessTable::new();
        let entries = vec![
            ProcessEntry::new(1, "user", 10.0, 5.0, "low_oom").with_oom_score(100),
            ProcessEntry::new(2, "user", 10.0, 5.0, "high_oom").with_oom_score(800),
        ];
        table.set_processes(entries);

        // Sort by OOM twice to toggle to ascending
        table.sort_by(ProcessSort::Oom);
        table.sort_by(ProcessSort::Oom);

        // Verify order is now ascending: low (100) -> high (800)
        assert_eq!(table.processes[0].command, "low_oom");
        assert_eq!(table.processes[1].command, "high_oom");
    }
}
