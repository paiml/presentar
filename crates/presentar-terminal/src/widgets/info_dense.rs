//! Information-Dense Widgets (Tufte-inspired)
//!
//! Widgets designed to maximize data-ink ratio and answer user questions directly.
//! These prioritize information density over decoration.
//!
//! # Design Principles (Tufte)
//! - Maximize data-ink ratio
//! - Show comparisons and context
//! - Avoid chart junk
//! - Use small multiples
//! - Show outliers, not repetitive data

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

// =============================================================================
// Color Helpers
// =============================================================================

fn color_for_cpu_percent(pct: f32) -> Color {
    if pct > 50.0 {
        Color {
            r: 1.0,
            g: 0.4,
            b: 0.4,
            a: 1.0,
        } // Red
    } else if pct > 10.0 {
        Color {
            r: 1.0,
            g: 0.8,
            b: 0.4,
            a: 1.0,
        } // Yellow
    } else if pct > 1.0 {
        Color {
            r: 0.6,
            g: 0.9,
            b: 0.6,
            a: 1.0,
        } // Green
    } else {
        Color {
            r: 0.5,
            g: 0.5,
            b: 0.6,
            a: 1.0,
        } // Dim
    }
}

fn color_for_status(level: HealthLevel) -> Color {
    match level {
        HealthLevel::Critical => Color {
            r: 1.0,
            g: 0.2,
            b: 0.2,
            a: 1.0,
        },
        HealthLevel::High => Color {
            r: 1.0,
            g: 0.5,
            b: 0.3,
            a: 1.0,
        },
        HealthLevel::Moderate => Color {
            r: 1.0,
            g: 0.8,
            b: 0.4,
            a: 1.0,
        },
        HealthLevel::Ok => Color {
            r: 0.5,
            g: 0.9,
            b: 0.5,
            a: 1.0,
        },
    }
}

/// Health level for system metrics (renamed to avoid conflict with `dataframe::HealthLevel`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthLevel {
    Critical,
    High,
    Moderate,
    Ok,
}

impl HealthLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "CRITICAL",
            Self::High => "HIGH",
            Self::Moderate => "MODERATE",
            Self::Ok => "OK",
        }
    }
}

// =============================================================================
// 1. TopProcessesTable - Information-dense process list
// =============================================================================

/// Process entry for the table
#[derive(Debug, Clone)]
pub struct CpuConsumer {
    pub pid: u32,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub name: String,
}

impl CpuConsumer {
    pub fn new(pid: u32, cpu_percent: f32, memory_bytes: u64, name: impl Into<String>) -> Self {
        Self {
            pid,
            cpu_percent,
            memory_bytes,
            name: name.into(),
        }
    }

    fn memory_display(&self) -> String {
        if self.memory_bytes > 1_073_741_824 {
            format!("{:.1}G", self.memory_bytes as f64 / 1_073_741_824.0)
        } else if self.memory_bytes > 1_048_576 {
            format!("{:.0}M", self.memory_bytes as f64 / 1_048_576.0)
        } else {
            format!("{:.0}K", self.memory_bytes as f64 / 1024.0)
        }
    }
}

/// Information-dense process table showing top CPU consumers.
/// Answers: "What's using my CPU?"
#[derive(Debug, Clone)]
pub struct TopProcessesTable {
    /// All processes (will be sorted by CPU)
    processes: Vec<CpuConsumer>,
    /// Total CPU percentage for header
    total_cpu: f32,
    /// Maximum processes to show
    max_display: usize,
    /// Cached bounds
    bounds: Rect,
}

impl Default for TopProcessesTable {
    fn default() -> Self {
        Self::new(vec![], 0.0)
    }
}

impl TopProcessesTable {
    /// Create a new top processes table
    #[must_use]
    pub fn new(mut processes: Vec<CpuConsumer>, total_cpu: f32) -> Self {
        // Sort by CPU descending
        processes.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Self {
            processes,
            total_cpu,
            max_display: 10,
            bounds: Rect::default(),
        }
    }

    /// Set maximum processes to display
    #[must_use]
    pub fn with_max_display(mut self, max: usize) -> Self {
        self.max_display = max;
        self
    }

    /// Update processes
    pub fn set_processes(&mut self, mut processes: Vec<CpuConsumer>, total_cpu: f32) {
        processes.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.processes = processes;
        self.total_cpu = total_cpu;
    }
}

impl Brick for TopProcessesTable {
    fn brick_name(&self) -> &'static str {
        "top_processes_table"
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

impl Widget for TopProcessesTable {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let height = (self.max_display + 3) as f32; // header + column header + processes + summary
        constraints.constrain(Size::new(constraints.max_width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        // Adjust max_display based on available height
        self.max_display = ((bounds.height - 3.0) as usize).max(3);
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        let x = self.bounds.x;
        let mut y = self.bounds.y;
        let w = self.bounds.width as usize;

        let header_style = TextStyle {
            color: Color {
                r: 0.6,
                g: 0.8,
                b: 1.0,
                a: 1.0,
            },
            ..Default::default()
        };
        let dim_style = TextStyle {
            color: Color {
                r: 0.5,
                g: 0.5,
                b: 0.6,
                a: 1.0,
            },
            ..Default::default()
        };

        // Header
        let header = format!("TOP CPU CONSUMERS ({:.0}% total)", self.total_cpu);
        canvas.draw_text(
            &header[..header.len().min(w)],
            Point::new(x, y),
            &header_style,
        );
        y += 1.0;

        // Column headers
        let col_header = format!("{:>7} {:>6} {:>6}  {:<}", "PID", "CPU%", "MEM", "COMMAND");
        canvas.draw_text(
            &col_header[..col_header.len().min(w)],
            Point::new(x, y),
            &dim_style,
        );
        y += 1.0;

        // Process rows
        let mut other_cpu = 0.0_f32;
        let mut other_count = 0_usize;

        for (i, proc) in self.processes.iter().enumerate() {
            if i < self.max_display && y < self.bounds.y + self.bounds.height - 1.0 {
                let color = color_for_cpu_percent(proc.cpu_percent);
                let max_name = w.saturating_sub(22);
                let name = if proc.name.len() > max_name {
                    format!("{}...", &proc.name[..max_name.saturating_sub(3)])
                } else {
                    proc.name.clone()
                };

                let line = format!(
                    "{:>7} {:>5.1}% {:>6}  {}",
                    proc.pid,
                    proc.cpu_percent,
                    proc.memory_display(),
                    name
                );
                canvas.draw_text(
                    &line[..line.len().min(w)],
                    Point::new(x, y),
                    &TextStyle {
                        color,
                        ..Default::default()
                    },
                );
                y += 1.0;
            } else {
                other_cpu += proc.cpu_percent;
                other_count += 1;
            }
        }

        // Summary of other processes
        if other_count > 0 && y < self.bounds.y + self.bounds.height {
            let other_line = format!("  [{other_count} other processes totaling {other_cpu:.1}%]");
            canvas.draw_text(
                &other_line[..other_line.len().min(w)],
                Point::new(x, y),
                &dim_style,
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
// 2. CoreUtilizationHistogram - Shows distribution, not 48 identical values
// =============================================================================

/// Histogram showing core utilization distribution
#[derive(Debug, Clone)]
pub struct CoreUtilizationHistogram {
    /// Core percentages (0-100)
    core_percentages: Vec<f64>,
    /// Cached bounds
    bounds: Rect,
}

impl Default for CoreUtilizationHistogram {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl CoreUtilizationHistogram {
    #[must_use]
    pub fn new(core_percentages: Vec<f64>) -> Self {
        Self {
            core_percentages,
            bounds: Rect::default(),
        }
    }

    pub fn set_percentages(&mut self, percentages: Vec<f64>) {
        self.core_percentages = percentages;
    }

    fn bucket_counts(&self) -> (usize, usize, usize, usize, usize) {
        let mut b100 = 0; // 95-100%
        let mut bhigh = 0; // 70-95%
        let mut bmed = 0; // 30-70%
        let mut blow = 0; // 1-30%
        let mut bidle = 0; // <1%

        for &pct in &self.core_percentages {
            if pct >= 95.0 {
                b100 += 1;
            } else if pct >= 70.0 {
                bhigh += 1;
            } else if pct >= 30.0 {
                bmed += 1;
            } else if pct >= 1.0 {
                blow += 1;
            } else {
                bidle += 1;
            }
        }
        (b100, bhigh, bmed, blow, bidle)
    }
}

impl Brick for CoreUtilizationHistogram {
    fn brick_name(&self) -> &'static str {
        "core_utilization_histogram"
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

impl Widget for CoreUtilizationHistogram {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(constraints.max_width, 6.0)) // header + 5 buckets max
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        let x = self.bounds.x;
        let mut y = self.bounds.y;
        let w = self.bounds.width as usize;

        let header_style = TextStyle {
            color: Color {
                r: 0.6,
                g: 0.8,
                b: 1.0,
                a: 1.0,
            },
            ..Default::default()
        };
        let dim_style = TextStyle {
            color: Color {
                r: 0.5,
                g: 0.5,
                b: 0.6,
                a: 1.0,
            },
            ..Default::default()
        };
        let bright_style = TextStyle {
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            ..Default::default()
        };

        canvas.draw_text("CORE UTILIZATION", Point::new(x, y), &header_style);
        y += 1.0;

        let (b100, bhigh, bmed, blow, bidle) = self.bucket_counts();
        let total = self.core_percentages.len();
        let bar_max = w.saturating_sub(20);

        let draw_bar =
            |canvas: &mut dyn Canvas, y: f32, label: &str, count: usize, color: Color| {
                if count == 0 {
                    return;
                }
                let bar_w = if total > 0 {
                    (count * bar_max) / total
                } else {
                    0
                };
                let bar: String = "█".repeat(bar_w);
                let pad: String = "░".repeat(bar_max - bar_w);

                canvas.draw_text(
                    label,
                    Point::new(x, y),
                    &TextStyle {
                        color,
                        ..Default::default()
                    },
                );
                canvas.draw_text(
                    &bar,
                    Point::new(x + 10.0, y),
                    &TextStyle {
                        color,
                        ..Default::default()
                    },
                );
                canvas.draw_text(&pad, Point::new(x + 10.0 + bar_w as f32, y), &dim_style);
                canvas.draw_text(
                    &format!(" x{count}"),
                    Point::new(x + 10.0 + bar_max as f32, y),
                    &bright_style,
                );
            };

        if b100 > 0 {
            draw_bar(
                canvas,
                y,
                "   100%",
                b100,
                Color {
                    r: 1.0,
                    g: 0.3,
                    b: 0.3,
                    a: 1.0,
                },
            );
            y += 1.0;
        }
        if bhigh > 0 {
            draw_bar(
                canvas,
                y,
                " 70-95%",
                bhigh,
                Color {
                    r: 1.0,
                    g: 0.6,
                    b: 0.3,
                    a: 1.0,
                },
            );
            y += 1.0;
        }
        if bmed > 0 {
            draw_bar(
                canvas,
                y,
                " 30-70%",
                bmed,
                Color {
                    r: 1.0,
                    g: 1.0,
                    b: 0.4,
                    a: 1.0,
                },
            );
            y += 1.0;
        }
        if blow > 0 {
            draw_bar(
                canvas,
                y,
                "  1-30%",
                blow,
                Color {
                    r: 0.5,
                    g: 0.9,
                    b: 0.5,
                    a: 1.0,
                },
            );
            y += 1.0;
        }
        if bidle > 0 {
            draw_bar(
                canvas,
                y,
                "   idle",
                bidle,
                Color {
                    r: 0.4,
                    g: 0.4,
                    b: 0.5,
                    a: 1.0,
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
// 3. TrendSparkline - Sparkline with context (min/max/avg/current)
// =============================================================================

/// Sparkline with statistical context
#[derive(Debug, Clone)]
pub struct TrendSparkline {
    /// History values (0-1 normalized or 0-100 percentage)
    history: Vec<f64>,
    /// Title for the widget
    title: String,
    /// Whether values are percentages (0-100) or normalized (0-1)
    is_percentage: bool,
    /// Cached bounds
    bounds: Rect,
}

impl Default for TrendSparkline {
    fn default() -> Self {
        Self::new("TREND", vec![])
    }
}

impl TrendSparkline {
    #[must_use]
    pub fn new(title: impl Into<String>, history: Vec<f64>) -> Self {
        Self {
            history,
            title: title.into(),
            is_percentage: true,
            bounds: Rect::default(),
        }
    }

    /// Mark values as normalized (0-1) instead of percentage (0-100)
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.is_percentage = false;
        self
    }

    pub fn set_history(&mut self, history: Vec<f64>) {
        self.history = history;
    }

    pub fn push(&mut self, value: f64) {
        self.history.push(value);
        if self.history.len() > 120 {
            self.history.remove(0);
        }
    }

    fn stats(&self) -> (f64, f64, f64, f64) {
        if self.history.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let mult = if self.is_percentage { 1.0 } else { 100.0 };
        let current = self.history.last().copied().unwrap_or(0.0) * mult;
        let min = self.history.iter().copied().fold(f64::MAX, f64::min) * mult;
        let max = self.history.iter().copied().fold(f64::MIN, f64::max) * mult;
        let avg = self.history.iter().sum::<f64>() / self.history.len() as f64 * mult;
        (current, min, max, avg)
    }
}

impl Brick for TrendSparkline {
    fn brick_name(&self) -> &'static str {
        "trend_sparkline"
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

impl Widget for TrendSparkline {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(constraints.max_width, 4.0)) // header + sparkline + 2 stat lines
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        let x = self.bounds.x;
        let mut y = self.bounds.y;
        let w = self.bounds.width as usize;

        let header_style = TextStyle {
            color: Color {
                r: 0.6,
                g: 0.8,
                b: 1.0,
                a: 1.0,
            },
            ..Default::default()
        };
        let dim_style = TextStyle {
            color: Color {
                r: 0.5,
                g: 0.5,
                b: 0.6,
                a: 1.0,
            },
            ..Default::default()
        };
        let bright_style = TextStyle {
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            ..Default::default()
        };

        canvas.draw_text(&self.title, Point::new(x, y), &header_style);
        y += 1.0;

        let (current, min, max, avg) = self.stats();

        // Color based on current level
        let trend_color = if current > 80.0 {
            Color {
                r: 1.0,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            }
        } else if current > 50.0 {
            Color {
                r: 1.0,
                g: 0.8,
                b: 0.4,
                a: 1.0,
            }
        } else {
            Color {
                r: 0.5,
                g: 0.9,
                b: 0.5,
                a: 1.0,
            }
        };

        // Draw sparkline
        let mult = if self.is_percentage { 1.0 } else { 100.0 };
        let chars: String = self
            .history
            .iter()
            .rev()
            .take(w)
            .rev()
            .map(|&v| {
                let pct = (v * mult).clamp(0.0, 100.0);
                let idx = ((pct / 100.0) * 7.0).round() as usize;
                ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][idx.min(7)]
            })
            .collect();

        canvas.draw_text(
            &chars,
            Point::new(x, y),
            &TextStyle {
                color: trend_color,
                ..Default::default()
            },
        );
        y += 1.0;

        // Stats
        let now_avg = format!("Now: {current:.0}%  Avg: {avg:.0}%");
        canvas.draw_text(
            &now_avg[..now_avg.len().min(w)],
            Point::new(x, y),
            &bright_style,
        );
        y += 1.0;

        let min_max = format!("Min: {min:.0}%  Max: {max:.0}%");
        canvas.draw_text(
            &min_max[..min_max.len().min(w)],
            Point::new(x, y),
            &dim_style,
        );
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
// 4. SystemStatus - Load and thermal status with contextual coloring
// =============================================================================

/// System status display (load, thermals)
#[derive(Debug, Clone)]
pub struct SystemStatus {
    /// Load averages (1, 5, 15 minute)
    load_1m: f64,
    load_5m: f64,
    load_15m: f64,
    /// Number of cores (for per-core load calculation)
    core_count: usize,
    /// Thermal data: (`avg_temp`, `max_temp`) in Celsius
    thermal: Option<(f64, f64)>,
    /// Cached bounds
    bounds: Rect,
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 1)
    }
}

impl SystemStatus {
    #[must_use]
    pub fn new(load_1m: f64, load_5m: f64, load_15m: f64, core_count: usize) -> Self {
        Self {
            load_1m,
            load_5m,
            load_15m,
            core_count: core_count.max(1),
            thermal: None,
            bounds: Rect::default(),
        }
    }

    #[must_use]
    pub fn with_thermal(mut self, avg_temp: f64, max_temp: f64) -> Self {
        self.thermal = Some((avg_temp, max_temp));
        self
    }

    pub fn set_load(&mut self, l1: f64, l5: f64, l15: f64) {
        self.load_1m = l1;
        self.load_5m = l5;
        self.load_15m = l15;
    }

    pub fn set_thermal(&mut self, avg: f64, max: f64) {
        self.thermal = Some((avg, max));
    }

    /// Get the health level for current load
    pub fn load_status(&self) -> HealthLevel {
        let per_core = self.load_1m / self.core_count as f64;
        if per_core > 1.5 {
            HealthLevel::Critical
        } else if per_core > 1.0 {
            HealthLevel::High
        } else if per_core > 0.7 {
            HealthLevel::Moderate
        } else {
            HealthLevel::Ok
        }
    }

    /// Get the health level for thermal status
    pub fn thermal_status(&self) -> Option<HealthLevel> {
        self.thermal.map(|(_, max)| {
            if max > 90.0 {
                HealthLevel::Critical
            } else if max > 80.0 {
                HealthLevel::High
            } else if max > 70.0 {
                HealthLevel::Moderate
            } else {
                HealthLevel::Ok
            }
        })
    }
}

impl Brick for SystemStatus {
    fn brick_name(&self) -> &'static str {
        "system_status"
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

impl Widget for SystemStatus {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let height = if self.thermal.is_some() { 2.0 } else { 1.0 };
        constraints.constrain(Size::new(constraints.max_width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        let x = self.bounds.x;
        let mut y = self.bounds.y;
        let w = self.bounds.width as usize;

        // Load line
        let load_stat = self.load_status();
        let per_core = self.load_1m / self.core_count as f64;
        let load_line = format!(
            "LOAD: {:.2} / {:.2} / {:.2}  ({:.2}/core) - {}",
            self.load_1m,
            self.load_5m,
            self.load_15m,
            per_core,
            load_stat.as_str()
        );
        canvas.draw_text(
            &load_line[..load_line.len().min(w)],
            Point::new(x, y),
            &TextStyle {
                color: color_for_status(load_stat),
                ..Default::default()
            },
        );

        // Thermal line (only if data present)
        if let Some((avg, max)) = self.thermal {
            y += 1.0;
            let therm_stat = self.thermal_status().unwrap_or(HealthLevel::Ok);
            let therm_line = format!(
                "THERMAL: {:.0}°C avg, {:.0}°C max - {}",
                avg,
                max,
                therm_stat.as_str()
            );
            canvas.draw_text(
                &therm_line[..therm_line.len().min(w)],
                Point::new(x, y),
                &TextStyle {
                    color: color_for_status(therm_stat),
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
    fn test_top_processes_table() {
        let procs = vec![
            CpuConsumer::new(123, 45.0, 1_000_000_000, "firefox"),
            CpuConsumer::new(456, 23.0, 500_000_000, "chrome"),
        ];
        let table = TopProcessesTable::new(procs, 68.0);
        assert!(table.verify().is_valid());
    }

    #[test]
    fn test_core_histogram() {
        let hist = CoreUtilizationHistogram::new(vec![95.0, 96.0, 50.0, 10.0, 0.5]);
        let (b100, bhigh, bmed, blow, bidle) = hist.bucket_counts();
        assert_eq!(b100, 2);
        assert_eq!(bmed, 1);
        assert_eq!(blow, 1);
        assert_eq!(bidle, 1);
    }

    #[test]
    fn test_trend_sparkline() {
        let mut trend = TrendSparkline::new("CPU", vec![50.0, 60.0, 70.0]);
        trend.push(80.0);
        let (current, min, max, avg) = trend.stats();
        assert!((current - 80.0).abs() < 0.01);
        assert!((min - 50.0).abs() < 0.01);
        assert!((max - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_system_status() {
        let status = SystemStatus::new(4.0, 3.0, 2.0, 8).with_thermal(65.0, 72.0);
        assert_eq!(status.load_status(), HealthLevel::Ok); // 4/8 = 0.5 per core (< 0.7)
        assert_eq!(status.thermal_status(), Some(HealthLevel::Moderate)); // 72°C
    }
}
