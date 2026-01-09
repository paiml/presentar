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

## 7. Integration with trueno-viz

The cbtop patterns complement `trueno-viz` monitor widgets:

- `trueno-viz` provides: PNG/SVG output, web rendering
- `presentar-terminal` provides: Zero-allocation TUI output

Both share common visualization patterns for data science:
- Loss curves
- ROC/PR curves
- Heatmaps
- Sparklines

## 8. Future Extensions

- [ ] Interactive mode with keyboard navigation
- [ ] Remote monitoring via SSH
- [ ] Plugin system for custom collectors
- [ ] Alert thresholds with notifications
- [ ] Historical data persistence

## References

- [simplified-tui-spec.md](simplified-tui-spec.md) - Direct TUI backend spec
- PROBAR-SPEC-009 - Brick Architecture specification
- trueno-viz btop example - Reference implementation
