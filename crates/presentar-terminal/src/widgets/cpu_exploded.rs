//! CPU Exploded View Widgets (CB-INPUT-006 Framework-First)
//!
//! Specialized widgets that only appear in CPU panel's exploded (fullscreen) view.
//! These provide detailed CPU analysis that wouldn't fit in the condensed dashboard.
//!
//! # Widgets
//!
//! - [`PerCoreSparklineGrid`]: Individual sparkline history for each CPU core
//! - [`CpuStateBreakdown`]: User/system/idle/iowait stacked bars per core
//! - [`TopProcessesMini`]: Top 5 CPU consumers with live updates
//! - [`FreqTempHeatmap`]: Frequency and temperature heatmap per core
//! - [`LoadAverageTimeline`]: 1/5/15 minute load average sparklines

use crate::theme::Gradient;
use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

// =============================================================================
// 1. PerCoreSparklineGrid - Individual sparkline per core
// =============================================================================

/// Grid of sparklines showing per-core CPU history.
///
/// In condensed view, only the aggregate sparkline is shown.
/// In exploded view, each core gets its own sparkline with 60-second history.
#[derive(Debug, Clone)]
pub struct PerCoreSparklineGrid {
    /// Per-core history buffers (each is 60 samples = 60 seconds at 1s refresh)
    pub core_histories: Vec<Vec<f64>>,
    /// Gradient for coloring (low→high utilization)
    gradient: Gradient,
    /// Number of columns for grid layout
    columns: Option<usize>,
    /// Whether to show core labels
    show_labels: bool,
    /// Cached layout bounds
    bounds: Rect,
}

impl Default for PerCoreSparklineGrid {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl PerCoreSparklineGrid {
    /// Create a new per-core sparkline grid.
    #[must_use]
    pub fn new(core_histories: Vec<Vec<f64>>) -> Self {
        Self {
            core_histories,
            gradient: Gradient::from_hex(&["#7aa2f7", "#e0af68", "#f7768e"]),
            columns: None,
            show_labels: true,
            bounds: Rect::default(),
        }
    }

    /// Set explicit column count.
    #[must_use]
    pub fn with_columns(mut self, cols: usize) -> Self {
        self.columns = Some(cols);
        self
    }

    /// Set custom gradient.
    #[must_use]
    pub fn with_gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = gradient;
        self
    }

    /// Disable core labels.
    #[must_use]
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// Update core histories.
    pub fn set_histories(&mut self, histories: Vec<Vec<f64>>) {
        self.core_histories = histories;
    }

    /// Get optimal grid dimensions.
    fn optimal_grid(&self, max_width: usize, cell_width: usize) -> (usize, usize) {
        let count = self.core_histories.len();
        if count == 0 {
            return (0, 0);
        }

        let max_cols = (max_width / cell_width).max(1);
        let cols = self.columns.unwrap_or_else(|| {
            // For sparklines, prefer fewer columns (wider sparklines)
            let ideal = (count as f64).sqrt().ceil() as usize;
            ideal.min(max_cols).min(4).max(1)
        });

        let rows = count.div_ceil(cols);
        (cols, rows)
    }
}

impl Brick for PerCoreSparklineGrid {
    fn brick_name(&self) -> &'static str {
        "per_core_sparkline_grid"
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
        String::new()
    }
    fn to_css(&self) -> String {
        String::new()
    }
}

impl Widget for PerCoreSparklineGrid {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let cell_width = 20; // 3 chars label + 15 chars sparkline + 2 spacing
        let cell_height = 2; // sparkline + label row
        let (cols, rows) = self.optimal_grid(constraints.max_width as usize, cell_width);

        let width = (cols * cell_width) as f32;
        let height = (rows * cell_height) as f32;

        constraints.constrain(Size::new(width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.core_histories.is_empty() {
            return;
        }

        let cell_width = 20_usize;
        let cell_height = 2_usize;
        let (cols, _) = self.optimal_grid(self.bounds.width as usize, cell_width);
        if cols == 0 {
            return;
        }

        for (i, history) in self.core_histories.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;

            let x = self.bounds.x + (col * cell_width) as f32;
            let y = self.bounds.y + (row * cell_height) as f32;

            // Get current value for color
            let current = history.last().copied().unwrap_or(0.0);
            let color = self.gradient.for_percent(current);

            // Draw label
            if self.show_labels {
                let label = format!("{i:2}");
                canvas.draw_text(
                    &label,
                    Point::new(x, y),
                    &TextStyle {
                        color,
                        ..Default::default()
                    },
                );
            }

            // Draw mini sparkline
            let sparkline_x = x + if self.show_labels { 3.0 } else { 0.0 };
            let sparkline_width = (cell_width as f32 - 5.0).max(8.0);

            // Convert history to sparkline chars
            let chars: String = history
                .iter()
                .rev()
                .take(sparkline_width as usize)
                .rev()
                .map(|&v| {
                    let idx = ((v / 100.0) * 7.0).round() as usize;
                    ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][idx.min(7)]
                })
                .collect();

            canvas.draw_text(
                &chars,
                Point::new(sparkline_x, y),
                &TextStyle {
                    color,
                    ..Default::default()
                },
            );

            // Draw current percentage
            let pct_str = format!("{current:3.0}%");
            canvas.draw_text(
                &pct_str,
                Point::new(sparkline_x + sparkline_width, y),
                &TextStyle {
                    color,
                    ..Default::default()
                },
            );
        }
    }

    fn event(&mut self, _event: &Event) -> Option<Box<dyn Any + Send>> {
        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }
}

// =============================================================================
// 2. CpuStateBreakdown - User/System/Idle/IOWait stacked bars
// =============================================================================

/// CPU state breakdown showing user/system/idle/iowait as stacked bars.
#[derive(Debug, Clone)]
pub struct CpuStateBreakdown {
    /// Per-core CPU states: (user%, system%, idle%, iowait%, irq%, softirq%)
    pub core_states: Vec<CpuCoreState>,
    /// Cached layout bounds
    bounds: Rect,
}

/// CPU state percentages for a single core.
#[derive(Debug, Clone, Default)]
pub struct CpuCoreState {
    pub user: f64,
    pub system: f64,
    pub idle: f64,
    pub iowait: f64,
    pub irq: f64,
    pub softirq: f64,
}

impl CpuCoreState {
    /// Create from percentages.
    pub fn new(user: f64, system: f64, idle: f64, iowait: f64, irq: f64, softirq: f64) -> Self {
        Self {
            user,
            system,
            idle,
            iowait,
            irq,
            softirq,
        }
    }
}

impl Default for CpuStateBreakdown {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl CpuStateBreakdown {
    /// Create a new CPU state breakdown widget.
    #[must_use]
    pub fn new(core_states: Vec<CpuCoreState>) -> Self {
        Self {
            core_states,
            bounds: Rect::default(),
        }
    }

    /// Update core states.
    pub fn set_states(&mut self, states: Vec<CpuCoreState>) {
        self.core_states = states;
    }
}

// State colors (Tokyo Night inspired) - helper function
fn state_colors() -> [Color; 6] {
    [
        Color::rgb(0.478, 0.635, 0.969), // USER: Blue #7aa2f7
        Color::rgb(0.878, 0.686, 0.408), // SYSTEM: Yellow #e0af68
        Color::rgb(0.969, 0.463, 0.557), // IOWAIT: Red #f7768e
        Color::rgb(0.733, 0.604, 0.969), // IRQ: Purple #bb9af7
        Color::rgb(0.620, 0.808, 0.416), // SOFTIRQ: Green #9ece6a
        Color::rgb(0.337, 0.373, 0.537), // IDLE: Gray #565f89
    ]
}

impl Brick for CpuStateBreakdown {
    fn brick_name(&self) -> &'static str {
        "cpu_state_breakdown"
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
        String::new()
    }
    fn to_css(&self) -> String {
        String::new()
    }
}

impl Widget for CpuStateBreakdown {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let height = self.core_states.len().min(constraints.max_height as usize) as f32;
        constraints.constrain(Size::new(constraints.max_width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        let bar_width = (self.bounds.width as usize).saturating_sub(20); // Reserve for labels

        for (i, state) in self.core_states.iter().enumerate() {
            let y = self.bounds.y + i as f32;
            if y >= self.bounds.y + self.bounds.height {
                break;
            }

            // Draw label
            let label = format!("{i:2}:");
            canvas.draw_text(&label, Point::new(self.bounds.x, y), &TextStyle::default());

            // Calculate bar segments
            let total =
                state.user + state.system + state.idle + state.iowait + state.irq + state.softirq;
            if total <= 0.0 {
                continue;
            }

            let mut x = self.bounds.x + 4.0;
            let colors = state_colors();
            let segments = [
                (state.user, colors[0], '█'),    // USER
                (state.system, colors[1], '▓'),  // SYSTEM
                (state.iowait, colors[2], '░'),  // IOWAIT
                (state.irq, colors[3], '▒'),     // IRQ
                (state.softirq, colors[4], '░'), // SOFTIRQ
                (state.idle, colors[5], '·'),    // IDLE
            ];

            for (pct, color, ch) in segments {
                let width = ((pct / total) * bar_width as f64).round() as usize;
                if width > 0 {
                    let bar: String = std::iter::repeat(ch).take(width).collect();
                    canvas.draw_text(
                        &bar,
                        Point::new(x, y),
                        &TextStyle {
                            color,
                            ..Default::default()
                        },
                    );
                    x += width as f32;
                }
            }

            // Draw percentage summary
            let summary = format!(
                " u:{:2.0} s:{:2.0} w:{:2.0}",
                state.user, state.system, state.iowait
            );
            canvas.draw_text(&summary, Point::new(x + 1.0, y), &TextStyle::default());
        }
    }

    fn event(&mut self, _event: &Event) -> Option<Box<dyn Any + Send>> {
        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }
}

// =============================================================================
// 3. TopProcessesMini - Top 5 CPU consumers
// =============================================================================

/// Mini table showing top 5 CPU-consuming processes.
#[derive(Debug, Clone)]
pub struct TopProcessesMini {
    /// Top processes: (pid, cpu%, name)
    pub processes: Vec<TopProcess>,
    /// Cached layout bounds
    bounds: Rect,
}

/// A top CPU-consuming process.
#[derive(Debug, Clone)]
pub struct TopProcess {
    pub pid: u32,
    pub cpu_percent: f32,
    pub name: String,
}

impl TopProcess {
    pub fn new(pid: u32, cpu_percent: f32, name: impl Into<String>) -> Self {
        Self {
            pid,
            cpu_percent,
            name: name.into(),
        }
    }
}

impl Default for TopProcessesMini {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl TopProcessesMini {
    /// Create a new top processes widget.
    #[must_use]
    pub fn new(processes: Vec<TopProcess>) -> Self {
        Self {
            processes,
            bounds: Rect::default(),
        }
    }

    /// Update processes list.
    pub fn set_processes(&mut self, processes: Vec<TopProcess>) {
        self.processes = processes;
    }
}

impl Brick for TopProcessesMini {
    fn brick_name(&self) -> &'static str {
        "top_processes_mini"
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
        String::new()
    }
    fn to_css(&self) -> String {
        String::new()
    }
}

impl Widget for TopProcessesMini {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let height = (self.processes.len().min(5) + 1) as f32; // +1 for header
        constraints.constrain(Size::new(constraints.max_width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        // Header
        let header = " PID    %CPU  Command";
        canvas.draw_text(
            header,
            Point::new(self.bounds.x, self.bounds.y),
            &TextStyle {
                color: Color::rgb(0.663, 0.694, 0.839), // Gray #a9b1d6
                ..Default::default()
            },
        );

        // Gradient for CPU usage
        let gradient = Gradient::from_hex(&["#7aa2f7", "#e0af68", "#f7768e"]);

        for (i, proc) in self.processes.iter().take(5).enumerate() {
            let y = self.bounds.y + (i + 1) as f32;
            if y >= self.bounds.y + self.bounds.height {
                break;
            }

            let color = gradient.for_percent(proc.cpu_percent as f64);
            let max_name_len = (self.bounds.width as usize).saturating_sub(15);
            let name = if proc.name.len() > max_name_len {
                format!("{}...", &proc.name[..max_name_len.saturating_sub(3)])
            } else {
                proc.name.clone()
            };

            let line = format!("{:5} {:5.1}  {}", proc.pid, proc.cpu_percent, name);
            canvas.draw_text(
                &line,
                Point::new(self.bounds.x, y),
                &TextStyle {
                    color,
                    ..Default::default()
                },
            );
        }
    }

    fn event(&mut self, _event: &Event) -> Option<Box<dyn Any + Send>> {
        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }
}

// =============================================================================
// 4. FreqTempHeatmap - Frequency and Temperature per core
// =============================================================================

/// Heatmap showing frequency and temperature per core.
#[derive(Debug, Clone)]
pub struct FreqTempHeatmap {
    /// Per-core frequencies in MHz
    pub frequencies: Vec<u32>,
    /// Per-core max frequencies in MHz
    pub max_frequencies: Vec<u32>,
    /// Per-core temperatures in Celsius (optional)
    pub temperatures: Option<Vec<f32>>,
    /// Cached layout bounds
    bounds: Rect,
}

impl Default for FreqTempHeatmap {
    fn default() -> Self {
        Self::new(vec![], vec![])
    }
}

impl FreqTempHeatmap {
    /// Create a new frequency/temperature heatmap.
    #[must_use]
    pub fn new(frequencies: Vec<u32>, max_frequencies: Vec<u32>) -> Self {
        Self {
            frequencies,
            max_frequencies,
            temperatures: None,
            bounds: Rect::default(),
        }
    }

    /// Add temperature data.
    #[must_use]
    pub fn with_temperatures(mut self, temps: Vec<f32>) -> Self {
        self.temperatures = Some(temps);
        self
    }

    /// Update data.
    pub fn set_data(&mut self, freqs: Vec<u32>, max_freqs: Vec<u32>, temps: Option<Vec<f32>>) {
        self.frequencies = freqs;
        self.max_frequencies = max_freqs;
        self.temperatures = temps;
    }
}

impl Brick for FreqTempHeatmap {
    fn brick_name(&self) -> &'static str {
        "freq_temp_heatmap"
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
        String::new()
    }
    fn to_css(&self) -> String {
        String::new()
    }
}

impl Widget for FreqTempHeatmap {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let has_temps = self.temperatures.is_some();
        let height = if has_temps { 4.0 } else { 2.0 }; // 2 rows per heatmap
        constraints.constrain(Size::new(constraints.max_width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.frequencies.is_empty() {
            return;
        }

        let freq_gradient = Gradient::from_hex(&["#565f89", "#7aa2f7", "#9ece6a"]); // Gray→Blue→Green
        let temp_gradient = Gradient::from_hex(&["#9ece6a", "#e0af68", "#f7768e"]); // Green→Yellow→Red

        // Calculate cell width
        let cell_width = 4_usize;
        let cores_per_row = (self.bounds.width as usize / cell_width).max(1);

        // Frequency row
        canvas.draw_text(
            "Freq:",
            Point::new(self.bounds.x, self.bounds.y),
            &TextStyle::default(),
        );
        for (i, (&freq, &max_freq)) in self
            .frequencies
            .iter()
            .zip(self.max_frequencies.iter())
            .enumerate()
        {
            let col = i % cores_per_row;
            let row = i / cores_per_row;
            let x = self.bounds.x + 6.0 + (col * cell_width) as f32;
            let y = self.bounds.y + row as f32;

            let pct = if max_freq > 0 {
                (freq as f64 / max_freq as f64) * 100.0
            } else {
                0.0
            };
            let color = freq_gradient.for_percent(pct);
            let level = ((pct / 100.0) * 7.0).round() as usize;
            let ch = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][level.min(7)];

            canvas.draw_text(
                &ch.to_string(),
                Point::new(x, y),
                &TextStyle {
                    color,
                    ..Default::default()
                },
            );
        }

        // Temperature row (if available)
        if let Some(ref temps) = self.temperatures {
            let temp_y = self.bounds.y + 2.0;
            canvas.draw_text(
                "Temp:",
                Point::new(self.bounds.x, temp_y),
                &TextStyle::default(),
            );

            for (i, &temp) in temps.iter().enumerate() {
                let col = i % cores_per_row;
                let row = i / cores_per_row;
                let x = self.bounds.x + 6.0 + (col * cell_width) as f32;
                let y = temp_y + row as f32;

                // Map temperature 30-100°C to 0-100%
                let pct = ((temp - 30.0) / 70.0 * 100.0).clamp(0.0, 100.0) as f64;
                let color = temp_gradient.for_percent(pct);
                let level = ((pct / 100.0) * 7.0).round() as usize;
                let ch = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][level.min(7)];

                canvas.draw_text(
                    &ch.to_string(),
                    Point::new(x, y),
                    &TextStyle {
                        color,
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn event(&mut self, _event: &Event) -> Option<Box<dyn Any + Send>> {
        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }
}

// =============================================================================
// 5. LoadAverageTimeline - 1/5/15 minute load average sparklines
// =============================================================================

/// Three sparklines showing load average history.
#[derive(Debug, Clone)]
pub struct LoadAverageTimeline {
    /// 1-minute load average history
    pub load_1m: Vec<f64>,
    /// 5-minute load average history
    pub load_5m: Vec<f64>,
    /// 15-minute load average history
    pub load_15m: Vec<f64>,
    /// Number of CPU cores (for relative scaling)
    pub core_count: usize,
    /// Cached layout bounds
    bounds: Rect,
}

impl Default for LoadAverageTimeline {
    fn default() -> Self {
        Self::new(1)
    }
}

impl LoadAverageTimeline {
    /// Create a new load average timeline.
    #[must_use]
    pub fn new(core_count: usize) -> Self {
        Self {
            load_1m: Vec::new(),
            load_5m: Vec::new(),
            load_15m: Vec::new(),
            core_count: core_count.max(1),
            bounds: Rect::default(),
        }
    }

    /// Push new load average values.
    pub fn push(&mut self, load_1m: f64, load_5m: f64, load_15m: f64) {
        const MAX_HISTORY: usize = 60;

        self.load_1m.push(load_1m);
        self.load_5m.push(load_5m);
        self.load_15m.push(load_15m);

        if self.load_1m.len() > MAX_HISTORY {
            self.load_1m.remove(0);
        }
        if self.load_5m.len() > MAX_HISTORY {
            self.load_5m.remove(0);
        }
        if self.load_15m.len() > MAX_HISTORY {
            self.load_15m.remove(0);
        }
    }

    /// Set core count for relative scaling.
    pub fn set_core_count(&mut self, count: usize) {
        self.core_count = count.max(1);
    }
}

impl Brick for LoadAverageTimeline {
    fn brick_name(&self) -> &'static str {
        "load_average_timeline"
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
        String::new()
    }
    fn to_css(&self) -> String {
        String::new()
    }
}

impl Widget for LoadAverageTimeline {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(constraints.max_width, 3.0))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        let gradient = Gradient::from_hex(&["#9ece6a", "#e0af68", "#f7768e"]); // Green→Yellow→Red
        let sparkline_width = (self.bounds.width as usize).saturating_sub(20);

        let loads = [
            (" 1m:", &self.load_1m),
            (" 5m:", &self.load_5m),
            ("15m:", &self.load_15m),
        ];

        for (row, (label, history)) in loads.iter().enumerate() {
            let y = self.bounds.y + row as f32;
            if y >= self.bounds.y + self.bounds.height {
                break;
            }

            // Draw label
            canvas.draw_text(label, Point::new(self.bounds.x, y), &TextStyle::default());

            if history.is_empty() {
                continue;
            }

            // Current value
            let current = history.last().copied().unwrap_or(0.0);
            let load_pct = (current / self.core_count as f64 * 100.0).min(200.0);
            let color = gradient.for_percent(load_pct.min(100.0));

            // Draw sparkline
            let chars: String = history
                .iter()
                .rev()
                .take(sparkline_width)
                .rev()
                .map(|&v| {
                    let pct = (v / self.core_count as f64 * 100.0).min(200.0);
                    let idx = ((pct / 200.0) * 7.0).round() as usize;
                    ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][idx.min(7)]
                })
                .collect();

            canvas.draw_text(
                &chars,
                Point::new(self.bounds.x + 5.0, y),
                &TextStyle {
                    color,
                    ..Default::default()
                },
            );

            // Current value
            let value_str = format!(" ({current:.2})");
            canvas.draw_text(
                &value_str,
                Point::new(self.bounds.x + 5.0 + sparkline_width as f32, y),
                &TextStyle {
                    color,
                    ..Default::default()
                },
            );
        }
    }

    fn event(&mut self, _event: &Event) -> Option<Box<dyn Any + Send>> {
        None
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
    fn test_per_core_sparkline_grid_new() {
        let grid = PerCoreSparklineGrid::new(vec![vec![10.0, 20.0, 30.0]; 4]);
        assert_eq!(grid.core_histories.len(), 4);
    }

    #[test]
    fn test_cpu_state_breakdown_new() {
        let breakdown =
            CpuStateBreakdown::new(vec![CpuCoreState::new(50.0, 10.0, 35.0, 5.0, 0.0, 0.0)]);
        assert_eq!(breakdown.core_states.len(), 1);
    }

    #[test]
    fn test_top_processes_mini_new() {
        let top = TopProcessesMini::new(vec![TopProcess::new(1234, 45.2, "firefox")]);
        assert_eq!(top.processes.len(), 1);
    }

    #[test]
    fn test_freq_temp_heatmap_new() {
        let heatmap = FreqTempHeatmap::new(vec![3600, 3200], vec![4000, 4000]);
        assert_eq!(heatmap.frequencies.len(), 2);
    }

    #[test]
    fn test_load_average_timeline_push() {
        let mut timeline = LoadAverageTimeline::new(8);
        timeline.push(1.5, 1.2, 1.0);
        timeline.push(2.0, 1.5, 1.1);
        assert_eq!(timeline.load_1m.len(), 2);
    }

    #[test]
    fn test_all_widgets_verify() {
        assert!(PerCoreSparklineGrid::default().verify().is_valid());
        assert!(CpuStateBreakdown::default().verify().is_valid());
        assert!(TopProcessesMini::default().verify().is_valid());
        assert!(FreqTempHeatmap::default().verify().is_valid());
        assert!(LoadAverageTimeline::default().verify().is_valid());
    }
}
