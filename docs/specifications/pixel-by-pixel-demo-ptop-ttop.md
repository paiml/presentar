# SPEC-024: Pixel-by-Pixel Recreation of cbtop/ttop Using presentar-terminal

**Status**: Draft → **Under Review**
**Author**: Claude Code
**Date**: 2026-01-09
**Version**: 1.2.0
**Revision**: Added Chaos Engineering strategy, expanded checklist to 125 points (Fuzzing/Stress), refined Accessibility specs.

## 1. Executive Summary

This specification defines the requirements for a pixel-perfect recreation of the `cbtop` (Compute Block Top) terminal UI from `trueno/crates/cbtop` using exclusively `presentar-terminal` widgets. The goal is to prove that presentar-terminal provides complete feature parity with btop/htop-style terminal interfaces.

## 2. Background and Motivation

### 2.1 Problem Statement

Terminal UI frameworks (TUI) must provide:
1. High-resolution character graphics (braille, block characters)
2. Perceptually uniform color gradients
3. Real-time data visualization
4. Responsive layouts

### 2.2 Academic Foundation

This specification is grounded in peer-reviewed research:

| Citation | Contribution |
|----------|--------------|
| Fairchild, M.D. (2013). *Color Appearance Models*. Wiley. | CIELAB perceptual uniformity for gradient interpolation |
| Ware, C. (2012). *Information Visualization: Perception for Design*. Morgan Kaufmann. | Pre-attentive processing for real-time monitoring displays |
| Tufte, E.R. (2001). *The Visual Display of Quantitative Information*. Graphics Press. | Data-ink ratio optimization for dense terminal displays |
| Few, S. (2009). *Now You See It*. Analytics Press. | Dashboard design patterns for system monitoring |
| Stone, M. (2006). "Choosing Colors for Data Visualization". Perceptual Edge. | Color selection for categorical and sequential data |
| Healey, C.G. & Enns, J.T. (2012). "Attention and Visual Memory in Visualization and Computer Graphics". IEEE TVCG. | Braille pattern recognition and visual search |
| Borland, D. & Taylor, R.M. (2007). "Rainbow Color Map (Still) Considered Harmful". IEEE CG&A. | Perceptually linear gradients for quantitative data |
| Zeileis, A. et al. (2020). "colorspace: A Toolbox for Manipulating and Assessing Colors and Palettes". JSS. | HCL color space implementation |

### 2.3 Design Principles

Following Toyota Production System (TPS) principles:

- **Jidoka**: Rendering blocked if Brick verification fails
- **Poka-Yoke**: Type system prevents invalid widget configurations
- **Genchi Genbutsu**: Visual regression tests verify actual pixel output

## 3. Target Interface: cbtop

### 3.1 Layout Structure

```
+─────────────────────────────────────────────────────────────────────────────────+
│ cbtop - Compute Block System Monitor                      uptime: 5d 12:34:56  │
+────────────────────────────────────+────────────────────────────────────────────+
│ ─CPU──────────────── 28.2%───────  │ ─Memory──────────────────75.1/128 GB────  │
│ ⢀⣀⠤⠤⠤⠤⠤⠤⠤⠤⠤⢄⣀⣀⣀⠐⠒⠒⠤⠤⣀⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀  │ ⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠊⠉⠒⠒⠒⠒⠒⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀  │
│  0▃ 1█ 2█ 3█ 4█ 5▆ 6▄ 7▃          │   Used: 50.0G ██░░░ 39%                   │
│  8▄ 9▅10▇11█12█13▇14▅15▄          │  Cached: 30.0G █░░░░ 23%                   │
│                                    │    Swap:  2.0G ░░░░░  2%                   │
+────────────────────────────────────+────────────────────────────────────────────+
│ ─Network─────────────────────────  │ ─Disk────────────────────────────────────  │
│ Network                            │ /       ██████████████░░░░░░ 70.5%         │
│ eth0  ▄▄▄▄▄▅▅▅ 67.6M/s ↓ ▃▅▄▃▂▄  │ /home   █████████████░░░░░░░ 62.7%         │
│ wlan0 ▃▃▃▃▃▃▄▄  3.6M/s ↓ ▇████▇▇  │ /data   ███████████████░░░░░ 72.5%         │
+────────────────────────────────────+────────────────────────────────────────────+
│ ─Processes──────────────────────────────────────────────────────────────────── │
│    PID │ USER     │   CPU% │   MEM% │ COMMAND                                   │
│ ────────────────────────────────────────────────────────────────────────────── │
│   1234 │ noah     │  25.3% │   5.5% │ firefox                                   │
│   5678 │ noah     │  18.7% │  12.3% │ rustc                                     │
│   9012 │ noah     │  15.2% │   8.1% │ code                                      │
│   3456 │ root     │  12.8% │   3.2% │ dockerd                                   │
+─────────────────────────────────────────────────────────────────────────────────+
│ [q]quit  [h]help  [c]sort:cpu  [m]sort:mem  [p]sort:pid  [k]kill  [/]filter    │
+─────────────────────────────────────────────────────────────────────────────────+
```

### 3.2 Widget Mapping

| UI Region | cbtop Implementation | presentar-terminal Widget |
|-----------|---------------------|---------------------------|
| CPU Graph | `BrailleGraph` | `BrailleGraph` |
| CPU Grid | Custom paint | `CpuGrid` |
| Memory Bars | Custom paint | `MemoryBar` |
| Network Panel | Custom paint | `NetworkPanel` |
| Process Table | Custom paint | `ProcessTable` |
| Borders | Manual draw | `Border` |
| Sparklines | `BrailleGraph` | `Sparkline` |
| Gauges | Custom meters | `Gauge`, `Meter` |

## 4. Widget Specifications

### 4.1 BrailleGraph

**Purpose**: Time-series visualization using 2×4 braille dot matrix per cell.

**Symbols**: Uses `BRAILLE_UP[25]` for upward-filling patterns.

```rust
pub const BRAILLE_UP: [char; 25] = [
    ' ', '⢀', '⢠', '⢰', '⢸',  // left=0
    '⡀', '⣀', '⣠', '⣰', '⣸',  // left=1
    '⡄', '⣄', '⣤', '⣴', '⣼',  // left=2
    '⡆', '⣆', '⣦', '⣶', '⣾',  // left=3
    '⡇', '⣇', '⣧', '⣷', '⣿',  // left=4
];
```

**Resolution**: 2 horizontal × 4 vertical dots per character cell.

**Reference**: Unicode Standard 15.0, Block "Braille Patterns" (U+2800–U+28FF).

### 4.2 CpuGrid

**Purpose**: Per-core CPU utilization display with compact bars.

**Layout**: Configurable columns (default 8), block character bars (▁▂▃▄▅▆▇█).

**Color Gradient**: CIELAB interpolation from green (0%) → yellow (50%) → red (100%).

### 4.3 MemoryBar

**Purpose**: Segmented memory usage visualization.

**Segments**: Used, Cached, Available (configurable).

**Reference**: Tufte's data-ink ratio principle - minimal chrome, maximum data.

### 4.4 ProcessTable

**Purpose**: Sortable process list with selection highlighting.

**Columns**: PID, USER, CPU%, MEM%, COMMAND.

**Colors**:
- PID/USER/COMMAND: Light gray (`Color::new(0.8, 0.8, 0.8, 1.0)`)
- CPU%: Gradient based on value
- MEM%: Gradient based on value
- Selected row: White text, highlighted background

### 4.5 NetworkPanel

**Purpose**: Interface bandwidth with directional sparklines.

**Layout**: `{name} {rx_spark} {rx_rate}↓ {tx_spark} {tx_rate}↑`

**Colors**: Green for RX (download), Red for TX (upload).

## 5. Color System

### 5.1 Gradient Implementation

Using CIELAB (L*a*b*) color space for perceptual uniformity:

```rust
fn interpolate_lab(c1: Color, c2: Color, t: f64) -> Color {
    // Convert RGB → XYZ → Lab
    let lab1 = rgb_to_lab(c1);
    let lab2 = rgb_to_lab(c2);

    // Linear interpolation in Lab space
    let l = lab1.l + (lab2.l - lab1.l) * t;
    let a = lab1.a + (lab2.a - lab1.a) * t;
    let b = lab1.b + (lab2.b - lab1.b) * t;

    // Convert Lab → XYZ → RGB
    lab_to_rgb(Lab { l, a, b })
}
```

**Reference**: Fairchild (2013), Chapter 10: "CIELAB Color Space".

### 5.2 Theme Palettes

| Theme | Background | Foreground | CPU Gradient | Memory Gradient |
|-------|------------|------------|--------------|-----------------|
| Tokyo Night | `#1a1b26` | `#c0caf5` | `#7aa2f7→#e0af68→#f7768e` | `#9ece6a→#e0af68→#f7768e` |
| Dracula | `#282a36` | `#f8f8f2` | `#50fa7b→#f1fa8c→#ff5555` | `#8be9fd→#f1fa8c→#ff5555` |
| Nord | `#2e3440` | `#eceff4` | `#a3be8c→#ebcb8b→#bf616a` | `#88c0d0→#ebcb8b→#bf616a` |
| Monokai | `#272822` | `#f8f8f2` | `#a6e22e→#e6db74→#f92672` | `#66d9ef→#e6db74→#f92672` |

### 5.3 Color Mode Fallback

```rust
pub enum ColorMode {
    TrueColor,  // 24-bit RGB (16M colors)
    Color256,   // 256-color palette
    Color16,    // 16 ANSI colors
    Mono,       // No color
}
```

**Auto-detection Algorithm**:
```rust
fn detect_with_env(colorterm: Option<String>, term: Option<String>) -> ColorMode {
    // 1. COLORTERM takes priority (most reliable)
    if colorterm == "truecolor" || colorterm == "24bit" { return TrueColor; }

    // 2. Fall back to TERM
    match term {
        Some(t) if t.contains("256color") => Color256,
        Some(t) if t.contains("color") || t.contains("xterm") => Color16,
        Some("dumb") | None => Mono,  // NOTE: Unknown defaults to Mono, not Color16
        _ => Color16,
    }
}
```

**CRITICAL**: Unknown `TERM` values (e.g., `"vt100"`) default to `Color16`, but **missing** `TERM` defaults to `Mono`.

## 6. Rendering Pipeline

### 6.1 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    presentar-terminal                            │
├─────────────────────────────────────────────────────────────────┤
│  Widget Layer:  CpuGrid, MemoryBar, ProcessTable, NetworkPanel  │
├─────────────────────────────────────────────────────────────────┤
│  Canvas Layer:  DirectTerminalCanvas → CellBuffer               │
├─────────────────────────────────────────────────────────────────┤
│  Render Layer:  DiffRenderer → ANSI escape sequences            │
├─────────────────────────────────────────────────────────────────┤
│  Output:        stdout (crossterm)                               │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 Zero-Allocation Steady State

After initial allocation:
- `CellBuffer`: Pre-allocated grid, reused each frame
- `DiffRenderer`: Only emits changed cells
- No heap allocation during paint cycle

**Reference**: Rust Performance Book, "Avoiding Allocations".

## 7. Verification Framework

### 7.1 Brick Architecture (PROBAR-SPEC-009)

Every widget implements the `Brick` trait:

```rust
pub trait Brick {
    fn brick_name(&self) -> &'static str;
    fn assertions(&self) -> Vec<BrickAssertion>;
    fn budget(&self) -> BrickBudget;
    fn verify(&self) -> BrickVerification;
    fn can_render(&self) -> bool { self.verify().is_ok() }
}
```

### 7.2 Assertions

| Widget | Assertions |
|--------|------------|
| `CpuGrid` | `MinWidth(20)`, `DataNotEmpty`, `MaxLatency(8ms)` |
| `MemoryBar` | `MinWidth(30)`, `SegmentsSum100`, `MaxLatency(4ms)` |
| `ProcessTable` | `MinHeight(5)`, `SelectedInRange`, `MaxLatency(16ms)` |
| `NetworkPanel` | `MinWidth(40)`, `InterfacesNotEmpty`, `MaxLatency(8ms)` |
| `BrailleGraph` | `MinWidth(10)`, `DataInRange`, `MaxLatency(4ms)` |

## 8. Accessibility Requirements (WCAG 2.1 AA)

Terminal UIs have unique accessibility challenges. This section defines requirements for accessible monitoring dashboards.

### 8.1 Color Contrast

| Requirement | WCAG Criterion | Implementation |
|------------|----------------|----------------|
| Text contrast ≥4.5:1 | 1.4.3 (AA) | All themes must validate foreground/background contrast |
| Large text contrast ≥3:1 | 1.4.3 (AA) | Headers and titles with ≥18pt equivalent |
| Non-text contrast ≥3:1 | 1.4.11 (AA) | Gauge borders, graph lines, UI components |

**Verification**:
```rust
fn contrast_ratio(fg: Color, bg: Color) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}
```

### 8.2 Color Independence

Critical information must not rely solely on color:

| Widget | Color-Independent Indicator |
|--------|----------------------------|
| CPU Usage | Numeric percentage + bar length |
| Memory | Numeric value + segment labels |
| Process Table | Sort arrows (▲▼) + column headers |
| Alerts | Symbol prefix (⚠ ✗ ✓) + text label |

### 8.3 Keyboard Navigation

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Navigate between panels |
| `↑` / `↓` | Navigate within list/table |
| `Enter` | Select/activate |
| `Esc` | Close/cancel |
| `q` | Quit application |
| `?` / `h` | Show help |

### 8.4 Screen Reader Compatibility

Widgets must expose semantic information via the `accessibility()` method:

```rust
pub trait Widget {
    fn accessibility(&self) -> AccessibilityInfo {
        AccessibilityInfo {
            role: Role::Generic,
            label: None,
            value: None,
            description: None,
        }
    }
}
```

## 9. Error Handling Specifications

### 9.1 Graceful Degradation

| Failure Mode | Recovery Strategy |
|--------------|-------------------|
| Data source unavailable | Display "N/A" or last known value with stale indicator |
| Terminal resize during render | Abort frame, clear buffer, re-layout next frame |
| Invalid data values (NaN, Inf) | Clamp to 0.0 or 100.0 with warning log |
| Unicode rendering failure | Fall back to ASCII (TTY mode) |
| Memory allocation failure | Use pre-allocated buffers, avoid heap in hot path |

### 9.2 Error Types

```rust
pub enum TuiError {
    /// Terminal initialization failed
    TerminalInit(String),
    /// Widget verification failed (Brick assertion)
    WidgetVerification { widget: &'static str, assertion: String },
    /// Render budget exceeded
    BudgetExceeded { widget: &'static str, actual_ms: f64, budget_ms: f64 },
    /// Invalid configuration
    Config(String),
    /// I/O error (stdout write failed)
    Io(std::io::Error),
}
```

### 9.3 Panic Safety

**Invariant**: No widget may panic during the render cycle. All public methods must handle edge cases:

| Method | Edge Cases | Handling |
|--------|------------|----------|
| `measure()` | Zero constraints | Return `Size::ZERO` |
| `layout()` | Negative bounds | Clamp to zero |
| `paint()` | Out-of-bounds draw | Clip silently |
| `event()` | Unknown event | Return `None` |

## 10. Concurrency & Thread Safety

### 10.1 Thread Safety Requirements

All widgets implement `Send + Sync`:

```rust
pub trait Widget: Brick + Send + Sync {
    // ...
}
```

### 10.2 Data Update Pattern

Data updates must be atomic and non-blocking:

```rust
// CORRECT: Atomic swap
let new_data = fetch_cpu_metrics();
widget.set_data(Arc::new(new_data));

// INCORRECT: Lock during render
let mut data = widget.data.lock(); // BLOCKS RENDER
data.push(new_value);
```

### 10.3 Frame Synchronization

```
Main Thread:        [Event] → [Update State] → [Layout] → [Paint] → [Render]
                                    ↑
Data Thread:        [Fetch] ──────────────────────────────────────────→ [Atomic Store]
```

## 11. Chaos Engineering & Fuzzing

To ensure robust operation under hostile conditions, the system must undergo rigorous chaos testing.

### 11.1 Fuzzing Strategy (AFL++ / Honggfuzz)

Targets for continuous fuzzing:
1.  **Input Parsing**: Random byte streams sent to `handle_event()`.
2.  **Data Ingestion**: Malformed JSON/Metrics injected into `Widget::set_data()`.
3.  **Config Loading**: Corrupted TOML/YAML configuration files.

**Pass Criteria**: No panic, memory leak, or hang after 1 billion iterations.

### 11.2 Fault Injection

Simulated failures during runtime:
-   **Slow I/O**: Delays in data fetch (100ms - 10s).
-   **Terminal Noise**: Garbage characters written to stdin.
-   **Resource Starvation**: Restricting file descriptors or memory.

---

## 12. 125-Point Popperian Falsification QA Checklist

Based on Karl Popper's falsificationism: each test attempts to **disprove** the hypothesis that presentar-terminal achieves pixel-perfect parity with cbtop/btop.

### Methodology

Each test must satisfy three criteria:

1. **Measurable**: Quantitative threshold or exact expected value
2. **Reproducible**: Automated test in `tests/pixel_perfect_tests.rs` or `tests/cbtop_visibility.rs`
3. **Falsifiable**: Clear failure condition that invalidates the claim

**Test Execution**:
```bash
# Run all falsification tests
cargo test -p presentar-terminal --test pixel_perfect_tests

# Run with coverage tolerance (CI)
COVERAGE_MODE=1 cargo test -p presentar-terminal
```

### Section A: Symbol Rendering (F001-F020)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F001 | Braille empty is space | `BRAILLE_UP[0] != ' '` | [ ] |
| F002 | Braille full is ⣿ | `BRAILLE_UP[24] != '⣿'` | [ ] |
| F003 | Braille array length | `BRAILLE_UP.len() != 25` | [ ] |
| F004 | Block empty is space | `BLOCK_UP[0] != ' '` | [ ] |
| F005 | Block full is █ | `BLOCK_UP[24] != '█'` | [ ] |
| F006 | Block array length | `BLOCK_UP.len() != 25` | [ ] |
| F007 | TTY uses ASCII only | Any non-ASCII in `TTY_UP` | [ ] |
| F008 | Sparkline 8 levels | `SPARKLINE.len() != 8` | [ ] |
| F009 | Sparkline range ▁→█ | `SPARKLINE[0] != '▁' \|\| SPARKLINE[7] != '█'` | [ ] |
| F010 | Superscript 10 digits | `SUPERSCRIPT.len() != 10` | [ ] |
| F011 | Subscript 10 digits | `SUBSCRIPT.len() != 10` | [ ] |
| F012 | Braille pair index formula | `idx = left*5 + right` yields wrong char | [ ] |
| F013 | Braille left=4,right=0 | `BRAILLE_UP[20] != '⡇'` | [ ] |
| F014 | Braille left=0,right=4 | `BRAILLE_UP[4] != '⢸'` | [ ] |
| F015 | Block chars progressive | BLOCK_UP not monotonically increasing | [ ] |
| F016 | Unicode braille range | Any char outside U+2800-U+28FF | [ ] |
| F017 | Braille down inverted | BRAILLE_DOWN[24] != '⣿' | [ ] |
| F018 | Custom symbols fallback | Custom with None data uses Braille | [ ] |
| F019 | Symbol set default | `SymbolSet::default() != SymbolSet::Braille` | [ ] |
| F020 | Box drawing chars | Missing ─│┌┐└┘├┤┬┴┼ | [ ] |

### Section B: Color System (F021-F040)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F021 | LAB interpolation midpoint | RGB(red→blue).sample(0.5) differs from LAB(red→blue).sample(0.5) by >ΔE 5 | [ ] |
| F022 | Gradient 0.0 returns start | `gradient.sample(0.0) != stops[0]` | [ ] |
| F023 | Gradient 1.0 returns end | `gradient.sample(1.0) != stops[last]` | [ ] |
| F024 | Gradient clamping | `sample(-0.5)` or `sample(1.5)` panics | [ ] |
| F025 | 256-color grayscale | Gray not mapped to 232-255 range | [ ] |
| F026 | 256-color cube | RGB not mapped to 16-231 cube | [ ] |
| F027 | 16-color mapping | Bright colors not distinguished | [ ] |
| F028 | ColorMode detection TrueColor | `COLORTERM=truecolor` not detected | [ ] |
| F029 | ColorMode detection 256 | `TERM=xterm-256color` not detected | [ ] |
| F030 | ColorMode fallback | Missing TERM defaults to Mono; Unknown TERM defaults to Color16 | [ ] |
| F031 | RGB to ANSI escape | `Color(1,0,0)` != `\x1b[38;2;255;0;0m` | [ ] |
| F032 | Theme tokyo_night colors | Any color != spec | [ ] |
| F033 | Theme dracula colors | Any color != spec | [ ] |
| F034 | Theme nord colors | Any color != spec | [ ] |
| F035 | Theme monokai colors | Any color != spec | [ ] |
| F036 | CPU gradient green→yellow→red | Incorrect interpolation order | [ ] |
| F037 | Memory gradient distinct | CPU and Memory gradients identical | [ ] |
| F038 | Gradient for_percent(50) | Returns middle color | [ ] |
| F039 | Gradient 3-stop correct | `Gradient::three(R,G,B).sample(0.5)` not equal to G ±ΔE 2 | [ ] |
| F040 | Color alpha handling | Alpha != 1.0 causes rendering issues | [ ] |

### Section C: Widget Layout (F041-F060)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F041 | CpuGrid 8 columns | Default columns != 8 | [ ] |
| F042 | CpuGrid compact mode | Compact not reducing height | [ ] |
| F043 | CpuGrid empty data | Empty data causes panic | [ ] |
| F044 | MemoryBar segments sum | Segments don't sum to 100% | [ ] |
| F045 | MemoryBar labels visible | Labels outside bounds | [ ] |
| F046 | ProcessTable header row | No header row rendered | [ ] |
| F047 | ProcessTable separator | No separator line after header | [ ] |
| F048 | ProcessTable selection | Selected row not highlighted | [ ] |
| F049 | ProcessTable sorting | Sort not affecting order | [ ] |
| F050 | ProcessTable scrolling | Scroll offset incorrect | [ ] |
| F051 | NetworkPanel compact | Compact mode not single line | [ ] |
| F052 | NetworkPanel RX/TX colors | RX not green, TX not red | [ ] |
| F053 | BrailleGraph range | Data outside range clips | [ ] |
| F054 | BrailleGraph width | Graph exceeds bounds | [ ] |
| F055 | Sparkline normalization | Max value not mapped to █ | [ ] |
| F056 | Gauge percentage | 100% not full bar | [ ] |
| F057 | Border styles | All BorderStyle variants render | [ ] |
| F058 | Tree indentation | Child nodes not indented | [ ] |
| F059 | Scrollbar position | Position not matching content | [ ] |
| F060 | Heatmap cell bounds | Cells overflow grid | [ ] |

### Section D: Text Rendering (F061-F075)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F061 | Default text not black | `TextStyle::default().color` is black | [ ] |
| F062 | PID column visible | PID rendered as black | [ ] |
| F063 | USER column visible | USER rendered as black | [ ] |
| F064 | COMMAND column visible | COMMAND rendered as black | [ ] |
| F065 | Interface name visible | eth0/wlan0 rendered as black | [ ] |
| F066 | Selected text white | Selected row not white | [ ] |
| F067 | Header bold | Headers not bold weight | [ ] |
| F068 | Dim text distinct | Dim same as foreground | [ ] |
| F069 | Text truncation | Long text overflows | [ ] |
| F070 | Text alignment | Right-align not working | [ ] |
| F071 | Superscript rendering | to_superscript(123) != "¹²³" | [ ] |
| F072 | Subscript rendering | to_subscript(123) != "₁₂₃" | [ ] |
| F073 | Unicode width | Wide chars break layout | [ ] |
| F074 | Empty string | Empty text causes panic | [ ] |
| F075 | Newline handling | Newline chars break layout | [ ] |

### Section E: Performance (F076-F085)

**Methodology**: Performance tests use `std::time::Instant` with tolerance multipliers for coverage instrumentation (50x overhead acceptable for P1, 5000x for P2).

| ID | Test | Falsification Criterion | Tolerance |
|----|------|------------------------|-----------|
| F076 | Frame budget 16ms | Full 80×24 redraw > 16ms | 50ms w/ coverage |
| F077 | Steady-state alloc | `#[global_allocator]` counter > 0 after 100 frames | N/A |
| F078 | Diff render efficiency | `DiffRenderer::stats().cells_written > 0.1 * total` when unchanged | N/A |
| F079 | Large data handling | BrailleGraph(10K points) paint > 100ms | 500ms w/ coverage |
| F080 | Process table 1000 rows | ProcessTable(1000).paint() > 100ms | 500ms w/ coverage |
| F081 | CellBuffer reuse | `CellBuffer::new()` called per frame | N/A |
| F082 | Color conversion cache | Same RGB→ANSI computed twice in hot path | N/A |
| F083 | String formatting | `format!()` in Widget::paint() | N/A |
| F084 | Widget measure cost | Any widget.measure() > 1ms | 5ms w/ coverage |
| F085 | Paint cost | Full screen paint > 8ms | 40ms w/ coverage |

### Section F: Integration (F086-F100)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F086 | system_dashboard runs | Example crashes or panics | [ ] |
| F087 | All examples compile | Any example fails to build | [ ] |
| F088 | Widget composition | Nested widgets break layout | [ ] |
| F089 | Theme switching | Runtime theme change fails | [ ] |
| F090 | ColorMode runtime | Mode switch causes artifacts | [ ] |
| F091 | Terminal resize | Resize causes crash | [ ] |
| F092 | Empty terminal | 0x0 terminal handled | [ ] |
| F093 | Minimum terminal | 20x10 minimum works | [ ] |
| F094 | Input handling | Keyboard events processed | [ ] |
| F095 | Mouse support | Mouse events cause crash | [ ] |
| F096 | SIGWINCH handling | Window resize signal handled | [ ] |
| F097 | Raw mode cleanup | Terminal not restored on exit | [ ] |
| F098 | Alternate screen | Screen not restored on panic | [ ] |
| F099 | cbtop widget source | Any widget NOT from presentar-terminal | [ ] |
| F100 | Pixel diff baseline | Output differs from baseline >1% | [ ] |

### Section G: Edge Cases & Boundary Conditions (F101-F115)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F101 | NaN data handling | `BrailleGraph::set_data([NaN])` panics | [ ] |
| F102 | Inf data handling | `Gauge::new(f64::INFINITY, 100.0)` panics | [ ] |
| F103 | Negative values | `MemoryBar::new(-50.0)` panics | [ ] |
| F104 | Zero-width terminal | `CellBuffer::new(0, 24)` panics | [ ] |
| F105 | Zero-height terminal | `CellBuffer::new(80, 0)` panics | [ ] |
| F106 | Single-cell render | Widget renders incorrectly in 1×1 | [ ] |
| F107 | UTF-8 boundary | Multi-byte char split causes panic | [ ] |
| F108 | Emoji handling | 👨‍👩‍👧‍👦 (ZWJ sequence) breaks layout | [ ] |
| F109 | RTL text | Arabic/Hebrew text renders incorrectly | [ ] |
| F110 | 100K data points | BrailleGraph with 100K points OOMs | [ ] |
| F111 | Rapid resize | 100 resize events/sec causes crash | [ ] |
| F112 | Theme hot-swap | Theme change mid-render causes artifact | [ ] |
| F113 | Concurrent updates | Race between data update and paint | [ ] |
| F114 | Signal during render | SIGWINCH during paint() corrupts state | [ ] |
| F115 | Ctrl+C cleanup | SIGINT leaves terminal in raw mode | [ ] |

### Section H: Accessibility Compliance (F116-F120)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F116 | Text contrast ratio | Any theme fg/bg contrast < 4.5:1 | [ ] |
| F117 | Color-only information | Critical info uses only color (no text/symbol) | [ ] |
| F118 | Focus indication | Focused widget not visually distinct | [ ] |
| F119 | Keyboard navigable | Any widget unreachable via keyboard | [ ] |
| F120 | Screen reader labels | Widget.accessibility().label is None for interactive | [ ] |

### Section I: Chaos Engineering (F121-F125)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F121 | Fuzzing Input | Panic on random bytes > 1MB | [ ] |
| F122 | Fuzzing Config | Panic on malformed config file | [ ] |
| F123 | Faulty I/O | Panic when data source hangs/errors | [ ] |
| F124 | OOM Recovery | Non-graceful exit on allocation failure | [ ] |
| F125 | Terminal Corrupt | Artifacts persist after random write to stdout | [ ] |

---

## 13. Test Implementation

### 12.1 Automated Tests

```bash
# Run all widget visibility tests
cargo test -p presentar-terminal --test cbtop_visibility

# Run full test suite
cargo test -p presentar-terminal

# Run pixel diff comparison
./scripts/pixel_diff.sh compare system_dashboard
```

### 12.2 Visual Regression

Baseline images stored in `__pixel_baselines__/`.

Comparison tool: ImageMagick `compare -metric AE`.

Threshold: <1% pixel difference (accounting for timing variations).

## 14. Conclusion

This specification provides a rigorous framework for proving that `presentar-terminal` can pixel-perfectly recreate btop/htop-style terminal interfaces. The 125-point falsification checklist ensures that any claim of feature parity can be systematically tested and disproven if incorrect.

### 14.1 Compliance Summary

| Category | Tests | Status |
|----------|-------|--------|
| Symbol Rendering (A) | F001-F020 | ~76 tests implemented |
| Color System (B) | F021-F040 | ~45 tests implemented |
| Widget Layout (C) | F041-F060 | ~30 tests implemented |
| Text Rendering (D) | F061-F075 | ~15 tests implemented |
| Performance (E) | F076-F085 | Requires benchmark harness |
| Integration (F) | F086-F100 | ~6 tests implemented |
| Edge Cases (G) | F101-F115 | Requires fuzzing harness |
| Accessibility (H) | F116-F120 | Requires contrast checker |
| Chaos Engineering (I)| F121-F125 | Requires AFL++/Honggfuzz |

### 14.2 Known Limitations

1. **Performance tests (F076-F085)** require instrumented benchmarks with `cargo criterion`
2. **Pixel diff baseline (F100)** requires populated `__pixel_baselines__/` directory
3. **WCAG contrast verification** requires automated contrast ratio checks

## References

### Visualization & Color Theory
1. Fairchild, M.D. (2013). *Color Appearance Models*. 3rd ed. Wiley.
2. Ware, C. (2012). *Information Visualization: Perception for Design*. 3rd ed. Morgan Kaufmann.
3. Tufte, E.R. (2001). *The Visual Display of Quantitative Information*. 2nd ed. Graphics Press.
4. Few, S. (2009). *Now You See It*. Analytics Press.
5. Stone, M. (2006). "Choosing Colors for Data Visualization". *Perceptual Edge*.
6. Healey, C.G. & Enns, J.T. (2012). "Attention and Visual Memory in Visualization and Computer Graphics". *IEEE TVCG*, 18(7).
7. Borland, D. & Taylor, R.M. (2007). "Rainbow Color Map (Still) Considered Harmful". *IEEE CG&A*, 27(2).
8. Zeileis, A. et al. (2020). "colorspace: A Toolbox for Manipulating and Assessing Colors and Palettes". *Journal of Statistical Software*, 96(1).

### Standards & Specifications
9. Unicode Consortium (2022). *The Unicode Standard, Version 15.0*.
10. W3C (2018). "Web Content Accessibility Guidelines (WCAG) 2.1". W3C Recommendation.
11. W3C (2023). "Accessible Rich Internet Applications (WAI-ARIA) 1.2". W3C Recommendation.
12. ECMA International (2017). "ECMA-48: Control Functions for Coded Character Sets". 5th ed.

### Philosophy of Science
13. Popper, K. (1959). *The Logic of Scientific Discovery*. Hutchinson.
14. Lakatos, I. (1978). *The Methodology of Scientific Research Programmes*. Cambridge UP.

---

**Document History**

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-01-09 | Claude Code | Initial specification |
| 1.1.0 | 2026-01-09 | Claude Code | Added sections 8-10 (Accessibility, Error Handling, Concurrency); Strengthened falsification criteria (F021, F030, F039, F076-F085); Added tolerance methodology; Fixed ColorMode fallback documentation; Added compliance summary |
