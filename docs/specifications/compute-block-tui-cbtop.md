# ComputeBlock TUI Specification (cbtop)

**Version:** 1.0.0
**Status:** DRAFT
**Date:** 2026-01-09

## Abstract

This specification defines `cbtop` - a terminal-based ComputeBlock monitoring system built on the `presentar-terminal` direct rendering backend. It provides real-time visualization of compute resources, ML workloads, and system metrics using the zero-allocation TUI architecture from PROBAR-SPEC-009.

## 1. Overview

### 1.1 Purpose

`cbtop` is the canonical monitoring application for the Sovereign AI Stack, providing:

- Real-time system resource monitoring (CPU, Memory, Network, Disk)
- ML training and inference metrics visualization
- GPU compute utilization tracking
- Batch job and pipeline progress monitoring
- Kubernetes/cluster health dashboards

### 1.2 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    cbtop Application                        │
├─────────────────────────────────────────────────────────────┤
│  Collectors        │  Widgets           │  Renderers        │
│  ├─ CpuCollector   │  ├─ BrailleGraph   │  ├─ CellBuffer    │
│  ├─ MemCollector   │  ├─ Meter          │  ├─ DiffRenderer  │
│  ├─ GpuCollector   │  ├─ Table          │  └─ ColorMode     │
│  ├─ NetCollector   │  └─ (custom)       │                   │
│  └─ TrainCollector │                    │                   │
├─────────────────────────────────────────────────────────────┤
│                  presentar-terminal                         │
│  DirectTerminalCanvas → crossterm → Terminal I/O            │
└─────────────────────────────────────────────────────────────┘
```

## 2. Core Components

### 2.1 Widget Types

All widgets implement `Brick + Widget` traits with falsifiable assertions.

#### 2.1.1 BrailleGraph

Time-series visualization using Unicode braille patterns (U+2800-28FF).

```rust
pub struct BrailleGraph {
    data: Vec<f64>,
    color: Color,
    min: f64,
    max: f64,
    mode: GraphMode,  // Braille | Block | Tty
}
```

**Render Modes:**
- `Braille`: 2×4 dots per cell (highest resolution)
- `Block`: Half-block characters (▀▄█)
- `Tty`: ASCII-only (`*` characters)

#### 2.1.2 Meter

Horizontal progress/gauge widget with optional gradient.

```rust
pub struct Meter {
    value: f64,
    max: f64,
    label: String,
    fill_color: Color,
    gradient_end: Option<Color>,
}
```

#### 2.1.3 Table

Sortable, scrollable data table with column alignment.

### 2.2 Color Coding Standards

| Metric Range | Color | RGB |
|--------------|-------|-----|
| Critical (>90%) | Red | (1.0, 0.3, 0.3) |
| Warning (>70%) | Orange | (1.0, 0.7, 0.2) |
| Elevated (>50%) | Yellow | (1.0, 1.0, 0.3) |
| Normal (<50%) | Green | (0.3, 1.0, 0.5) |
| Idle (<10%) | Gray | (0.5, 0.5, 0.5) |

## 3. Example Applications

### 3.1 System Monitoring Examples

| Example | Description | Key Widgets |
|---------|-------------|-------------|
| `cpu_monitor` | Per-core CPU with history | BrailleGraph, Meter bars |
| `memory_monitor` | RAM/Swap with breakdown | Graph, stacked bars |
| `network_traffic` | RX/TX per interface | Dual graphs, table |
| `system_dashboard` | Combined btop-style view | All widgets |

### 3.2 ML/Data Science Examples

| Example | Description | Key Widgets |
|---------|-------------|-------------|
| `training_metrics` | Loss/accuracy curves | Dual graphs, legend |
| `gpu_compute` | GPU utilization/VRAM | Per-GPU meters, temp |
| `inference_server` | Request latency/queue | P50/P99 graphs |
| `batch_progress` | Pipeline job progress | Progress bars, ETA |

### 3.3 Infrastructure Examples

| Example | Description | Key Widgets |
|---------|-------------|-------------|
| `queue_monitor` | Message queue depth | Table, throughput |
| `cluster_status` | Kubernetes nodes/pods | Node table, graphs |
| `sensor_dashboard` | IoT sensor readings | Multi-metric graphs |

## 4. Usage Patterns

### 4.1 Basic Graph Usage

```rust
use presentar_terminal::{BrailleGraph, GraphMode};

let mut graph = BrailleGraph::new(history_data)
    .with_color(Color::new(0.3, 0.9, 0.5, 1.0))
    .with_range(0.0, 100.0)
    .with_mode(GraphMode::Braille);

graph.layout(bounds);
graph.paint(&mut canvas);
```

### 4.2 Meter with Gradient

```rust
use presentar_terminal::Meter;

let meter = Meter::percentage(75.0)
    .with_label("CPU")
    .with_gradient(Color::GREEN, Color::RED);
```

### 4.3 Real-time Updates

```rust
// Push new data point
graph.push(new_value);

// Or replace entire dataset
graph.set_data(new_history);
```

## 5. Performance Requirements

Per PROBAR-SPEC-009 falsification checklist:

| Metric | Target | Tolerance |
|--------|--------|-----------|
| Full 80×24 redraw | <1ms | <50ms (coverage) |
| 10% differential update | <0.1ms | <5ms (coverage) |
| Memory (80×24) | <100KB | - |
| Steady-state allocations | 0 | - |

## 6. Running Examples

```bash
# System monitoring
cargo run -p presentar-terminal --example cpu_monitor
cargo run -p presentar-terminal --example memory_monitor
cargo run -p presentar-terminal --example network_traffic
cargo run -p presentar-terminal --example system_dashboard

# ML/Data Science
cargo run -p presentar-terminal --example training_metrics
cargo run -p presentar-terminal --example gpu_compute
cargo run -p presentar-terminal --example inference_server
cargo run -p presentar-terminal --example batch_progress

# Infrastructure
cargo run -p presentar-terminal --example queue_monitor
cargo run -p presentar-terminal --example cluster_status
cargo run -p presentar-terminal --example sensor_dashboard
```

## 7. trueno-viz and trueno Integration

### 7.1 Pure TUI/WASM Primitive Mapping

The following table maps trueno-viz primitives to pure TUI/WASM constructs:

| trueno-viz Type | presentar-terminal TUI | WASM Support |
|-----------------|------------------------|--------------|
| `plots::ScatterPlot` | BrailleGraph (scatter mode) | ✅ |
| `plots::LineChart` | BrailleGraph (line mode) | ✅ |
| `plots::Histogram` | VerticalBarChart | ✅ |
| `plots::Heatmap` | TuiHeatmap (block chars) | ✅ |
| `plots::LossCurve` | BrailleGraph (multi-series) | ✅ |
| `plots::RocCurve` | BrailleGraph (curve mode) | ✅ |
| `plots::PrCurve` | BrailleGraph (curve mode) | ✅ |
| `plots::ConfusionMatrix` | TuiConfusionMatrix | ✅ |
| `plots::BoxPlot` | TuiBoxPlot (ASCII art) | ✅ |
| `plots::ForceGraph` | TuiTree (hierarchical) | ✅ |
| `monitor::Graph` | BrailleGraph | ✅ |
| `monitor::Meter` | Meter | ✅ |
| `monitor::Gauge` | TuiGauge (arc chars) | ✅ |
| `monitor::Table` | Table | ✅ |
| `monitor::Tree` | TuiTree | ✅ |
| `monitor::Sparkline` | TuiSparkline | ✅ |
| `monitor::Heatmap` | TuiHeatmap | ✅ |
| `widgets::ResourceBar` | Meter | ✅ |
| `widgets::RunTable` | Table | ✅ |

### 7.2 trueno SIMD Integration

All TUI widgets can leverage trueno's SIMD-accelerated operations for data processing:

```rust
use trueno::prelude::*;
use presentar_terminal::BrailleGraph;

// SIMD-accelerated data transformation
fn process_metrics(raw: &[f64]) -> Vec<f64> {
    let vec = Vector::from_slice(raw);

    // SIMD normalization
    let min = vec.min();
    let max = vec.max();
    let normalized = vec.sub_scalar(min).div_scalar(max - min);

    normalized.to_vec()
}

// Use processed data in TUI widget
let graph = BrailleGraph::new(process_metrics(&cpu_samples))
    .with_mode(GraphMode::Braille);
```

### 7.3 WASM-First Architecture

All cbtop primitives compile to `wasm32-unknown-unknown`:

```rust
#[cfg(target_arch = "wasm32")]
pub fn render_to_canvas(widget: &impl Widget, canvas_id: &str) {
    // Direct WebGL/Canvas2D rendering
}

#[cfg(not(target_arch = "wasm32"))]
pub fn render_to_terminal(widget: &impl Widget, stdout: &mut impl Write) {
    // crossterm-based rendering
}
```

### 7.4 Missing Widget Implementations

The following widgets need implementation in presentar-terminal:

#### 7.4.1 TuiGauge (Arc/Circular)

```rust
/// Arc gauge using Unicode box-drawing characters.
pub struct TuiGauge {
    value: f64,
    max: f64,
    radius: u16,
    label: Option<String>,
}

impl TuiGauge {
    /// Render using arc characters: ╭─╮ ╰─╯ │
    fn render_arc(&self, canvas: &mut impl Canvas, center: Point) {
        // Arc approximation using box-drawing
        let chars = ['╭', '─', '╮', '│', '╰', '─', '╯'];
        // ...
    }
}
```

#### 7.4.2 TuiTree (Collapsible Hierarchy)

```rust
/// Tree view for process/cluster hierarchies.
pub struct TuiTree<T> {
    root: TreeNode<T>,
    expanded: HashSet<NodeId>,
}

impl<T: Display> TuiTree<T> {
    /// Render with tree characters: ├── └── │
    fn render_node(&self, canvas: &mut impl Canvas, node: &TreeNode<T>, depth: u16) {
        let prefix = if node.is_last { "└── " } else { "├── " };
        // ...
    }
}
```

#### 7.4.3 TuiConfusionMatrix

```rust
/// Confusion matrix visualization.
pub struct TuiConfusionMatrix {
    matrix: Vec<Vec<u64>>,
    labels: Vec<String>,
    normalization: Normalization,
}

impl TuiConfusionMatrix {
    /// Render as colored grid with values.
    fn render(&self, canvas: &mut impl Canvas, bounds: Rect) {
        for (i, row) in self.matrix.iter().enumerate() {
            for (j, &value) in row.iter().enumerate() {
                let color = self.value_color(value);
                // Draw cell with value
            }
        }
    }
}
```

#### 7.4.4 TuiBoxPlot

```rust
/// Box plot using ASCII art.
pub struct TuiBoxPlot {
    stats: Vec<BoxStats>,
    labels: Vec<String>,
    orientation: Orientation,
}

impl TuiBoxPlot {
    /// Render: ├──[████|████]──┤
    fn render_horizontal(&self, canvas: &mut impl Canvas, y: f32, stats: &BoxStats) {
        // Whiskers: ├──
        // Box: [████
        // Median: |
        // Box: ████]
        // Whiskers: ──┤
    }
}
```

### 7.5 Shared Color Palettes

Both trueno-viz and presentar-terminal use consistent color schemes:

```rust
/// Viridis-like palette for heatmaps (TUI-safe).
pub const VIRIDIS_TUI: [Color; 8] = [
    Color::new(0.27, 0.00, 0.33, 1.0), // Dark purple
    Color::new(0.28, 0.14, 0.45, 1.0),
    Color::new(0.26, 0.24, 0.53, 1.0),
    Color::new(0.22, 0.34, 0.55, 1.0),
    Color::new(0.18, 0.44, 0.56, 1.0),
    Color::new(0.12, 0.56, 0.55, 1.0),
    Color::new(0.20, 0.72, 0.47, 1.0),
    Color::new(0.99, 0.91, 0.15, 1.0), // Yellow
];

/// Plasma palette for diverging data.
pub const PLASMA_TUI: [Color; 8] = [
    Color::new(0.05, 0.03, 0.53, 1.0), // Dark blue
    Color::new(0.42, 0.05, 0.68, 1.0),
    Color::new(0.70, 0.08, 0.64, 1.0),
    Color::new(0.89, 0.27, 0.50, 1.0),
    Color::new(0.98, 0.50, 0.30, 1.0),
    Color::new(0.99, 0.70, 0.17, 1.0),
    Color::new(0.94, 0.89, 0.26, 1.0),
    Color::new(0.94, 0.98, 0.56, 1.0), // Light yellow
];
```

### 7.6 Backend Dispatch

```rust
/// Automatic backend selection for optimal performance.
pub enum RenderBackend {
    /// Direct terminal via crossterm (Linux/macOS/Windows).
    Terminal,
    /// WebAssembly Canvas2D.
    WasmCanvas,
    /// WebAssembly WebGL.
    WasmWebGL,
    /// Headless (for testing).
    Headless,
}

impl RenderBackend {
    pub fn detect() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            if has_webgl_support() {
                Self::WasmWebGL
            } else {
                Self::WasmCanvas
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if std::env::var("HEADLESS").is_ok() {
                Self::Headless
            } else {
                Self::Terminal
            }
        }
    }
}
```

## 8. Advanced Patterns

### 8.1 Multi-Metric Overlays

Display multiple series on the same graph with legends:

```rust
// Draw training vs validation loss on same graph
let train_graph = BrailleGraph::new(train_loss)
    .with_color(Color::new(0.3, 0.7, 1.0, 1.0))
    .with_label("Train");

let val_graph = BrailleGraph::new(val_loss)
    .with_color(Color::new(1.0, 0.5, 0.3, 1.0))
    .with_label("Val");

// Paint both on same canvas region
train_graph.layout(bounds);
train_graph.paint(&mut canvas);
val_graph.layout(bounds);
val_graph.paint(&mut canvas);
```

### 8.2 Status Indicators

Color-coded status indicators for categorical data:

```rust
fn status_color(status: &Status) -> Color {
    match status {
        Status::Running => Color::new(0.3, 0.9, 0.5, 1.0),   // Green
        Status::Pending => Color::new(0.9, 0.9, 0.3, 1.0),   // Yellow
        Status::Completed => Color::new(0.3, 0.7, 1.0, 1.0), // Blue
        Status::Failed => Color::new(1.0, 0.3, 0.3, 1.0),    // Red
    }
}
```

### 8.3 Progress Bars with ETA

Pipeline job progress with estimated time:

```rust
fn draw_progress(canvas: &mut impl Canvas, progress: f64, eta_secs: u64) {
    let bar_width = 30;
    let filled = (progress * bar_width as f64) as usize;

    let mut bar = String::with_capacity(bar_width + 2);
    bar.push('[');
    for i in 0..bar_width {
        bar.push(if i < filled { '█' } else { '░' });
    }
    bar.push(']');

    let pct = format!("{:5.1}%", progress * 100.0);
    let eta = format!("ETA: {}:{:02}", eta_secs / 60, eta_secs % 60);

    canvas.draw_text(&format!("{} {} {}", bar, pct, eta), pos, &style);
}
```

### 8.4 Sparkline Pattern

Compact inline graphs for table cells:

```rust
fn draw_sparkline(canvas: &mut impl Canvas, data: &[f64], x: f32, y: f32, width: usize) {
    let min = data.iter().fold(f64::MAX, |a, &b| a.min(b));
    let max = data.iter().fold(f64::MIN, |a, &b| a.max(b));
    let range = (max - min).max(0.001);

    let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let mut spark = String::with_capacity(width);

    let step = data.len() / width.max(1);
    for i in 0..width.min(data.len()) {
        let val = data.get(i * step).unwrap_or(&min);
        let norm = ((val - min) / range * 7.0) as usize;
        spark.push(chars[norm.min(7)]);
    }

    canvas.draw_text(&spark, Point::new(x, y), &TextStyle::default());
}
```

### 8.5 Real-time Tick Pattern

High-performance tick loop for 60fps updates:

```rust
use std::time::{Duration, Instant};

fn run_monitoring_loop<F: FnMut(&mut CellBuffer)>(mut render: F) {
    let tick_rate = Duration::from_millis(16); // 60fps
    let mut last_tick = Instant::now();
    let mut buffer = CellBuffer::new(80, 24);
    let mut renderer = DiffRenderer::with_color_mode(ColorMode::detect());

    loop {
        if last_tick.elapsed() >= tick_rate {
            render(&mut buffer);

            let mut output = Vec::with_capacity(8192);
            renderer.flush(&mut buffer, &mut output).unwrap();
            std::io::Write::write_all(&mut std::io::stdout(), &output).unwrap();

            last_tick = Instant::now();
        }

        if crossterm::event::poll(Duration::from_millis(1)).unwrap() {
            if let Ok(Event::Key(key)) = crossterm::event::read() {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }
}
```

### 8.6 Box Drawing Pattern

Consistent panel borders:

```rust
const BOX_CHARS: &str = "─│┌┐└┘├┤┬┴┼";

fn draw_box(canvas: &mut impl Canvas, x: f32, y: f32, w: f32, h: f32, title: &str) {
    let style = TextStyle { color: Color::new(0.4, 0.4, 0.4, 1.0), ..Default::default() };

    // Top border with title
    let top = format!("┌─{}─{:─<width$}┐", title, "", width = (w as usize - title.len() - 4));
    canvas.draw_text(&top, Point::new(x, y), &style);

    // Bottom border
    let bottom = format!("└{:─<width$}┘", "", width = w as usize - 2);
    canvas.draw_text(&bottom, Point::new(x, y + h - 1.0), &style);

    // Side borders
    for row in 1..(h as usize - 1) {
        canvas.draw_text("│", Point::new(x, y + row as f32), &style);
        canvas.draw_text("│", Point::new(x + w - 1.0, y + row as f32), &style);
    }
}
```

### 8.7 Heatmap Pattern

For correlation matrices and resource distribution:

```rust
fn heatmap_color(value: f64) -> Color {
    // Blue (cold) -> White (neutral) -> Red (hot)
    let t = value.clamp(0.0, 1.0) as f32;
    if t < 0.5 {
        let s = t * 2.0;
        Color::new(s, s, 1.0, 1.0) // Blue to white
    } else {
        let s = (t - 0.5) * 2.0;
        Color::new(1.0, 1.0 - s, 1.0 - s, 1.0) // White to red
    }
}

fn draw_heatmap(canvas: &mut impl Canvas, data: &[&[f64]], x: f32, y: f32) {
    for (row, values) in data.iter().enumerate() {
        for (col, &value) in values.iter().enumerate() {
            let color = heatmap_color(value);
            canvas.fill_rect(
                Rect::new(x + col as f32, y + row as f32, 1.0, 1.0),
                color,
            );
        }
    }
}
```

### 8.8 Threshold Alerts

Visual alerts when metrics cross thresholds:

```rust
fn draw_value_with_alert(
    canvas: &mut impl Canvas,
    value: f64,
    warn_threshold: f64,
    critical_threshold: f64,
    x: f32,
    y: f32,
) {
    let (color, prefix) = if value >= critical_threshold {
        (Color::new(1.0, 0.3, 0.3, 1.0), "🔴")  // Critical
    } else if value >= warn_threshold {
        (Color::new(1.0, 0.7, 0.2, 1.0), "🟡")  // Warning
    } else {
        (Color::new(0.3, 1.0, 0.5, 1.0), "🟢")  // Normal
    };

    let style = TextStyle { color, ..Default::default() };
    canvas.draw_text(&format!("{} {:.1}%", prefix, value), Point::new(x, y), &style);
}
```

## 9. Layout Patterns

### 9.1 Split Panels

Two-column layout for dashboard views:

```rust
fn split_horizontal(bounds: Rect, ratio: f32) -> (Rect, Rect) {
    let split = bounds.width * ratio;
    (
        Rect::new(bounds.x, bounds.y, split, bounds.height),
        Rect::new(bounds.x + split, bounds.y, bounds.width - split, bounds.height),
    )
}

fn split_vertical(bounds: Rect, ratio: f32) -> (Rect, Rect) {
    let split = bounds.height * ratio;
    (
        Rect::new(bounds.x, bounds.y, bounds.width, split),
        Rect::new(bounds.x, bounds.y + split, bounds.width, bounds.height - split),
    )
}
```

### 9.2 Grid Layout

For multi-GPU or multi-metric displays:

```rust
fn grid_layout(bounds: Rect, cols: usize, rows: usize) -> Vec<Rect> {
    let cell_w = bounds.width / cols as f32;
    let cell_h = bounds.height / rows as f32;

    (0..rows * cols)
        .map(|i| {
            let col = i % cols;
            let row = i / cols;
            Rect::new(
                bounds.x + col as f32 * cell_w,
                bounds.y + row as f32 * cell_h,
                cell_w,
                cell_h,
            )
        })
        .collect()
}
```

## 10. Future Extensions

- [ ] Interactive mode with keyboard navigation
- [ ] Remote monitoring via SSH
- [ ] Plugin system for custom collectors
- [ ] Alert thresholds with notifications
- [ ] Historical data persistence
- [ ] ROC/PR curve widgets for ML evaluation
- [ ] Confusion matrix visualization
- [ ] Log tail widget with filtering
- [ ] Process tree visualization

## 11. ttop-Style Dense Monitoring Widgets

Reference: btop (C++) and ttop (trueno-viz/ratatui) implementations.

### 11.1 Design Philosophy

ttop/btop achieve information density through:
- **Compact meters**: Single-character-height bars for per-core CPU
- **Grid layouts**: 48 cores in 12×4 or 8×6 grids
- **Stacked bars**: Memory breakdown in single bar with segments
- **Inline sparklines**: History graphs embedded in text lines
- **Gradient colors**: 101-value precomputed color arrays for smooth transitions

### 11.2 CpuGrid Widget

Dense per-core CPU visualization. Arranges N cores in compact grid.

```rust
/// Per-core CPU grid with gradient-colored meters.
/// Layout: Automatically arranges cores in optimal grid.
#[derive(Debug, Clone)]
pub struct CpuGrid {
    /// Per-core utilization (0.0-100.0).
    pub core_usage: Vec<f64>,
    /// Gradient for coloring (low→high).
    pub gradient: Gradient,
    /// Number of columns (auto-calculated if None).
    pub columns: Option<usize>,
    /// Show core labels (0, 1, 2...).
    pub show_labels: bool,
    /// Compact mode (no spacing).
    pub compact: bool,
}

impl CpuGrid {
    pub fn new(core_usage: Vec<f64>) -> Self;
    pub fn with_gradient(self, gradient: Gradient) -> Self;
    pub fn with_columns(self, cols: usize) -> Self;
    pub fn compact(self) -> Self;

    /// Calculate optimal grid dimensions for N cores.
    fn optimal_grid(core_count: usize, max_width: usize) -> (usize, usize);
}
```

**Render output** (48 cores, 8 columns):
```
CPU 12% │ 48 cores │ 5.3GHz │ 62°C
 0▃ 1▅ 2▂ 3▇ 4▄ 5▁ 6▆ 7▃
 8▄ 9▅10▂11▇12▄13▁14▆15▃
16▄17▅18▂19▇20▄21▁22▆23▃
24▄25▅26▂27▇28▄29▁30▆31▃
32▄33▅34▂35▇36▄37▁38▆39▃
40▄41▅42▂43▇44▄45▁46▆47▃
```

### 11.3 MemoryBar Widget

Stacked/segmented bar showing memory breakdown.

```rust
/// Stacked memory bar with labeled segments.
#[derive(Debug, Clone)]
pub struct MemoryBar {
    pub segments: Vec<MemorySegment>,
    pub total_bytes: u64,
    pub show_labels: bool,
    pub show_values: bool,
}

#[derive(Debug, Clone)]
pub struct MemorySegment {
    pub name: String,      // "Used", "Cached", "Swap", "Free"
    pub bytes: u64,
    pub color: Color,
}

impl MemoryBar {
    pub fn from_meminfo(info: &MemoryInfo) -> Self;
    pub fn with_zram(self, compressed: u64, uncompressed: u64) -> Self;
}
```

**Render output**:
```
Memory │ 93.6G │ 125.3G (75%) │ ZRAM:3.0x
Used: 93.6G ████████████████████████░░░░ 75%
Swap:  9.0G ░░░░░░░░░░░░░░░░░░░░░░░░░░░░  0%
Cache:18.4G ██████░░░░░░░░░░░░░░░░░░░░░░ 15%
```

### 11.4 NetworkPanel Widget

Network interface with inline sparklines for upload/download.

```rust
/// Network interface panel with sparkline history.
#[derive(Debug, Clone)]
pub struct NetworkPanel {
    pub interface: String,
    pub download_history: RingBuffer<u64>,  // bytes/s
    pub upload_history: RingBuffer<u64>,
    pub download_total: u64,
    pub upload_total: u64,
    pub gradient_down: Gradient,
    pub gradient_up: Gradient,
}

impl NetworkPanel {
    pub fn new(interface: &str) -> Self;
    pub fn push_sample(&mut self, down_bytes: u64, up_bytes: u64);
}
```

**Render output**:
```
Network (eno2) │ ↓ 18.9K/s │ ↑ 493.1K/s
↓ Download ▁▂▃▂▄▅▆▇▆▅▄▃▂▃▄▅▆▅▄▃  18.9K
↑ Upload   ▁▁▁▂▂▃▃▄▄▅▅▆▆▇▇▆▅▄▃▂ 493.1
Session: 14.3G ↓ 12.5G ↑ │ TCP: 28/12
```

### 11.5 ProcessTable Widget

Sortable process list with CPU/memory columns.

```rust
/// Process table with sorting and filtering.
#[derive(Debug, Clone)]
pub struct ProcessTable {
    pub processes: Vec<ProcessInfo>,
    pub sort_column: SortColumn,
    pub sort_descending: bool,
    pub filter: Option<String>,
    pub visible_rows: usize,
    pub scroll_offset: usize,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub state: char,        // R, S, D, Z, T
    pub cpu_percent: f64,
    pub mem_percent: f64,
    pub command: String,
}

#[derive(Debug, Clone, Copy)]
pub enum SortColumn {
    Pid, Name, Cpu, Memory, State,
}
```

**Render output**:
```
Processes (782) │ Sort: CPU% ▼
 PID S  C%  M% COMMAND
307329 R  4  2 gpu_showcase_be /mnt/nvme...
293746 R  2  1 whisper-apr-cli /mnt/nvme...
 45005 S  1  1 claude
 33185 R  1  1 ttop
```

### 11.6 DiskPanel Widget

Per-device disk I/O with read/write rates.

```rust
/// Disk I/O panel with per-device breakdown.
#[derive(Debug, Clone)]
pub struct DiskPanel {
    pub devices: Vec<DiskDevice>,
    pub show_iops: bool,
}

#[derive(Debug, Clone)]
pub struct DiskDevice {
    pub name: String,       // "nvme0n1", "sda"
    pub mount_point: String,
    pub read_bytes_sec: u64,
    pub write_bytes_sec: u64,
    pub iops: u32,
    pub usage_percent: f64,
}
```

**Render output**:
```
Disk │ R: 216.6K/s │ W: 542.2K/s │ 239 IOPS
/          1.8T ████████████░░░░ 44%  356K/s
nvme-raid 14.4T ██░░░░░░░░░░░░░░  1%   54M/s
```

### 11.7 GpuPanel Widget

NVIDIA/AMD GPU with utilization, memory, temp, power.

```rust
/// GPU information panel with all metrics.
#[derive(Debug, Clone)]
pub struct GpuPanel {
    pub device_name: String,
    pub gpu_util: f64,          // 0-100%
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub temperature_c: u32,
    pub power_watts: u32,
    pub power_limit_watts: u32,
    pub clock_mhz: u32,
    pub gpu_util_history: RingBuffer<f64>,
    pub gradient: Gradient,
}
```

**Render output**:
```
NVIDIA GeForce RTX 4090 │ 35°C │ 90W
GPU   2% ▁▁▁▂▂▁▁▂▃▂▁▁▁▁▂▂▁▁▁▁
VRAM 3.4/24.0G ██░░░░░░░░░░░░░░ 14%
90W/480W │ 2625MHz
```

### 11.8 SensorPanel Widget

Temperature sensors with color-coded values.

```rust
/// Temperature sensors panel.
#[derive(Debug, Clone)]
pub struct SensorPanel {
    pub sensors: Vec<TempSensor>,
    pub gradient: Gradient,  // cold→hot
}

#[derive(Debug, Clone)]
pub struct TempSensor {
    pub name: String,
    pub temp_c: f64,
    pub high_c: Option<f64>,
    pub crit_c: Option<f64>,
}
```

### 11.9 Implementation Priority

Based on ttop screenshot analysis:

| Priority | Widget | Complexity | Impact |
|----------|--------|------------|--------|
| P0 | CpuGrid | Medium | High - Core visual |
| P0 | MemoryBar | Low | High - Essential |
| P1 | NetworkPanel | Medium | Medium |
| P1 | ProcessTable | High | High - Interactive |
| P1 | GpuPanel | Low | High - GPU monitoring |
| P2 | DiskPanel | Low | Medium |
| P2 | SensorPanel | Low | Low |

### 11.10 Color Gradient Precomputation

Following btop pattern - precompute 101-element color arrays:

```rust
impl Gradient {
    /// Precompute 101 colors for fast lookup (0-100%).
    pub fn precompute(&self) -> [Color; 101] {
        let mut colors = [Color::BLACK; 101];
        for i in 0..=100 {
            colors[i] = self.sample(i as f64 / 100.0);
        }
        colors
    }

    /// Fast lookup by integer percentage.
    pub fn at_percent(&self, pct: u8) -> Color {
        // Use precomputed if available
        self.sample(pct as f64 / 100.0)
    }
}
```

### 11.11 Braille Graph Symbol Sets (btop reference)

btop uses 4 distinct symbol sets with 25 characters each for different graph styles:

```rust
/// Symbol sets for graph rendering (btop pattern).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolSet {
    /// Braille patterns - highest resolution (2x4 dots per cell)
    Braille,
    /// Block characters - high compatibility
    Block,
    /// TTY-safe ASCII - universal compatibility
    Tty,
    /// Custom user-defined set
    Custom,
}

/// Braille characters for upward-filling graphs (5x5 grid = 25 chars).
/// Each column represents 0-4 filled dots from bottom.
pub const BRAILLE_UP: [char; 25] = [
    ' ', '⢀', '⢠', '⢰', '⢸',
    '⡀', '⣀', '⣠', '⣰', '⣸',
    '⡄', '⣄', '⣤', '⣴', '⣼',
    '⡆', '⣆', '⣦', '⣶', '⣾',
    '⡇', '⣇', '⣧', '⣷', '⣿',
];

/// Braille characters for downward-filling graphs.
pub const BRAILLE_DOWN: [char; 25] = [
    ' ', '⠈', '⠘', '⠸', '⢸',
    '⠁', '⠉', '⠙', '⠹', '⢹',
    '⠃', '⠋', '⠛', '⠻', '⢻',
    '⠇', '⠏', '⠟', '⠿', '⢿',
    '⡇', '⡏', '⡟', '⡿', '⣿',
];

/// Block characters for upward-filling graphs.
/// Uses half/quarter blocks: ▁▂▃▄▅▆▇█
pub const BLOCK_UP: [char; 25] = [
    ' ', '▁', '▂', '▃', '▄',
    '▁', '▂', '▃', '▄', '▅',
    '▂', '▃', '▄', '▅', '▆',
    '▃', '▄', '▅', '▆', '▇',
    '▄', '▅', '▆', '▇', '█',
];

/// Block characters for downward-filling graphs.
pub const BLOCK_DOWN: [char; 25] = [
    ' ', '▔', '▔', '▀', '▀',
    '▔', '▔', '▀', '▀', '█',
    '▔', '▀', '▀', '█', '█',
    '▀', '▀', '█', '█', '█',
    '▀', '█', '█', '█', '█',
];

/// TTY-safe ASCII characters for graphs (universal compatibility).
pub const TTY_UP: [char; 25] = [
    ' ', '.', '.', 'o', 'o',
    '.', '.', 'o', 'o', 'O',
    '.', 'o', 'o', 'O', 'O',
    'o', 'o', 'O', 'O', '#',
    'o', 'O', 'O', '#', '#',
];

/// TTY-safe ASCII for downward graphs.
pub const TTY_DOWN: [char; 25] = [
    ' ', '\'', '\'', '"', '"',
    '\'', '\'', '"', '"', '*',
    '\'', '"', '"', '*', '*',
    '"', '"', '*', '*', '#',
    '"', '*', '*', '#', '#',
];

/// Custom symbol set builder.
#[derive(Debug, Clone)]
pub struct CustomSymbols {
    pub up: [char; 25],
    pub down: [char; 25],
}

impl CustomSymbols {
    pub fn from_chars(chars: &str) -> Self;
}

/// BrailleSymbols unified interface.
pub struct BrailleSymbols {
    set: SymbolSet,
    custom: Option<CustomSymbols>,
}

impl BrailleSymbols {
    pub fn new(set: SymbolSet) -> Self;
    pub fn with_custom(chars: CustomSymbols) -> Self;

    /// Get character for value (0.0-1.0) in up direction.
    #[inline]
    pub fn char_up(&self, value: f64) -> char;

    /// Get character for value in down direction.
    #[inline]
    pub fn char_down(&self, value: f64) -> char;

    /// Get character pair for two values (left 0-4, right 0-4).
    #[inline]
    pub fn char_pair(&self, left: u8, right: u8) -> char;
}
```

### 11.12 TextInput Widget

Full-featured text input with cursor, selection, and editing.

```rust
/// Text input widget with full editing capabilities.
#[derive(Debug, Clone)]
pub struct TextInput {
    /// Current text content.
    pub text: String,
    /// Cursor position (byte index).
    pub cursor: usize,
    /// Selection range (start, end) if active.
    pub selection: Option<(usize, usize)>,
    /// Placeholder text when empty.
    pub placeholder: String,
    /// Input mask (e.g., password).
    pub mask: Option<char>,
    /// Maximum length (None = unlimited).
    pub max_length: Option<usize>,
    /// Horizontal scroll offset.
    pub scroll_offset: usize,
    /// Is focused.
    pub focused: bool,
}

impl TextInput {
    pub fn new() -> Self;
    pub fn with_placeholder(self, text: &str) -> Self;
    pub fn with_mask(self, ch: char) -> Self;
    pub fn with_max_length(self, len: usize) -> Self;

    // Editing operations
    pub fn insert(&mut self, ch: char);
    pub fn insert_str(&mut self, s: &str);
    pub fn delete(&mut self);           // Delete at cursor
    pub fn backspace(&mut self);        // Delete before cursor
    pub fn delete_word(&mut self);      // Delete word at cursor
    pub fn delete_line(&mut self);      // Delete entire line

    // Cursor movement
    pub fn move_left(&mut self);
    pub fn move_right(&mut self);
    pub fn move_word_left(&mut self);
    pub fn move_word_right(&mut self);
    pub fn move_home(&mut self);
    pub fn move_end(&mut self);

    // Selection
    pub fn select_all(&mut self);
    pub fn select_word(&mut self);
    pub fn extend_selection_left(&mut self);
    pub fn extend_selection_right(&mut self);
    pub fn clear_selection(&mut self);
    pub fn selected_text(&self) -> Option<&str>;
    pub fn delete_selection(&mut self);

    // Clipboard (caller provides)
    pub fn copy(&self) -> Option<String>;
    pub fn cut(&mut self) -> Option<String>;
    pub fn paste(&mut self, text: &str);

    // State
    pub fn text(&self) -> &str;
    pub fn set_text(&mut self, text: &str);
    pub fn is_empty(&self) -> bool;
}
```

**Render output**:
```
┌─ Filter ─────────────────────┐
│ cpu |                        │
└──────────────────────────────┘
  cursor here ^
```

### 11.13 Scrollbar Widget

Vertical/horizontal scrollbar with arrow buttons.

```rust
/// Scrollbar with position indicator and arrow buttons.
#[derive(Debug, Clone)]
pub struct Scrollbar {
    /// Orientation.
    pub orientation: Orientation,
    /// Current scroll position (0.0-1.0).
    pub position: f64,
    /// Visible portion size (0.0-1.0).
    pub thumb_size: f64,
    /// Total content length.
    pub content_length: usize,
    /// Visible viewport length.
    pub viewport_length: usize,
    /// Show arrow buttons.
    pub show_arrows: bool,
    /// Characters used for rendering.
    pub chars: ScrollbarChars,
}

#[derive(Debug, Clone)]
pub struct ScrollbarChars {
    pub track: char,      // '░' or '│'
    pub thumb: char,      // '█' or '┃'
    pub arrow_up: char,   // '↑' or '▲'
    pub arrow_down: char, // '↓' or '▼'
    pub arrow_left: char, // '←' or '◀'
    pub arrow_right: char,// '→' or '▶'
}

impl Default for ScrollbarChars {
    fn default() -> Self {
        Self {
            track: '░',
            thumb: '█',
            arrow_up: '↑',
            arrow_down: '↓',
            arrow_left: '←',
            arrow_right: '→',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

impl Scrollbar {
    pub fn vertical(content_len: usize, viewport_len: usize) -> Self;
    pub fn horizontal(content_len: usize, viewport_len: usize) -> Self;
    pub fn with_arrows(self, show: bool) -> Self;
    pub fn with_chars(self, chars: ScrollbarChars) -> Self;

    /// Update scroll position from scroll offset.
    pub fn set_offset(&mut self, offset: usize);

    /// Get current offset.
    pub fn offset(&self) -> usize;

    /// Scroll by delta (positive = down/right).
    pub fn scroll(&mut self, delta: i32);

    /// Page up/down.
    pub fn page_up(&mut self);
    pub fn page_down(&mut self);

    /// Jump to position (0.0-1.0).
    pub fn jump_to(&mut self, position: f64);
}
```

**Render output** (vertical):
```
↑
█
█
░
░
░
↓
```

### 11.14 CollapsiblePanel Widget

Panel that can be collapsed/expanded with header.

```rust
/// Collapsible panel with header and toggle.
#[derive(Debug, Clone)]
pub struct CollapsiblePanel {
    /// Panel title.
    pub title: String,
    /// Collapsed state.
    pub collapsed: bool,
    /// Collapse direction.
    pub direction: CollapseDirection,
    /// Indicator characters.
    pub indicators: CollapseIndicators,
    /// Border style.
    pub border: BorderStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapseDirection {
    /// Collapses upward (content below header).
    Up,
    /// Collapses downward (content above header).
    Down,
    /// Collapses leftward.
    Left,
    /// Collapses rightward.
    Right,
}

#[derive(Debug, Clone)]
pub struct CollapseIndicators {
    pub expanded: char,   // '▼' or '−'
    pub collapsed: char,  // '▶' or '+'
}

impl Default for CollapseIndicators {
    fn default() -> Self {
        Self {
            expanded: '▼',
            collapsed: '▶',
        }
    }
}

impl CollapsiblePanel {
    pub fn new(title: &str) -> Self;
    pub fn with_collapsed(self, collapsed: bool) -> Self;
    pub fn with_direction(self, dir: CollapseDirection) -> Self;
    pub fn with_border(self, style: BorderStyle) -> Self;

    pub fn toggle(&mut self);
    pub fn expand(&mut self);
    pub fn collapse(&mut self);
    pub fn is_collapsed(&self) -> bool;
}
```

**Render output** (expanded):
```
╭─▼ CPU ───────────────────────╮
│ Core 0: 45%  ████████░░░░░░░ │
│ Core 1: 32%  ██████░░░░░░░░░ │
╰──────────────────────────────╯
```

**Render output** (collapsed):
```
╭─▶ CPU ───────────────────────╮
╰──────────────────────────────╯
```

### 11.15 Theme System

Configurable theme with base colors, box colors, and gradients.

```rust
/// Complete theme configuration (btop pattern).
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name.
    pub name: String,
    /// Base colors.
    pub base: BaseColors,
    /// Box/panel colors.
    pub boxes: BoxColors,
    /// Predefined gradients.
    pub gradients: ThemeGradients,
}

/// Base UI colors (8 colors).
#[derive(Debug, Clone)]
pub struct BaseColors {
    pub main_bg: Color,       // Background
    pub main_fg: Color,       // Primary text
    pub title: Color,         // Panel titles
    pub hi_fg: Color,         // Highlighted text
    pub selected_bg: Color,   // Selected item background
    pub selected_fg: Color,   // Selected item foreground
    pub inactive_fg: Color,   // Inactive/disabled text
    pub proc_misc: Color,     // Process state indicators
}

/// Box/panel border colors (4 colors).
#[derive(Debug, Clone)]
pub struct BoxColors {
    pub cpu: Color,           // CPU panel border
    pub mem: Color,           // Memory panel border
    pub net: Color,           // Network panel border
    pub proc: Color,          // Process panel border
}

/// Predefined gradients (8 gradients).
#[derive(Debug, Clone)]
pub struct ThemeGradients {
    pub cpu: Gradient,        // CPU usage (green→red)
    pub temp: Gradient,       // Temperature (blue→red)
    pub mem: Gradient,        // Memory (purple→yellow)
    pub download: Gradient,   // Download rate
    pub upload: Gradient,     // Upload rate
    pub used: Gradient,       // Disk used
    pub free: Gradient,       // Disk free
    pub process: Gradient,    // Process CPU/mem
}

impl Theme {
    /// Default dark theme (btop Default).
    pub fn dark() -> Self;

    /// Light theme.
    pub fn light() -> Self;

    /// Dracula theme.
    pub fn dracula() -> Self;

    /// Nord theme.
    pub fn nord() -> Self;

    /// Gruvbox theme.
    pub fn gruvbox() -> Self;

    /// Tokyo Night theme.
    pub fn tokyo_night() -> Self;

    /// Load from TOML file.
    pub fn from_toml(path: &Path) -> Result<Self, ThemeError>;

    /// Apply theme globally.
    pub fn apply(&self);
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Gradient with 101-step precomputation.
#[derive(Debug, Clone)]
pub struct Gradient {
    stops: Vec<(f64, Color)>,
    cache: Option<Box<[Color; 101]>>,
}

impl Gradient {
    pub fn new(stops: Vec<(f64, Color)>) -> Self;

    /// Two-color gradient.
    pub fn two(start: Color, end: Color) -> Self;

    /// Three-color gradient.
    pub fn three(start: Color, mid: Color, end: Color) -> Self;

    /// Precompute 101 colors for fast lookup.
    pub fn precompute(&mut self);

    /// Sample color at position (0.0-1.0).
    #[inline]
    pub fn sample(&self, t: f64) -> Color;

    /// Fast lookup by integer percentage (0-100).
    #[inline]
    pub fn at_percent(&self, pct: u8) -> Color;
}
```

**Built-in themes**:
| Theme | Background | Primary | Accent |
|-------|------------|---------|--------|
| Default (Dark) | #0d1117 | #c9d1d9 | #58a6ff |
| Light | #ffffff | #24292f | #0969da |
| Dracula | #282a36 | #f8f8f2 | #bd93f9 |
| Nord | #2e3440 | #eceff4 | #88c0d0 |
| Gruvbox | #282828 | #ebdbb2 | #fabd2f |
| Tokyo Night | #1a1b26 | #c0caf5 | #7aa2f7 |

### 11.16 Rounded Border Variant

Box-drawing with rounded corners.

```rust
/// Border style variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    /// Sharp corners: ┌┐└┘
    Sharp,
    /// Rounded corners: ╭╮╰╯
    Rounded,
    /// Double lines: ╔╗╚╝
    Double,
    /// Heavy lines: ┏┓┗┛
    Heavy,
    /// No border
    None,
}

/// Border characters for each style.
#[derive(Debug, Clone)]
pub struct BorderChars {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
    pub cross: char,
    pub t_down: char,     // ┬
    pub t_up: char,       // ┴
    pub t_right: char,    // ├
    pub t_left: char,     // ┤
}

impl BorderChars {
    pub fn sharp() -> Self {
        Self {
            top_left: '┌', top_right: '┐',
            bottom_left: '└', bottom_right: '┘',
            horizontal: '─', vertical: '│',
            cross: '┼', t_down: '┬', t_up: '┴',
            t_right: '├', t_left: '┤',
        }
    }

    pub fn rounded() -> Self {
        Self {
            top_left: '╭', top_right: '╮',
            bottom_left: '╰', bottom_right: '╯',
            horizontal: '─', vertical: '│',
            cross: '┼', t_down: '┬', t_up: '┴',
            t_right: '├', t_left: '┤',
        }
    }

    pub fn double() -> Self {
        Self {
            top_left: '╔', top_right: '╗',
            bottom_left: '╚', bottom_right: '╝',
            horizontal: '═', vertical: '║',
            cross: '╬', t_down: '╦', t_up: '╩',
            t_right: '╠', t_left: '╣',
        }
    }

    pub fn heavy() -> Self {
        Self {
            top_left: '┏', top_right: '┓',
            bottom_left: '┗', bottom_right: '┛',
            horizontal: '━', vertical: '┃',
            cross: '╋', t_down: '┳', t_up: '┻',
            t_right: '┣', t_left: '┫',
        }
    }
}

impl From<BorderStyle> for BorderChars {
    fn from(style: BorderStyle) -> Self {
        match style {
            BorderStyle::Sharp => Self::sharp(),
            BorderStyle::Rounded => Self::rounded(),
            BorderStyle::Double => Self::double(),
            BorderStyle::Heavy => Self::heavy(),
            BorderStyle::None => Self::sharp(), // Fallback, won't render
        }
    }
}
```

### 11.17 Superscript Numbers

For compact core labels and indices.

```rust
/// Superscript digit characters.
pub const SUPERSCRIPT: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];

/// Subscript digit characters.
pub const SUBSCRIPT: [char; 10] = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];

/// Convert number to superscript string.
pub fn to_superscript(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|c| SUPERSCRIPT[(c as u8 - b'0') as usize])
        .collect()
}

/// Convert number to subscript string.
pub fn to_subscript(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|c| SUBSCRIPT[(c as u8 - b'0') as usize])
        .collect()
}
```

**Usage in CpuGrid**:
```
CPU⁰▃ CPU¹▅ CPU²▂ CPU³▇ CPU⁴▄ CPU⁵▁ CPU⁶▆ CPU⁷▃
```

### 11.18 Battery Indicator

Battery status widget for laptops.

```rust
/// Battery status indicator.
#[derive(Debug, Clone)]
pub struct BatteryIndicator {
    /// Current charge level (0.0-1.0).
    pub level: f64,
    /// Charging state.
    pub state: BatteryState,
    /// Time remaining (seconds).
    pub time_remaining: Option<u64>,
    /// Gradient for level coloring.
    pub gradient: Gradient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryState {
    Discharging,
    Charging,
    Full,
    NotCharging,
    Unknown,
}

impl BatteryIndicator {
    pub fn new(level: f64, state: BatteryState) -> Self;
    pub fn with_time(self, seconds: u64) -> Self;

    /// Battery icon based on level.
    pub fn icon(&self) -> char {
        match (self.level * 10.0) as u8 {
            0 => '󰂎', // Empty
            1..=2 => '󰁺',
            3..=4 => '󰁼',
            5..=6 => '󰁾',
            7..=8 => '󰂀',
            _ => '󰁹', // Full
        }
    }
}
```

**Render output**:
```
󰁾 67% 2:34 remaining
```

## 12. Pixel-Perfect Testing with probar

### 12.1 Overview

All presentar-terminal widgets MUST produce **pixel-perfect** output identical to the reference implementations in btop and ttop. This is verified using probar's TUI testing framework with TextGrid snapshots.

### 12.2 Reference Capture Process

```rust
use probar::tui::{MockTty, TuiTestBackend, FrameSequence};

/// Capture reference frames from btop/ttop running in a PTY.
async fn capture_reference(cmd: &str, width: u16, height: u16) -> FrameSequence {
    let mut tty = MockTty::new(width, height);

    // Spawn reference program
    let mut child = Command::new(cmd)
        .env("TERM", "xterm-256color")
        .spawn_pty(width, height)?;

    // Capture frames at steady-state (after initial render)
    std::thread::sleep(Duration::from_millis(500));

    let frames = FrameSequence::capture(&mut child, Duration::from_secs(2))?;
    child.kill()?;

    frames
}
```

### 12.3 Widget Comparison Tests

Each widget has corresponding pixel-perfect tests:

| Widget | btop Component | ttop Component | Test |
|--------|----------------|----------------|------|
| CpuGrid | CPU panel with per-core bars | cpu_grid | `test_cpu_grid_matches_btop` |
| MemoryBar | Memory bar with segments | memory_bar | `test_memory_bar_matches_btop` |
| BrailleGraph | History graphs | braille_graph | `test_braille_graph_matches_ttop` |
| ProcessTable | Process list | process_table | `test_process_table_matches_btop` |
| NetworkPanel | Network panel with sparklines | network_panel | `test_network_panel_matches_btop` |
| GpuPanel | GPU panel (if present) | gpu_panel | `test_gpu_panel_matches_btop` |
| Meter | Horizontal bars | meter | `test_meter_matches_btop` |
| Scrollbar | Scrollbar indicators | scrollbar | `test_scrollbar_matches_btop` |
| CollapsiblePanel | Collapsible boxes | collapsible | `test_collapsible_matches_btop` |

### 12.4 Test Implementation Pattern

```rust
#[cfg(test)]
mod pixel_perfect_tests {
    use super::*;
    use probar::tui::{TuiTestBackend, TuiSnapshot, expect_frame};

    /// Reference snapshot from btop CPU panel (48 cores, 80x6 area).
    const BTOP_CPU_REF: &str = include_str!("fixtures/btop_cpu_48cores.txt");

    #[test]
    fn test_cpu_grid_matches_btop() {
        let mut backend = TuiTestBackend::new(80, 6);

        // Create widget with same data as reference
        let mut grid = CpuGrid::new(vec![
            12.5, 45.2, 3.1, 78.9, 22.4, 5.6, 67.8, 11.2,
            // ... all 48 cores matching reference
        ])
        .with_columns(8)
        .compact();

        grid.layout(Rect::new(0.0, 0.0, 80.0, 6.0));
        grid.paint(&mut backend);

        // Pixel-perfect comparison
        expect_frame(&backend.current_frame())
            .to_match_snapshot(BTOP_CPU_REF)
            .with_tolerance(0); // Zero tolerance = exact match
    }

    /// Test braille graph against ttop reference.
    #[test]
    fn test_braille_graph_matches_ttop() {
        let reference = TuiSnapshot::load("fixtures/ttop_braille_graph.snap")?;

        let mut backend = TuiTestBackend::new(40, 8);

        // Exact data from ttop capture
        let data: Vec<f64> = reference.metadata("data")
            .parse::<Vec<f64>>()
            .unwrap();

        let mut graph = BrailleGraph::new(data)
            .with_mode(GraphMode::Braille)
            .with_color(reference.metadata("color").parse()?);

        graph.layout(Rect::new(0.0, 0.0, 40.0, 8.0));
        graph.paint(&mut backend);

        expect_frame(&backend.current_frame())
            .to_match_snapshot(&reference)
            .with_tolerance(0);
    }
}
```

### 12.5 Fixture Generation

Fixtures are generated by capturing actual btop/ttop output:

```bash
# Capture btop CPU panel
cargo run --example capture_btop_cpu -- --width 80 --height 6 --output fixtures/btop_cpu_48cores.txt

# Capture ttop braille graph
cargo run --example capture_ttop_graph -- --width 40 --height 8 --output fixtures/ttop_braille_graph.snap

# Capture full btop screen
cargo run --example capture_btop_full -- --output fixtures/btop_full_screen.snap
```

### 12.6 Verification Requirements

Every PR that modifies a widget MUST:

1. **Pass pixel-perfect tests** - Zero tolerance for visual changes
2. **Update fixtures if intentional** - With reviewer approval
3. **Include before/after screenshots** - For visual review

### 12.7 CI Integration

```yaml
# .github/workflows/pixel-perfect.yml
pixel_perfect_tests:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Install btop
      run: sudo apt-get install -y btop
    - name: Install ttop
      run: cargo install ttop
    - name: Run pixel-perfect tests
      run: cargo test --package presentar-terminal pixel_perfect:: --release
    - name: Upload diff artifacts
      if: failure()
      uses: actions/upload-artifact@v4
      with:
        name: pixel-diff
        path: target/pixel-diffs/
```

### 12.8 Diff Visualization

When tests fail, probar generates visual diffs:

```
Expected (btop):           Got (presentar):           Diff:
╭─▼ CPU ──────────╮        ╭─▼ CPU ──────────╮       ................
│ 0▃ 1▅ 2▂ 3▇ 4▄  │        │ 0▃ 1▅ 2▂ 3▇ 4▄  │       ................
│ 5▁ 6▆ 7▃ 8▄ 9▅  │        │ 5▁ 6▆ 7▃ 8▄ 9▆  │       ..............X. <- mismatch
╰─────────────────╯        ╰─────────────────╯       ................
```

### 12.9 Tolerance Modes

For some widgets, exact matching is impractical:

```rust
expect_frame(&frame)
    .to_match_snapshot(&reference)
    .with_tolerance(0)           // Exact match (default)
    .ignore_color()              // Compare characters only
    .ignore_whitespace_at_eol()  // Ignore trailing spaces
    .with_region(Rect::new(1, 1, 78, 4)); // Compare only inner area
```

## References

- [simplified-tui-spec.md](simplified-tui-spec.md) - Direct TUI backend spec
- PROBAR-SPEC-009 - Brick Architecture specification
- trueno-viz btop example - Reference implementation
- btop (github.com/aristocratos/btop) - C++ reference for dense TUI
- [Unicode Braille Patterns](https://www.unicode.org/charts/PDF/U2800.pdf) - U+2800-28FF
- [Box Drawing Characters](https://www.unicode.org/charts/PDF/U2500.pdf) - U+2500-257F
