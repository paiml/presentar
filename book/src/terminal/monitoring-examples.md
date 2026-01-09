# Terminal Monitoring Examples

The `presentar-terminal` crate includes 11+ monitoring examples demonstrating real-time data visualization patterns. These examples follow the cbtop (ComputeBlock top) style, similar to btop/htop but focused on data science and ML workloads.

## Available Examples

### System Monitoring

| Example | Description | Command |
|---------|-------------|---------|
| `cpu_monitor` | Per-core CPU usage with history graphs | `cargo run -p presentar-terminal --example cpu_monitor` |
| `memory_monitor` | RAM/Swap with usage breakdown | `cargo run -p presentar-terminal --example memory_monitor` |
| `network_traffic` | RX/TX per interface with graphs | `cargo run -p presentar-terminal --example network_traffic` |
| `system_dashboard` | Combined btop-style overview | `cargo run -p presentar-terminal --example system_dashboard` |

### ML/Data Science

| Example | Description | Command |
|---------|-------------|---------|
| `training_metrics` | Loss/accuracy curves | `cargo run -p presentar-terminal --example training_metrics` |
| `gpu_compute` | GPU utilization/VRAM/temperature | `cargo run -p presentar-terminal --example gpu_compute` |
| `inference_server` | Request latency/queue depth | `cargo run -p presentar-terminal --example inference_server` |
| `batch_progress` | Pipeline job progress tracking | `cargo run -p presentar-terminal --example batch_progress` |

### Infrastructure

| Example | Description | Command |
|---------|-------------|---------|
| `queue_monitor` | Message queue depth/throughput | `cargo run -p presentar-terminal --example queue_monitor` |
| `cluster_status` | Kubernetes node/pod status | `cargo run -p presentar-terminal --example cluster_status` |
| `sensor_dashboard` | IoT sensor readings | `cargo run -p presentar-terminal --example sensor_dashboard` |

## Widget Usage Patterns

### BrailleGraph Widget

Time-series visualization with multiple render modes:

```rust
use presentar_core::Widget;
use presentar_terminal::{BrailleGraph, GraphMode};

// Create a graph with history data
let mut graph = BrailleGraph::new(history_data.to_vec())
    .with_color(Color::new(0.3, 0.9, 0.5, 1.0))
    .with_range(0.0, 100.0)
    .with_mode(GraphMode::Braille);

// Layout and paint
graph.layout(Rect::new(x, y, width, height));
graph.paint(&mut canvas);
```

**Render Modes:**
- `GraphMode::Braille` - Highest resolution (2×4 dots per cell)
- `GraphMode::Block` - Half-block characters (▀▄█)
- `GraphMode::Tty` - ASCII-only (`*` characters)

### Meter Widget

Horizontal progress/gauge:

```rust
use presentar_terminal::Meter;

let meter = Meter::percentage(75.0)
    .with_label("CPU")
    .with_gradient(Color::GREEN, Color::RED);
```

### Color Coding Standards

The examples follow consistent color coding:

| Metric Range | Color | Use Case |
|--------------|-------|----------|
| Critical (>90%) | Red `(1.0, 0.3, 0.3)` | Overloaded resources |
| Warning (>70%) | Orange `(1.0, 0.7, 0.2)` | Elevated usage |
| Elevated (>50%) | Yellow `(1.0, 1.0, 0.3)` | Moderate usage |
| Normal (<50%) | Green `(0.3, 1.0, 0.5)` | Healthy |
| Idle (<10%) | Gray `(0.5, 0.5, 0.5)` | Underutilized |

## Example Walkthroughs

### System Dashboard

The `system_dashboard` example demonstrates a complete btop-style interface:

```
┌─CPU────────────────────────────────────┬─Memory──────────────────────────────┐
│ CPU Total [ 45.2%]                     │ Memory    [18.2/32.0 GB]           │
│ ⣿⣷⣿⣷⣿⡿⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷  │ ████████████████████░░░░░░░░░░░     │
│ ▮▮▮▯ ▮▮▯▯ ▮▮▮▮ ▮▯▯▯ ▮▮▮▮ ▮▮▯▯ ▮▯▯▯ ▮▮▮▯ │ Swap: 1.2/8.0 GB                   │
│ 0  1  2  3  4  5  6  7                 │                                    │
├─Network────────────────────────────────┼─Disk────────────────────────────────┤
│ ↓ 45.2MB/s  ↑ 12.3MB/s                │ /      ██████████░░░░░  70.5%       │
│ ▄▄▆█▄▄▆▄▄▆▄▆▆▄▄▆▄▄▆▄▄▆▄▄▆█▆▄▄▆▄▄▆▄▄▆ │ /home  ████████████████░ 85.2%      │
└────────────────────────────────────────┴─────────────────────────────────────┘
```

### Training Metrics

The `training_metrics` example shows ML training progress:

```
┌─Loss Curves────────────────────────────┬─Accuracy Curves─────────────────────┐
│ ━Train ━Val                            │ ━Train ━Val                          │
│ ⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷ │ ⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿⣷⣿  │
│                                        │                                      │
├─Learning Rate──────────────────────────┼─Training Statistics──────────────────┤
│ lr=1.23e-04                            │ Best Train Loss: 0.0512              │
│ ▀▀▀▀▀▀▀▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄ │ Best Val Loss:   0.0823              │
└────────────────────────────────────────┴──────────────────────────────────────┘
```

## Creating Custom Monitoring Views

### Step 1: Set Up the Buffer

```rust
use presentar_terminal::direct::{CellBuffer, DiffRenderer, DirectTerminalCanvas};
use presentar_terminal::ColorMode;

// Create buffer and renderer
let mut buffer = CellBuffer::new(80, 24);
let mut renderer = DiffRenderer::with_color_mode(ColorMode::TrueColor);
```

### Step 2: Draw with Canvas

```rust
{
    let mut canvas = DirectTerminalCanvas::new(&mut buffer);

    // Background
    canvas.fill_rect(Rect::new(0.0, 0.0, 80.0, 24.0), Color::new(0.02, 0.02, 0.05, 1.0));

    // Draw your widgets
    draw_header(&mut canvas);
    draw_graphs(&mut canvas, &data);
    draw_footer(&mut canvas);
}
```

### Step 3: Render Output

```rust
let mut output = Vec::with_capacity(8192);
renderer.flush(&mut buffer, &mut output).unwrap();
std::io::Write::write_all(&mut std::io::stdout(), &output).unwrap();
```

## Performance Considerations

All examples are designed for:
- **60fps rendering** (<16ms frame time)
- **Zero allocations** in steady state
- **Minimal terminal I/O** via differential updates

For real-time applications, use the high-performance config:

```rust
let config = TuiConfig::high_performance(); // 16ms tick, 60fps
```

## See Also

- [Direct Backend](./direct-backend.md) - Architecture details
- [cbtop Specification](../../docs/specifications/compute-block-tui-cbtop.md) - Full specification
- [Trueno-Viz Examples](/ecosystem/trueno-viz.md) - Related visualization examples
