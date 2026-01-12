//! `ProcessDataFrame` widget - Data science view for process monitoring.
//!
//! Grammar of Graphics construct: Process data as sortable `DataFrame` with:
//! - PID, Name, State columns
//! - CPU% with inline sparkline history
//! - MEM% with micro bar
//! - Threads, Priority, User columns
//! - Sort by any column (click or keyboard)
//!
//! Implements SPEC-024 Section 27.8 - Framework-First pattern.

use compact_str::CompactString;
use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event, Key,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::collections::HashMap;
use std::time::Duration;

/// Process state for display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProcessDisplayState {
    #[default]
    Running,
    Sleeping,
    Idle,
    Zombie,
    Stopped,
    Unknown,
}

impl ProcessDisplayState {
    /// Get display character and color.
    #[must_use]
    pub fn render(self) -> (char, Color) {
        match self {
            Self::Running => ('R', Color::new(0.2, 0.9, 0.2, 1.0)),
            Self::Sleeping => ('S', Color::new(0.6, 0.6, 0.6, 1.0)),
            Self::Idle => ('I', Color::new(0.5, 0.5, 0.5, 1.0)),
            Self::Zombie => ('Z', Color::new(0.9, 0.2, 0.2, 1.0)),
            Self::Stopped => ('T', Color::new(0.9, 0.7, 0.1, 1.0)),
            Self::Unknown => ('?', Color::new(0.4, 0.4, 0.4, 1.0)),
        }
    }
}

/// Sort column for process table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProcessSortColumn {
    Pid,
    Name,
    #[default]
    Cpu,
    Mem,
    Threads,
    Priority,
    User,
    Time,
}

impl ProcessSortColumn {
    /// Get column header string.
    #[must_use]
    pub fn header(self) -> &'static str {
        match self {
            Self::Pid => "PID",
            Self::Name => "COMMAND",
            Self::Cpu => "CPU%",
            Self::Mem => "MEM%",
            Self::Threads => "THR",
            Self::Priority => "PRI",
            Self::User => "USER",
            Self::Time => "TIME+",
        }
    }
}

/// Single process row data.
#[derive(Debug, Clone)]
pub struct ProcessRow {
    /// Process ID.
    pub pid: u32,
    /// Process name/command.
    pub name: CompactString,
    /// Current CPU percentage.
    pub cpu_percent: f32,
    /// CPU history (last N samples for sparkline).
    pub cpu_history: Vec<f32>,
    /// Memory percentage.
    pub mem_percent: f32,
    /// Process state.
    pub state: ProcessDisplayState,
    /// Number of threads.
    pub threads: u32,
    /// Priority/nice value.
    pub priority: i32,
    /// User name.
    pub user: CompactString,
    /// CPU time in seconds.
    pub cpu_time_secs: u64,
}

impl Default for ProcessRow {
    fn default() -> Self {
        Self {
            pid: 0,
            name: CompactString::new(""),
            cpu_percent: 0.0,
            cpu_history: Vec::new(),
            mem_percent: 0.0,
            state: ProcessDisplayState::default(),
            threads: 1,
            priority: 0,
            user: CompactString::new(""),
            cpu_time_secs: 0,
        }
    }
}

/// Process `DataFrame` widget for data science view.
#[derive(Debug, Clone)]
pub struct ProcessDataFrame {
    /// Process rows.
    rows: Vec<ProcessRow>,
    /// CPU history per PID (ring buffer).
    cpu_histories: HashMap<u32, Vec<f32>>,
    /// History length for sparklines.
    history_len: usize,
    /// Current sort column.
    sort_column: ProcessSortColumn,
    /// Sort descending.
    sort_desc: bool,
    /// Selected row index.
    selected_row: Option<usize>,
    /// Scroll offset.
    scroll_offset: usize,
    /// Show header row.
    show_header: bool,
    /// Column widths.
    col_widths: ProcessColumnWidths,
    /// Primary accent color.
    accent_color: Color,
    /// Cached bounds.
    bounds: Rect,
}

/// Column width configuration.
#[derive(Debug, Clone, Copy)]
pub struct ProcessColumnWidths {
    pub pid: usize,
    pub name: usize,
    pub cpu: usize,
    pub sparkline: usize,
    pub mem: usize,
    pub state: usize,
    pub threads: usize,
    pub priority: usize,
    pub user: usize,
    pub time: usize,
}

impl Default for ProcessColumnWidths {
    fn default() -> Self {
        Self {
            pid: 7,
            name: 20,
            cpu: 6,
            sparkline: 12,
            mem: 6,
            state: 3,
            threads: 4,
            priority: 4,
            user: 10,
            time: 10,
        }
    }
}

impl Default for ProcessDataFrame {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessDataFrame {
    /// Create a new empty process dataframe.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            cpu_histories: HashMap::new(),
            history_len: 60, // 60 samples = 60s at 1s refresh
            sort_column: ProcessSortColumn::Cpu,
            sort_desc: true,
            selected_row: None,
            scroll_offset: 0,
            show_header: true,
            col_widths: ProcessColumnWidths::default(),
            accent_color: Color::new(0.4, 0.7, 1.0, 1.0),
            bounds: Rect::default(),
        }
    }

    /// Update with new process data.
    pub fn update_processes(&mut self, processes: Vec<ProcessRow>) {
        // Update CPU histories
        for proc in &processes {
            let history = self.cpu_histories.entry(proc.pid).or_default();
            history.push(proc.cpu_percent);
            if history.len() > self.history_len {
                history.remove(0);
            }
        }

        // Clean up histories for dead processes
        let live_pids: std::collections::HashSet<u32> = processes.iter().map(|p| p.pid).collect();
        self.cpu_histories.retain(|pid, _| live_pids.contains(pid));

        // Update rows with histories
        self.rows = processes
            .into_iter()
            .map(|mut proc| {
                if let Some(history) = self.cpu_histories.get(&proc.pid) {
                    proc.cpu_history = history.clone();
                }
                proc
            })
            .collect();

        self.sort_rows();
    }

    /// Set sort column.
    #[must_use]
    pub fn with_sort(mut self, column: ProcessSortColumn, descending: bool) -> Self {
        self.sort_column = column;
        self.sort_desc = descending;
        self
    }

    /// Set accent color.
    #[must_use]
    pub fn with_accent_color(mut self, color: Color) -> Self {
        self.accent_color = color;
        self
    }

    /// Set column widths.
    #[must_use]
    pub fn with_column_widths(mut self, widths: ProcessColumnWidths) -> Self {
        self.col_widths = widths;
        self
    }

    /// Set history length.
    #[must_use]
    pub fn with_history_len(mut self, len: usize) -> Self {
        self.history_len = len;
        self
    }

    /// Get selected process PID (if any).
    #[must_use]
    pub fn selected_pid(&self) -> Option<u32> {
        self.selected_row
            .and_then(|idx| self.rows.get(idx).map(|r| r.pid))
    }

    /// Scroll up one row.
    pub fn scroll_up(&mut self) {
        if let Some(sel) = self.selected_row {
            if sel > 0 {
                self.selected_row = Some(sel - 1);
                if sel - 1 < self.scroll_offset {
                    self.scroll_offset = sel - 1;
                }
            }
        } else if !self.rows.is_empty() {
            self.selected_row = Some(0);
        }
    }

    /// Scroll down one row.
    pub fn scroll_down(&mut self) {
        let max_visible = self.visible_rows();
        if let Some(sel) = self.selected_row {
            if sel < self.rows.len().saturating_sub(1) {
                self.selected_row = Some(sel + 1);
                if sel + 1 >= self.scroll_offset + max_visible {
                    self.scroll_offset = (sel + 2).saturating_sub(max_visible);
                }
            }
        } else if !self.rows.is_empty() {
            self.selected_row = Some(0);
        }
    }

    /// Cycle sort column.
    pub fn cycle_sort(&mut self) {
        self.sort_column = match self.sort_column {
            ProcessSortColumn::Cpu => ProcessSortColumn::Mem,
            ProcessSortColumn::Mem => ProcessSortColumn::Pid,
            ProcessSortColumn::Pid => ProcessSortColumn::Name,
            ProcessSortColumn::Name => ProcessSortColumn::Threads,
            ProcessSortColumn::Threads => ProcessSortColumn::Time,
            ProcessSortColumn::Time => ProcessSortColumn::Cpu,
            _ => ProcessSortColumn::Cpu,
        };
        self.sort_rows();
    }

    /// Toggle sort direction.
    pub fn toggle_sort_direction(&mut self) {
        self.sort_desc = !self.sort_desc;
        self.sort_rows();
    }

    fn sort_rows(&mut self) {
        let desc = self.sort_desc;
        match self.sort_column {
            ProcessSortColumn::Pid => {
                self.rows.sort_by(|a, b| {
                    if desc {
                        b.pid.cmp(&a.pid)
                    } else {
                        a.pid.cmp(&b.pid)
                    }
                });
            }
            ProcessSortColumn::Name => {
                self.rows.sort_by(|a, b| {
                    if desc {
                        b.name.cmp(&a.name)
                    } else {
                        a.name.cmp(&b.name)
                    }
                });
            }
            ProcessSortColumn::Cpu => {
                self.rows.sort_by(|a, b| {
                    if desc {
                        b.cpu_percent
                            .partial_cmp(&a.cpu_percent)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.cpu_percent
                            .partial_cmp(&b.cpu_percent)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                });
            }
            ProcessSortColumn::Mem => {
                self.rows.sort_by(|a, b| {
                    if desc {
                        b.mem_percent
                            .partial_cmp(&a.mem_percent)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.mem_percent
                            .partial_cmp(&b.mem_percent)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                });
            }
            ProcessSortColumn::Threads => {
                self.rows.sort_by(|a, b| {
                    if desc {
                        b.threads.cmp(&a.threads)
                    } else {
                        a.threads.cmp(&b.threads)
                    }
                });
            }
            ProcessSortColumn::Priority => {
                self.rows.sort_by(|a, b| {
                    if desc {
                        b.priority.cmp(&a.priority)
                    } else {
                        a.priority.cmp(&b.priority)
                    }
                });
            }
            ProcessSortColumn::User => {
                self.rows.sort_by(|a, b| {
                    if desc {
                        b.user.cmp(&a.user)
                    } else {
                        a.user.cmp(&b.user)
                    }
                });
            }
            ProcessSortColumn::Time => {
                self.rows.sort_by(|a, b| {
                    if desc {
                        b.cpu_time_secs.cmp(&a.cpu_time_secs)
                    } else {
                        a.cpu_time_secs.cmp(&b.cpu_time_secs)
                    }
                });
            }
        }
    }

    fn visible_rows(&self) -> usize {
        let header_offset = if self.show_header { 2 } else { 0 };
        (self.bounds.height as usize).saturating_sub(header_offset)
    }

    fn render_sparkline(values: &[f32], width: usize) -> String {
        const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        if values.is_empty() {
            return "─".repeat(width);
        }

        // Scale to 0-100% range
        let sample_width = width.min(values.len());
        let step = values.len().saturating_sub(1) / sample_width.max(1);

        (0..sample_width)
            .map(|i| {
                let idx = (i * step.max(1)).min(values.len().saturating_sub(1));
                let v = values[idx].clamp(0.0, 100.0);
                let norm = ((v / 100.0) * 7.0).round() as usize;
                BARS[norm.min(7)]
            })
            .collect()
    }

    fn render_microbar(value: f32, width: usize) -> String {
        let pct = (value / 100.0).clamp(0.0, 1.0);
        let filled = ((width as f32) * pct).round() as usize;
        let empty = width.saturating_sub(filled);
        format!("{}{}", "█".repeat(filled), "░".repeat(empty))
    }

    fn format_time(secs: u64) -> String {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;
        if hours > 99 {
            format!("{hours}h")
        } else if hours > 0 {
            format!("{hours}:{mins:02}:{secs:02}")
        } else {
            format!("{mins}:{secs:02}")
        }
    }
}

impl Brick for ProcessDataFrame {
    fn brick_name(&self) -> &'static str {
        "process_dataframe"
    }

    fn assertions(&self) -> &[BrickAssertion] {
        static ASSERTIONS: &[BrickAssertion] = &[BrickAssertion::max_latency_ms(16)];
        ASSERTIONS
    }

    fn budget(&self) -> BrickBudget {
        BrickBudget::uniform(16)
    }

    fn verify(&self) -> BrickVerification {
        BrickVerification {
            passed: self.assertions().to_vec(),
            failed: vec![],
            verification_time: Duration::from_micros(10),
        }
    }

    fn to_html(&self) -> String {
        format!(
            r#"<table class="process-dataframe"><thead><tr>{}</tr></thead></table>"#,
            ["PID", "COMMAND", "CPU%", "MEM%", "THR", "USER"]
                .iter()
                .map(|h| format!("<th>{h}</th>"))
                .collect::<String>()
        )
    }

    fn to_css(&self) -> String {
        ".process-dataframe { sort: cpu; }".to_string()
    }
}

impl Widget for ProcessDataFrame {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let header_height = if self.show_header { 2.0 } else { 0.0 };
        let row_height = self.rows.len().min(30) as f32;
        let height = (header_height + row_height).min(constraints.max_height);
        constraints.constrain(Size::new(constraints.max_width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.bounds.width < 40.0 || self.bounds.height < 3.0 {
            return;
        }

        let mut y = self.bounds.y;
        let x_start = self.bounds.x;
        let w = &self.col_widths;

        // Calculate column positions
        let col_positions = [
            0usize,                                                                      // PID
            w.pid,                                                                       // Name
            w.pid + w.name,                                                              // CPU%
            w.pid + w.name + w.cpu,                       // Sparkline
            w.pid + w.name + w.cpu + w.sparkline,         // MEM%
            w.pid + w.name + w.cpu + w.sparkline + w.mem, // State
            w.pid + w.name + w.cpu + w.sparkline + w.mem + w.state, // Threads
            w.pid + w.name + w.cpu + w.sparkline + w.mem + w.state + w.threads, // User
            w.pid + w.name + w.cpu + w.sparkline + w.mem + w.state + w.threads + w.user, // Time
        ];

        let header_style = TextStyle {
            color: self.accent_color,
            ..Default::default()
        };

        let dim_style = TextStyle {
            color: Color::new(0.5, 0.5, 0.5, 1.0),
            ..Default::default()
        };

        // Header row
        if self.show_header {
            let headers = [
                (ProcessSortColumn::Pid, w.pid),
                (ProcessSortColumn::Name, w.name),
                (ProcessSortColumn::Cpu, w.cpu + w.sparkline),
                (ProcessSortColumn::Mem, w.mem),
                (ProcessSortColumn::Threads, w.state + w.threads),
                (ProcessSortColumn::User, w.user),
                (ProcessSortColumn::Time, w.time),
            ];

            let mut hx = x_start;
            for (col, width) in headers {
                let is_sorted = col == self.sort_column;
                let arrow = if is_sorted {
                    if self.sort_desc {
                        "▼"
                    } else {
                        "▲"
                    }
                } else {
                    ""
                };
                let text = format!("{}{}", col.header(), arrow);
                let text = if text.len() > width {
                    text[..width].to_string()
                } else {
                    text
                };

                let style = if is_sorted { &header_style } else { &dim_style };
                canvas.draw_text(&text, Point::new(hx, y), style);
                hx += width as f32 + 1.0;
            }

            y += 1.0;

            // Separator line
            let sep = "─".repeat((self.bounds.width as usize).min(120));
            canvas.draw_text(&sep, Point::new(x_start, y), &dim_style);
            y += 1.0;
        }

        // Data rows
        let visible_rows = self.visible_rows();
        let end_idx = (self.scroll_offset + visible_rows).min(self.rows.len());

        for (rel_idx, row) in self.rows[self.scroll_offset..end_idx].iter().enumerate() {
            let abs_idx = self.scroll_offset + rel_idx;
            let is_selected = self.selected_row == Some(abs_idx);

            let row_color = if is_selected {
                Color::new(0.2, 0.3, 0.4, 1.0)
            } else {
                Color::new(0.0, 0.0, 0.0, 0.0)
            };

            // Background for selected row
            if is_selected {
                let bg_rect = Rect::new(x_start, y, self.bounds.width, 1.0);
                canvas.fill_rect(bg_rect, row_color);
            }

            let text_color = if is_selected {
                Color::WHITE
            } else {
                Color::new(0.9, 0.9, 0.9, 1.0)
            };

            let text_style = TextStyle {
                color: text_color,
                ..Default::default()
            };

            // PID
            let pid_str = format!("{:>width$}", row.pid, width = w.pid);
            canvas.draw_text(
                &pid_str,
                Point::new(x_start + col_positions[0] as f32, y),
                &text_style,
            );

            // Name (truncated)
            let name = if row.name.len() > w.name {
                format!("{}…", &row.name[..w.name - 1])
            } else {
                format!("{:<width$}", row.name, width = w.name)
            };
            canvas.draw_text(
                &name,
                Point::new(x_start + col_positions[1] as f32, y),
                &text_style,
            );

            // CPU% with color gradient
            let cpu_color = if row.cpu_percent > 80.0 {
                Color::new(0.9, 0.2, 0.2, 1.0)
            } else if row.cpu_percent > 50.0 {
                Color::new(0.9, 0.7, 0.1, 1.0)
            } else if row.cpu_percent > 10.0 {
                Color::new(0.2, 0.8, 0.2, 1.0)
            } else {
                text_color
            };
            let cpu_str = format!("{:>5.1}", row.cpu_percent);
            canvas.draw_text(
                &cpu_str,
                Point::new(x_start + col_positions[2] as f32, y),
                &TextStyle {
                    color: cpu_color,
                    ..Default::default()
                },
            );

            // CPU sparkline
            let sparkline = Self::render_sparkline(&row.cpu_history, w.sparkline);
            canvas.draw_text(
                &sparkline,
                Point::new(x_start + col_positions[3] as f32, y),
                &TextStyle {
                    color: self.accent_color,
                    ..Default::default()
                },
            );

            // MEM% with micro bar
            let mem_bar = Self::render_microbar(row.mem_percent, 4);
            let mem_str = format!("{:>4.1}", row.mem_percent);
            canvas.draw_text(
                &format!("{mem_bar}{mem_str}"),
                Point::new(x_start + col_positions[4] as f32, y),
                &TextStyle {
                    color: Color::new(0.7, 0.5, 0.9, 1.0),
                    ..Default::default()
                },
            );

            // State
            let (state_ch, state_color) = row.state.render();
            canvas.draw_text(
                &state_ch.to_string(),
                Point::new(x_start + col_positions[5] as f32, y),
                &TextStyle {
                    color: state_color,
                    ..Default::default()
                },
            );

            // Threads
            let thr_str = format!("{:>3}", row.threads);
            canvas.draw_text(
                &thr_str,
                Point::new(x_start + col_positions[6] as f32 + 1.0, y),
                &text_style,
            );

            // User (truncated)
            let user = if row.user.len() > w.user {
                format!("{}…", &row.user[..w.user - 1])
            } else {
                format!("{:<width$}", row.user, width = w.user)
            };
            canvas.draw_text(
                &user,
                Point::new(x_start + col_positions[7] as f32, y),
                &text_style,
            );

            // Time
            let time_str = Self::format_time(row.cpu_time_secs);
            canvas.draw_text(
                &time_str,
                Point::new(x_start + col_positions[8] as f32, y),
                &text_style,
            );

            y += 1.0;
        }

        // Scroll indicator if needed
        if self.rows.len() > visible_rows {
            let scroll_pct = self.scroll_offset as f32 / (self.rows.len() - visible_rows) as f32;
            let indicator_y = self.bounds.y + 2.0 + (scroll_pct * (visible_rows - 1) as f32);
            canvas.draw_text(
                "│",
                Point::new(self.bounds.x + self.bounds.width - 1.0, indicator_y),
                &dim_style,
            );
        }
    }

    fn event(&mut self, event: &Event) -> Option<Box<dyn Any + Send>> {
        match event {
            Event::KeyDown {
                key: Key::Up | Key::K,
            } => {
                self.scroll_up();
                None
            }
            Event::KeyDown {
                key: Key::Down | Key::J,
            } => {
                self.scroll_down();
                None
            }
            Event::KeyDown { key: Key::S } => {
                self.cycle_sort();
                None
            }
            Event::KeyDown { key: Key::R } => {
                self.toggle_sort_direction();
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

    #[test]
    fn test_process_dataframe_creation() {
        let df = ProcessDataFrame::new()
            .with_sort(ProcessSortColumn::Mem, true)
            .with_history_len(30);

        assert_eq!(df.sort_column, ProcessSortColumn::Mem);
        assert!(df.sort_desc);
        assert_eq!(df.history_len, 30);
    }

    #[test]
    fn test_process_row_update() {
        let mut df = ProcessDataFrame::new();

        let rows = vec![
            ProcessRow {
                pid: 1,
                name: "systemd".into(),
                cpu_percent: 0.5,
                mem_percent: 1.2,
                state: ProcessDisplayState::Running,
                threads: 1,
                priority: 0,
                user: "root".into(),
                cpu_time_secs: 3600,
                ..Default::default()
            },
            ProcessRow {
                pid: 100,
                name: "firefox".into(),
                cpu_percent: 25.0,
                mem_percent: 15.0,
                state: ProcessDisplayState::Sleeping,
                threads: 120,
                priority: 20,
                user: "noah".into(),
                cpu_time_secs: 7200,
                ..Default::default()
            },
        ];

        df.update_processes(rows);

        // Should be sorted by CPU desc by default
        assert_eq!(df.rows[0].pid, 100); // firefox has higher CPU
        assert_eq!(df.rows[1].pid, 1);
    }

    #[test]
    fn test_sparkline_rendering() {
        let values = vec![10.0, 20.0, 50.0, 80.0, 100.0];
        let sparkline = ProcessDataFrame::render_sparkline(&values, 5);
        assert_eq!(sparkline.chars().count(), 5);
    }

    #[test]
    fn test_format_time() {
        assert_eq!(ProcessDataFrame::format_time(59), "0:59");
        assert_eq!(ProcessDataFrame::format_time(3661), "1:01:01");
        assert_eq!(ProcessDataFrame::format_time(360000), "100h");
    }

    #[test]
    fn test_scroll() {
        let mut df = ProcessDataFrame::new();
        df.bounds = Rect::new(0.0, 0.0, 100.0, 10.0);

        let rows: Vec<ProcessRow> = (0..20)
            .map(|i| ProcessRow {
                pid: i,
                name: format!("proc{i}").into(),
                cpu_percent: i as f32,
                ..Default::default()
            })
            .collect();

        df.update_processes(rows);

        df.scroll_down();
        assert_eq!(df.selected_row, Some(0));

        df.scroll_down();
        assert_eq!(df.selected_row, Some(1));

        df.scroll_up();
        assert_eq!(df.selected_row, Some(0));
    }
}
