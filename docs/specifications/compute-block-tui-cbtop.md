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

## References

- [simplified-tui-spec.md](simplified-tui-spec.md) - Direct TUI backend spec
- PROBAR-SPEC-009 - Brick Architecture specification
- trueno-viz btop example - Reference implementation
- [Unicode Braille Patterns](https://www.unicode.org/charts/PDF/U2800.pdf) - U+2800-28FF
- [Box Drawing Characters](https://www.unicode.org/charts/PDF/U2500.pdf) - U+2500-257F
