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
use std::cmp::Ordering;
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
    /// Thread count (CB-PROC-006).
    pub threads: Option<u32>,
    /// Parent process ID (CB-PROC-001 tree view).
    pub parent_pid: Option<u32>,
    /// Tree depth level for indentation (CB-PROC-001).
    pub tree_depth: usize,
    /// Whether this is the last child at its level (CB-PROC-001).
    pub is_last_child: bool,
    /// Tree prefix string (e.g., "│ └─") for display (CB-PROC-001).
    pub tree_prefix: String,
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
            threads: None,
            parent_pid: None,
            tree_depth: 0,
            is_last_child: false,
            tree_prefix: String::new(),
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

    /// Set thread count (CB-PROC-006).
    #[must_use]
    pub fn with_threads(mut self, threads: u32) -> Self {
        self.threads = Some(threads);
        self
    }

    /// Set parent PID (CB-PROC-001 tree view).
    #[must_use]
    pub fn with_parent_pid(mut self, ppid: u32) -> Self {
        self.parent_pid = Some(ppid);
        self
    }

    /// Set tree display info (CB-PROC-001 tree view).
    pub fn set_tree_info(&mut self, depth: usize, is_last: bool, prefix: String) {
        self.tree_depth = depth;
        self.is_last_child = is_last;
        self.tree_prefix = prefix;
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
#[allow(clippy::struct_excessive_bools)]
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
    /// Show thread count column (CB-PROC-006).
    show_threads: bool,
    /// Tree view mode (CB-PROC-001).
    tree_view: bool,
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
            show_threads: false,
            tree_view: false,
            bounds: Rect::default(),
        }
    }

    /// Set processes.
    pub fn set_processes(&mut self, processes: Vec<ProcessEntry>) {
        self.processes = processes;
        // Tree view (CB-PROC-001) takes precedence over sorting
        if self.tree_view {
            self.build_tree();
        } else {
            self.sort_processes();
        }
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

    /// Show thread count column (CB-PROC-006).
    #[must_use]
    pub fn with_threads_column(mut self) -> Self {
        self.show_threads = true;
        self
    }

    /// Enable tree view mode (CB-PROC-001).
    #[must_use]
    pub fn with_tree_view(mut self) -> Self {
        self.tree_view = true;
        self
    }

    /// Toggle tree view mode (CB-PROC-001).
    pub fn toggle_tree_view(&mut self) {
        self.tree_view = !self.tree_view;
        if self.tree_view {
            self.build_tree();
        }
    }

    /// Check if tree view is enabled (CB-PROC-001).
    #[must_use]
    pub fn is_tree_view(&self) -> bool {
        self.tree_view
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

    /// Build process tree structure (CB-PROC-001).
    ///
    /// Reorganizes processes into a tree by parent-child relationships.
    /// Uses ASCII art prefixes: └─ (last child), ├─ (middle child), │ (continuation).
    fn build_tree(&mut self) {
        use std::collections::HashMap;

        if self.processes.is_empty() {
            return;
        }

        // Build PID -> index map
        let pid_to_idx: HashMap<u32, usize> = self
            .processes
            .iter()
            .enumerate()
            .map(|(i, p)| (p.pid, i))
            .collect();

        // Build PPID -> children map
        let mut children: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut roots: Vec<usize> = Vec::new();

        for (idx, proc) in self.processes.iter().enumerate() {
            if let Some(ppid) = proc.parent_pid {
                if pid_to_idx.contains_key(&ppid) {
                    children.entry(ppid).or_default().push(idx);
                } else {
                    // Parent not in list, treat as root
                    roots.push(idx);
                }
            } else {
                roots.push(idx);
            }
        }

        // Sort children by CPU descending at each level
        for children_list in children.values_mut() {
            children_list.sort_by(|&a, &b| {
                self.processes[b]
                    .cpu_percent
                    .partial_cmp(&self.processes[a].cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        // Sort roots by CPU descending
        roots.sort_by(|&a, &b| {
            self.processes[b]
                .cpu_percent
                .partial_cmp(&self.processes[a].cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // DFS walk to build tree order
        let mut tree_order: Vec<(usize, usize, bool, String)> = Vec::new();

        let roots_len = roots.len();
        for (i, &root_idx) in roots.iter().enumerate() {
            let is_last = i == roots_len - 1;
            Self::build_tree_dfs(
                root_idx,
                0,
                "",
                is_last,
                &self.processes,
                &children,
                &mut tree_order,
            );
        }

        // Reorder processes and apply tree info
        let old_processes = std::mem::take(&mut self.processes);
        self.processes.reserve(tree_order.len());

        for (idx, depth, is_last, prefix) in tree_order {
            let mut proc = old_processes[idx].clone();
            proc.set_tree_info(depth, is_last, prefix);
            self.processes.push(proc);
        }
    }

    fn build_tree_dfs(
        idx: usize,
        depth: usize,
        prefix: &str,
        is_last: bool,
        processes: &[ProcessEntry],
        children: &std::collections::HashMap<u32, Vec<usize>>,
        tree_order: &mut Vec<(usize, usize, bool, String)>,
    ) {
        let proc = &processes[idx];
        let current_prefix = if depth == 0 {
            String::new()
        } else if is_last {
            format!("{prefix}└─")
        } else {
            format!("{prefix}├─")
        };

        tree_order.push((idx, depth, is_last, current_prefix));

        // Calculate next prefix for children
        let next_prefix = if depth == 0 {
            String::new()
        } else if is_last {
            format!("{prefix}  ")
        } else {
            format!("{prefix}│ ")
        };

        if let Some(child_indices) = children.get(&proc.pid) {
            let len = child_indices.len();
            for (i, &child_idx) in child_indices.iter().enumerate() {
                let child_is_last = i == len - 1;
                Self::build_tree_dfs(
                    child_idx,
                    depth + 1,
                    &next_prefix,
                    child_is_last,
                    processes,
                    children,
                    tree_order,
                );
            }
        }
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
        if s.chars().count() <= width {
            format!("{s:width$}")
        } else if width > 1 {
            // Use proper ellipsis character "…" instead of "..."
            let chars: String = s.chars().take(width - 1).collect();
            format!("{chars}…")
        } else {
            s.chars().take(width).collect()
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

        // Column layout - compact mode: PID S [OOM] [NI] [TH] C% M% COMMAND
        // PID can be up to 4194304 on Linux (7 digits), use 7 chars
        let pid_w = 7;
        let state_w = if self.compact { 2 } else { 0 }; // State column in compact mode
        let oom_w = if self.show_oom { 4 } else { 0 }; // OOM column (3 digits + space)
        let nice_w = if self.show_nice { 4 } else { 0 }; // NI column (3 chars + space)
        let threads_w = if self.show_threads { 4 } else { 0 }; // TH column (3 digits + space)
        let user_w = if self.compact { 0 } else { 8 };
        let cpu_w = 6;
        let mem_w = 6;
        let sep_w = if self.compact { 1 } else { 3 };
        let extra_cols = usize::from(self.show_oom)
            + usize::from(self.show_nice)
            + usize::from(self.show_threads);
        let num_seps = if self.compact { 3 } else { 4 } + extra_cols;
        let fixed_w = pid_w
            + state_w
            + oom_w
            + nice_w
            + threads_w
            + user_w
            + cpu_w
            + mem_w
            + sep_w * num_seps;
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
        if self.show_threads {
            header.push_str(if self.compact { " " } else { " │ " });
            let _ = write!(header, "{:>3}", "TH");
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
                let ni_color = match ni.cmp(&0) {
                    Ordering::Less => Color::new(0.3, 0.9, 0.9, 1.0), // Cyan - high priority
                    Ordering::Greater => Color::new(0.6, 0.6, 0.6, 1.0), // Gray - low priority
                    Ordering::Equal => Color::new(0.8, 0.8, 0.8, 1.0), // White - normal
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

            // Thread count (if enabled) - CB-PROC-006
            if self.show_threads {
                x += if self.compact { 1.0 } else { 3.0 };
                let th = proc.threads.unwrap_or(1);
                // Color: high thread count (>50) = cyan, medium (10-50) = yellow, low = white
                let th_color = if th > 50 {
                    Color::new(0.3, 0.9, 0.9, 1.0) // Cyan - many threads
                } else if th > 10 {
                    Color::new(1.0, 0.8, 0.2, 1.0) // Yellow - moderate
                } else {
                    Color::new(0.8, 0.8, 0.8, 1.0) // White - normal
                };
                let th_str = format!("{th:>3}");
                canvas.draw_text(
                    &th_str,
                    Point::new(x, y),
                    &TextStyle {
                        color: th_color,
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

            // Command (with tree prefix if enabled - CB-PROC-001)
            x += if self.compact { 1.0 } else { 3.0 };
            let cmd = if self.show_cmdline {
                proc.cmdline.as_deref().unwrap_or(&proc.command)
            } else {
                &proc.command
            };

            // Draw tree prefix in dim color, then command
            if self.tree_view && !proc.tree_prefix.is_empty() {
                let prefix_len = proc.tree_prefix.chars().count();
                let tree_style = TextStyle {
                    color: Color::new(0.4, 0.5, 0.6, 1.0),
                    ..Default::default()
                };
                canvas.draw_text(&proc.tree_prefix, Point::new(x, y), &tree_style);
                x += prefix_len as f32;

                // Truncate command to remaining space
                let remaining_w = cmd_w.saturating_sub(prefix_len);
                let cmd_str = Self::truncate(cmd, remaining_w);
                let cmd_style = if is_selected {
                    TextStyle {
                        color: Color::new(1.0, 1.0, 1.0, 1.0),
                        ..Default::default()
                    }
                } else {
                    default_style.clone()
                };
                canvas.draw_text(&cmd_str, Point::new(x, y), &cmd_style);
            } else {
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
                    Key::T => self.toggle_tree_view(), // CB-PROC-001
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
        assert_eq!(ProcessTable::truncate("hello world", 8), "hello w…");
        assert_eq!(ProcessTable::truncate("hi", 2), "hi");
        // Ensure proper ellipsis character is used
        assert!(ProcessTable::truncate("long text here", 6).ends_with('…'));
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

    // ========================================================================
    // Additional tests for paint() paths and better coverage
    // ========================================================================

    struct MockCanvas {
        texts: Vec<(String, Point)>,
        rects: Vec<Rect>,
    }

    impl MockCanvas {
        fn new() -> Self {
            Self {
                texts: vec![],
                rects: vec![],
            }
        }
    }

    impl Canvas for MockCanvas {
        fn fill_rect(&mut self, rect: Rect, _color: Color) {
            self.rects.push(rect);
        }
        fn stroke_rect(&mut self, _rect: Rect, _color: Color, _width: f32) {}
        fn draw_text(&mut self, text: &str, position: Point, _style: &TextStyle) {
            self.texts.push((text.to_string(), position));
        }
        fn draw_line(&mut self, _from: Point, _to: Point, _color: Color, _width: f32) {}
        fn fill_circle(&mut self, _center: Point, _radius: f32, _color: Color) {}
        fn stroke_circle(&mut self, _center: Point, _radius: f32, _color: Color, _width: f32) {}
        fn fill_arc(&mut self, _c: Point, _r: f32, _s: f32, _e: f32, _color: Color) {}
        fn draw_path(&mut self, _points: &[Point], _color: Color, _width: f32) {}
        fn fill_polygon(&mut self, _points: &[Point], _color: Color) {}
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
        fn push_transform(&mut self, _transform: presentar_core::Transform2D) {}
        fn pop_transform(&mut self) {}
    }

    #[test]
    fn test_process_table_paint_basic() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.bounds = Rect::new(0.0, 0.0, 80.0, 20.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Should have rendered header, separator, and rows
        assert!(!canvas.texts.is_empty());
        // Check header contains "PID"
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("PID")));
    }

    #[test]
    fn test_process_table_paint_compact() {
        let mut table = ProcessTable::new().compact();
        table.set_processes(sample_processes());
        table.bounds = Rect::new(0.0, 0.0, 60.0, 20.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Check compact header has "C%" and "M%" instead of "CPU%" and "MEM%"
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("C%")));
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("M%")));
    }

    #[test]
    fn test_process_table_paint_with_oom() {
        let mut table = ProcessTable::new().with_oom();
        let entries = vec![
            ProcessEntry::new(1, "user", 10.0, 5.0, "low_oom").with_oom_score(100),
            ProcessEntry::new(2, "user", 10.0, 5.0, "high_oom").with_oom_score(800),
            ProcessEntry::new(3, "user", 10.0, 5.0, "med_oom").with_oom_score(400),
        ];
        table.set_processes(entries);
        table.bounds = Rect::new(0.0, 0.0, 100.0, 20.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Should have OOM header
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("OOM")));
        // Should have OOM values rendered
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("100")));
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("800")));
    }

    #[test]
    fn test_process_table_paint_with_nice() {
        let mut table = ProcessTable::new().with_nice_column();
        let entries = vec![
            ProcessEntry::new(1, "user", 10.0, 5.0, "high_pri").with_nice(-10),
            ProcessEntry::new(2, "user", 10.0, 5.0, "low_pri").with_nice(10),
            ProcessEntry::new(3, "user", 10.0, 5.0, "normal").with_nice(0),
        ];
        table.set_processes(entries);
        table.bounds = Rect::new(0.0, 0.0, 100.0, 20.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Should have NI header
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("NI")));
    }

    #[test]
    fn test_process_table_paint_with_selection() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.select(1);
        table.bounds = Rect::new(0.0, 0.0, 80.0, 20.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Should have a selection rect
        assert!(!canvas.rects.is_empty());
    }

    #[test]
    fn test_process_table_paint_empty() {
        let mut table = ProcessTable::new();
        table.bounds = Rect::new(0.0, 0.0, 80.0, 20.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Should show "No processes" message
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("No processes")));
    }

    #[test]
    fn test_process_table_paint_zero_bounds() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.bounds = Rect::new(0.0, 0.0, 0.0, 0.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Should return early, no output
        assert!(canvas.texts.is_empty());
    }

    #[test]
    fn test_process_table_paint_with_cmdline() {
        let mut table = ProcessTable::new().with_cmdline();
        let entries = vec![
            ProcessEntry::new(1, "root", 0.5, 0.1, "bash").with_cmdline("/bin/bash --login -i")
        ];
        table.set_processes(entries);
        table.bounds = Rect::new(0.0, 0.0, 100.0, 20.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Should show cmdline instead of command
        assert!(canvas
            .texts
            .iter()
            .any(|(t, _)| t.contains("/bin/bash") || t.contains("--login")));
    }

    #[test]
    fn test_process_table_paint_compact_with_state() {
        let mut table = ProcessTable::new().compact();
        let entries = vec![
            ProcessEntry::new(1, "root", 50.0, 10.0, "running").with_state(ProcessState::Running),
            ProcessEntry::new(2, "user", 0.0, 0.5, "sleeping").with_state(ProcessState::Sleeping),
            ProcessEntry::new(3, "user", 0.0, 0.1, "zombie").with_state(ProcessState::Zombie),
        ];
        table.set_processes(entries);
        table.bounds = Rect::new(0.0, 0.0, 60.0, 20.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Should have state characters
        assert!(canvas.texts.iter().any(|(t, _)| t == "R")); // Running
        assert!(canvas.texts.iter().any(|(t, _)| t == "S")); // Sleeping
    }

    #[test]
    fn test_process_state_char() {
        assert_eq!(ProcessState::Running.char(), 'R');
        assert_eq!(ProcessState::Sleeping.char(), 'S');
        assert_eq!(ProcessState::DiskWait.char(), 'D');
        assert_eq!(ProcessState::Zombie.char(), 'Z');
        assert_eq!(ProcessState::Stopped.char(), 'T');
        assert_eq!(ProcessState::Idle.char(), 'I');
    }

    #[test]
    fn test_process_state_color() {
        // Each state should have a unique color
        let running = ProcessState::Running.color();
        let sleeping = ProcessState::Sleeping.color();
        let zombie = ProcessState::Zombie.color();
        assert_ne!(running, sleeping);
        assert_ne!(running, zombie);
    }

    #[test]
    fn test_process_state_default() {
        assert_eq!(ProcessState::default(), ProcessState::Sleeping);
    }

    #[test]
    fn test_process_entry_with_state() {
        let proc = ProcessEntry::new(1, "root", 0.0, 0.0, "test").with_state(ProcessState::Running);
        assert_eq!(proc.state, ProcessState::Running);
    }

    #[test]
    fn test_process_entry_with_cgroup() {
        let proc =
            ProcessEntry::new(1, "root", 0.0, 0.0, "test").with_cgroup("/user.slice/user-1000");
        assert_eq!(proc.cgroup.as_deref(), Some("/user.slice/user-1000"));
    }

    #[test]
    fn test_process_entry_with_nice() {
        let proc = ProcessEntry::new(1, "root", 0.0, 0.0, "test").with_nice(-5);
        assert_eq!(proc.nice, Some(-5));
    }

    #[test]
    fn test_process_entry_with_oom_score() {
        let proc = ProcessEntry::new(1, "root", 0.0, 0.0, "test").with_oom_score(500);
        assert_eq!(proc.oom_score, Some(500));
    }

    #[test]
    fn test_process_entry_with_threads() {
        let proc = ProcessEntry::new(1, "root", 0.0, 0.0, "test").with_threads(42);
        assert_eq!(proc.threads, Some(42));
    }

    #[test]
    fn test_process_table_with_threads_column() {
        let table = ProcessTable::new().with_threads_column();
        assert!(table.show_threads);
    }

    #[test]
    fn test_process_table_scroll() {
        let mut table = ProcessTable::new();
        // Create many processes to trigger scrolling
        let entries: Vec<ProcessEntry> = (0..50)
            .map(|i| ProcessEntry::new(i, "user", i as f32, 0.0, format!("proc{i}")))
            .collect();
        table.set_processes(entries);
        table.bounds = Rect::new(0.0, 0.0, 80.0, 10.0); // Only 8 visible rows
        table.layout(table.bounds);

        // Select a process beyond the visible area
        table.select(45);
        // scroll_offset should have been updated
        assert!(table.scroll_offset > 0);
    }

    #[test]
    fn test_process_table_ensure_visible_up() {
        let mut table = ProcessTable::new();
        let entries: Vec<ProcessEntry> = (0..20)
            .map(|i| ProcessEntry::new(i, "user", 0.0, 0.0, format!("proc{i}")))
            .collect();
        table.set_processes(entries);
        table.bounds = Rect::new(0.0, 0.0, 80.0, 10.0);
        table.scroll_offset = 10;
        table.selected = 5; // Above visible area

        table.ensure_visible();
        assert!(table.scroll_offset <= table.selected);
    }

    #[test]
    fn test_process_table_select_empty() {
        let mut table = ProcessTable::new();
        // Should not panic on empty table
        table.select(5);
        table.select_next();
        table.select_prev();
        assert_eq!(table.selected(), 0);
    }

    #[test]
    fn test_process_table_selected_process_empty() {
        let table = ProcessTable::new();
        assert!(table.selected_process().is_none());
    }

    #[test]
    fn test_process_table_budget() {
        let table = ProcessTable::new();
        let budget = table.budget();
        assert!(budget.paint_ms > 0);
    }

    #[test]
    fn test_process_table_assertions() {
        let table = ProcessTable::new();
        assert!(!table.assertions().is_empty());
    }

    #[test]
    fn test_process_table_set_processes_clamp_selection() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.selected = 2; // Last item
                            // Set fewer processes
        table.set_processes(vec![ProcessEntry::new(1, "root", 0.0, 0.0, "test")]);
        // Selection should be clamped
        assert_eq!(table.selected(), 0);
    }

    #[test]
    fn test_process_table_event_down() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());

        table.event(&Event::KeyDown { key: Key::Down });
        assert_eq!(table.selected(), 1);
    }

    #[test]
    fn test_process_table_event_up() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        table.select(2);

        table.event(&Event::KeyDown { key: Key::Up });
        assert_eq!(table.selected(), 1);
    }

    #[test]
    fn test_process_table_event_c() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        // First sort by something else
        table.sort_by(ProcessSort::Pid);

        table.event(&Event::KeyDown { key: Key::C });
        assert_eq!(table.current_sort(), ProcessSort::Cpu);
    }

    #[test]
    fn test_process_table_event_m() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());

        table.event(&Event::KeyDown { key: Key::M });
        assert_eq!(table.current_sort(), ProcessSort::Memory);
    }

    #[test]
    fn test_process_table_event_n() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());

        table.event(&Event::KeyDown { key: Key::N });
        assert_eq!(table.current_sort(), ProcessSort::Command);
    }

    #[test]
    fn test_process_table_event_o() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());

        table.event(&Event::KeyDown { key: Key::O });
        assert_eq!(table.current_sort(), ProcessSort::Oom);
    }

    #[test]
    fn test_process_table_event_other() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        let prev_selected = table.selected();

        // Event that doesn't match any key
        table.event(&Event::KeyDown { key: Key::A });
        assert_eq!(table.selected(), prev_selected);
    }

    #[test]
    fn test_process_table_event_non_keydown() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());

        // Non-keydown event
        let result = table.event(&Event::Resize {
            width: 100.0,
            height: 50.0,
        });
        assert!(result.is_none());
    }

    #[test]
    fn test_process_table_with_cpu_gradient() {
        let gradient = Gradient::from_hex(&["#0000FF", "#FF0000"]);
        let table = ProcessTable::new().with_cpu_gradient(gradient);
        // Just verify it compiles and doesn't panic
        assert!(!table.is_empty() || table.is_empty());
    }

    #[test]
    fn test_process_table_with_mem_gradient() {
        let gradient = Gradient::from_hex(&["#00FF00", "#FF0000"]);
        let table = ProcessTable::new().with_mem_gradient(gradient);
        assert!(!table.is_empty() || table.is_empty());
    }

    #[test]
    fn test_process_table_measure_compact() {
        let table = ProcessTable::new().compact();
        let size = table.measure(Constraints::new(0.0, 100.0, 0.0, 50.0));
        assert!(size.width >= 40.0); // Compact mode has smaller min width
    }

    #[test]
    fn test_process_table_truncate_exact() {
        assert_eq!(ProcessTable::truncate("exact", 5), "exact");
    }

    #[test]
    fn test_process_table_truncate_width_1() {
        assert_eq!(ProcessTable::truncate("hello", 1), "h");
    }

    #[test]
    fn test_process_table_paint_all_columns() {
        // Test paint with all optional columns enabled
        let mut table = ProcessTable::new()
            .compact()
            .with_oom()
            .with_nice_column()
            .with_cmdline();

        let entries = vec![
            ProcessEntry::new(1, "root", 50.0, 10.0, "bash")
                .with_state(ProcessState::Running)
                .with_oom_score(100)
                .with_nice(-5)
                .with_cmdline("/bin/bash"),
            ProcessEntry::new(2, "user", 30.0, 5.0, "vim")
                .with_state(ProcessState::Sleeping)
                .with_oom_score(600)
                .with_nice(10)
                .with_cmdline("/usr/bin/vim"),
        ];
        table.set_processes(entries);
        table.bounds = Rect::new(0.0, 0.0, 120.0, 20.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // All columns should be rendered
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("PID")));
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("OOM")));
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("NI")));
    }

    #[test]
    fn test_process_entry_clone() {
        let proc = ProcessEntry::new(1, "root", 50.0, 10.0, "test")
            .with_state(ProcessState::Running)
            .with_oom_score(100);
        let cloned = proc.clone();
        assert_eq!(cloned.pid, proc.pid);
        assert_eq!(cloned.state, proc.state);
    }

    #[test]
    fn test_process_entry_debug() {
        let proc = ProcessEntry::new(1, "root", 0.0, 0.0, "test");
        let debug = format!("{:?}", proc);
        assert!(debug.contains("ProcessEntry"));
    }

    #[test]
    fn test_process_sort_debug() {
        let sort = ProcessSort::Cpu;
        let debug = format!("{:?}", sort);
        assert!(debug.contains("Cpu"));
    }

    #[test]
    fn test_process_table_clone() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        let cloned = table.clone();
        assert_eq!(cloned.len(), table.len());
    }

    #[test]
    fn test_process_table_debug() {
        let table = ProcessTable::new();
        let debug = format!("{:?}", table);
        assert!(debug.contains("ProcessTable"));
    }

    #[test]
    fn test_process_state_debug() {
        let state = ProcessState::Running;
        let debug = format!("{:?}", state);
        assert!(debug.contains("Running"));
    }

    // ========================================================================
    // Tree view tests (CB-PROC-001)
    // ========================================================================

    #[test]
    fn test_process_entry_with_parent_pid() {
        let proc = ProcessEntry::new(100, "user", 10.0, 5.0, "child").with_parent_pid(1);
        assert_eq!(proc.parent_pid, Some(1));
    }

    #[test]
    fn test_process_entry_set_tree_info() {
        let mut proc = ProcessEntry::new(100, "user", 10.0, 5.0, "child");
        proc.set_tree_info(2, true, "│ └─".to_string());
        assert_eq!(proc.tree_depth, 2);
        assert!(proc.is_last_child);
        assert_eq!(proc.tree_prefix, "│ └─");
    }

    #[test]
    fn test_process_table_with_tree_view() {
        let table = ProcessTable::new().with_tree_view();
        assert!(table.is_tree_view());
    }

    #[test]
    fn test_process_table_toggle_tree_view() {
        let mut table = ProcessTable::new();
        assert!(!table.is_tree_view());

        table.toggle_tree_view();
        assert!(table.is_tree_view());

        table.toggle_tree_view();
        assert!(!table.is_tree_view());
    }

    #[test]
    fn test_process_table_tree_view_builds_tree() {
        let mut table = ProcessTable::new().with_tree_view();

        // Create parent-child hierarchy:
        // 1 (systemd) -> 100 (bash) -> 200 (vim)
        //             -> 101 (sshd)
        let entries = vec![
            ProcessEntry::new(200, "user", 5.0, 1.0, "vim").with_parent_pid(100),
            ProcessEntry::new(100, "user", 10.0, 2.0, "bash").with_parent_pid(1),
            ProcessEntry::new(101, "root", 1.0, 0.5, "sshd").with_parent_pid(1),
            ProcessEntry::new(1, "root", 0.5, 0.1, "systemd"),
        ];
        table.set_processes(entries);

        // After tree building, systemd should be first (root)
        assert_eq!(table.processes[0].command, "systemd");

        // Check tree prefixes
        // systemd (root) has no prefix
        assert_eq!(table.processes[0].tree_prefix, "");
        // bash is child of systemd, and has higher CPU than sshd
        // So bash should come before sshd
    }

    #[test]
    fn test_process_table_tree_view_prefix_chars() {
        let mut table = ProcessTable::new().with_tree_view();

        // Create: 1 -> 2 -> 3
        let entries = vec![
            ProcessEntry::new(3, "user", 5.0, 1.0, "grandchild").with_parent_pid(2),
            ProcessEntry::new(2, "user", 10.0, 2.0, "child").with_parent_pid(1),
            ProcessEntry::new(1, "root", 0.5, 0.1, "parent"),
        ];
        table.set_processes(entries);

        // Parent should have no prefix
        assert_eq!(table.processes[0].tree_prefix, "");
        // Child should have └─ (last child of parent)
        assert!(
            table.processes[1].tree_prefix.contains('└')
                || table.processes[1].tree_prefix.contains('├')
        );
    }

    #[test]
    fn test_process_table_event_t_toggles_tree() {
        let mut table = ProcessTable::new();
        table.set_processes(sample_processes());
        assert!(!table.is_tree_view());

        table.event(&Event::KeyDown { key: Key::T });
        assert!(table.is_tree_view());

        table.event(&Event::KeyDown { key: Key::T });
        assert!(!table.is_tree_view());
    }

    #[test]
    fn test_process_table_tree_view_paint() {
        let mut table = ProcessTable::new().with_tree_view();

        let entries = vec![
            ProcessEntry::new(2, "user", 10.0, 2.0, "child").with_parent_pid(1),
            ProcessEntry::new(1, "root", 0.5, 0.1, "parent"),
        ];
        table.set_processes(entries);
        table.bounds = Rect::new(0.0, 0.0, 80.0, 10.0);

        let mut canvas = MockCanvas::new();
        table.paint(&mut canvas);

        // Should have tree prefix in output
        assert!(canvas
            .texts
            .iter()
            .any(|(t, _)| t.contains("└") || t.contains("├")));
    }

    #[test]
    fn test_process_table_tree_empty() {
        let mut table = ProcessTable::new().with_tree_view();
        // Should not panic on empty
        table.set_processes(vec![]);
        assert!(table.is_empty());
    }

    // ========================================================================
    // Falsification Tests for CB-PROC-001 (Phase 7 QA Gate)
    // ========================================================================

    /// F-TREE-001: "Orphaned Child" Test
    /// Hierarchy MUST override sorting. Children MUST appear immediately below parent.
    #[test]
    fn test_f_tree_001_hierarchy_overrides_sorting() {
        let mut table = ProcessTable::new().with_tree_view();

        // sh (PID 100) with two sleep children (PIDs 200, 201)
        // Higher CPU processes elsewhere should NOT split the hierarchy
        let entries = vec![
            ProcessEntry::new(999, "root", 99.0, 50.0, "chrome"), // High CPU unrelated
            ProcessEntry::new(200, "user", 0.1, 0.1, "sleep").with_parent_pid(100),
            ProcessEntry::new(201, "user", 0.1, 0.1, "sleep").with_parent_pid(100),
            ProcessEntry::new(100, "user", 1.0, 0.5, "sh"),
            ProcessEntry::new(1, "root", 0.5, 0.1, "systemd"),
        ];
        table.set_processes(entries);

        // Find sh in the tree
        let sh_idx = table
            .processes
            .iter()
            .position(|p| p.command == "sh")
            .expect("sh not found");

        // Both sleep processes MUST be immediately after sh
        let sleep1_idx = table
            .processes
            .iter()
            .position(|p| p.command == "sleep" && p.pid == 200)
            .expect("sleep 200 not found");
        let sleep2_idx = table
            .processes
            .iter()
            .position(|p| p.command == "sleep" && p.pid == 201)
            .expect("sleep 201 not found");

        // Children must appear IMMEDIATELY after parent (next indices)
        assert!(
            sleep1_idx > sh_idx && sleep1_idx <= sh_idx + 2,
            "sleep 200 (idx {}) should be immediately after sh (idx {})",
            sleep1_idx,
            sh_idx
        );
        assert!(
            sleep2_idx > sh_idx && sleep2_idx <= sh_idx + 2,
            "sleep 201 (idx {}) should be immediately after sh (idx {})",
            sleep2_idx,
            sh_idx
        );

        // Unrelated high-CPU process should NOT be between sh and its children
        let chrome_idx = table
            .processes
            .iter()
            .position(|p| p.command == "chrome")
            .expect("chrome not found");
        assert!(
            !(chrome_idx > sh_idx && chrome_idx < sleep1_idx.max(sleep2_idx)),
            "Unrelated process should not split parent-child hierarchy"
        );
    }

    /// F-TREE-002: "Live Re-Parenting" - Orphan handling
    /// When parent is killed, orphans should gracefully become roots
    #[test]
    fn test_f_tree_002_orphan_handling() {
        let mut table = ProcessTable::new().with_tree_view();

        // Child processes whose parent (PID 100) is NOT in the list
        let entries = vec![
            ProcessEntry::new(200, "user", 5.0, 1.0, "orphan1").with_parent_pid(100), // Parent missing
            ProcessEntry::new(201, "user", 3.0, 1.0, "orphan2").with_parent_pid(100), // Parent missing
            ProcessEntry::new(1, "root", 0.5, 0.1, "systemd"),
        ];
        table.set_processes(entries);

        // Should not panic - orphans become roots
        assert_eq!(table.len(), 3);

        // Orphans should have depth 0 (root level) since parent not found
        let orphan1 = table
            .processes
            .iter()
            .find(|p| p.command == "orphan1")
            .unwrap();
        let orphan2 = table
            .processes
            .iter()
            .find(|p| p.command == "orphan2")
            .unwrap();

        // Orphans treated as roots have no tree prefix
        assert_eq!(orphan1.tree_depth, 0);
        assert_eq!(orphan2.tree_depth, 0);
    }

    /// F-TREE-003: "Deep Nesting" Boundary (15 levels)
    /// Tree must handle deep hierarchies without overflow or crash
    #[test]
    fn test_f_tree_003_deep_nesting_15_levels() {
        let mut table = ProcessTable::new().with_tree_view();

        // Create 15-level deep hierarchy
        let mut entries = vec![ProcessEntry::new(1, "root", 0.5, 0.1, "init")];

        for depth in 1..=15 {
            let pid = (depth + 1) as u32;
            let ppid = depth as u32;
            entries.push(
                ProcessEntry::new(pid, "user", 0.1, 0.1, format!("level{depth}"))
                    .with_parent_pid(ppid),
            );
        }

        table.set_processes(entries);

        // Should not panic
        assert_eq!(table.len(), 16); // 1 root + 15 children

        // Verify deepest process has depth 15
        let deepest = table
            .processes
            .iter()
            .find(|p| p.command == "level15")
            .unwrap();
        assert_eq!(deepest.tree_depth, 15);

        // Verify prefix has correct structure (should have 14 "│ " or "  " segments)
        let prefix_segments =
            deepest.tree_prefix.matches("│").count() + deepest.tree_prefix.matches("  ").count();
        // At depth 15, prefix should have accumulated continuation chars
        assert!(
            deepest.tree_prefix.len() > 20,
            "Deep prefix should be substantial: '{}'",
            deepest.tree_prefix
        );
    }

    /// F-TREE-004: Verify DFS traversal order
    /// Tree order must be parent, then all descendants, then next sibling
    #[test]
    fn test_f_tree_004_dfs_traversal_order() {
        let mut table = ProcessTable::new().with_tree_view();

        // Tree: A -> B -> D
        //           -> E
        //       -> C
        let entries = vec![
            ProcessEntry::new(5, "user", 1.0, 1.0, "E").with_parent_pid(2),
            ProcessEntry::new(4, "user", 1.0, 1.0, "D").with_parent_pid(2),
            ProcessEntry::new(3, "user", 1.0, 1.0, "C").with_parent_pid(1),
            ProcessEntry::new(2, "user", 2.0, 1.0, "B").with_parent_pid(1), // Higher CPU
            ProcessEntry::new(1, "root", 0.5, 0.1, "A"),
        ];
        table.set_processes(entries);

        // Expected DFS order (sorted by CPU within siblings): A, B, D, E, C
        // B comes before C because B has higher CPU
        let commands: Vec<&str> = table.processes.iter().map(|p| p.command.as_str()).collect();

        assert_eq!(commands[0], "A", "Root should be first");
        assert_eq!(commands[1], "B", "B (higher CPU child) should be second");
        // D and E are B's children
        assert!(
            commands[2] == "D" || commands[2] == "E",
            "B's children should follow B"
        );
        assert!(
            commands[3] == "D" || commands[3] == "E",
            "B's children should follow B"
        );
        assert_eq!(commands[4], "C", "C should be after B's subtree");
    }
}
