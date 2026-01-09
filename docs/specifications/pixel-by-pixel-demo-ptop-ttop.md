# SPEC-024: ptop - A Pixel-Perfect ttop Clone Using presentar-terminal

**Status**: **INCOMPLETE** - Widgets exist, binary does not
**Author**: Claude Code
**Date**: 2026-01-09
**Version**: 2.0.0
**Breaking Change**: Spec rewritten from widget tests to binary requirements

## 1. Executive Summary

### 1.1 What This Spec Requires

Build `ptop`: a **runnable binary** that is visually indistinguishable from `ttop` when run side-by-side.

```bash
# These two commands must produce identical terminal output:
ttop
ptop
```

### 1.2 Current State (Honest Assessment)

| Component | ttop (reference) | ptop (target) | Status |
|-----------|------------------|---------------|--------|
| Binary | `ttop` in PATH | Does not exist | **MISSING** |
| Event loop | 50ms tick, 1s refresh | N/A | **MISSING** |
| Data collectors | sysinfo + /proc | N/A | **MISSING** |
| Keyboard handling | q,h,c,m,p,k,/,↑↓ | N/A | **MISSING** |
| Widgets | ratatui-based | presentar-terminal | **EXISTS** (196 tests) |

### 1.3 Deliverable

```
crates/presentar-terminal/
├── src/bin/ptop.rs          # ← THIS MUST EXIST
├── src/ptop/
│   ├── app.rs               # Application state + collectors
│   ├── ui.rs                # Layout (calls widgets)
│   └── input.rs             # Keyboard handling
└── examples/
    └── system_dashboard.rs  # Demo (already exists, NOT the deliverable)
```

### 1.4 Acceptance Criteria

```bash
# 1. Binary compiles and runs
cargo build --release -p presentar-terminal --bin ptop
./target/release/ptop

# 2. Visual diff test passes
./scripts/visual_diff.sh ttop ptop  # <1% pixel difference

# 3. Keyboard parity
ptop --help  # Same flags as ttop
# q=quit, h=help, c=sort:cpu, m=sort:mem, k=kill, /=filter
```

## 2. Reference Implementation: ttop

### 2.1 ttop Source Structure

Location: `/home/noah/src/trueno-viz/crates/ttop/src/`

```
ttop/src/
├── main.rs        (147 lines)   # Entry point, terminal setup, event loop
├── app.rs         (61KB)        # App state, metric collectors, keybindings
├── panels.rs      (172KB)       # ALL panel rendering code
├── ui.rs          (37KB)        # Layout logic, draw dispatch
├── state.rs       (15KB)        # System state types
├── theme.rs       (7KB)         # Color definitions
└── ring_buffer.rs (13KB)        # History storage
```

### 2.2 ttop Event Loop (what ptop must replicate)

```rust
// From ttop/src/main.rs - THIS IS THE PATTERN
fn run_app(terminal: &mut Terminal, mut app: App, cli: &Cli) -> Result<()> {
    let tick_rate = Duration::from_millis(50);      // UI refresh
    let collect_interval = Duration::from_millis(cli.refresh);  // Data refresh

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;  // Render frame

        if last_frame.elapsed() >= collect_interval {
            app.collect_metrics();  // Fetch real CPU/mem/disk/net/proc
        }

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key(key.code, key.modifiers) {
                    return Ok(());  // Quit
                }
            }
        }
    }
}
```

### 2.3 ttop Panels (what ptop must render identically)

| Panel | ttop function | Size | presentar widget |
|-------|---------------|------|------------------|
| CPU | `panels::draw_cpu()` | ~2000 lines | `BrailleGraph` + `CpuGrid` |
| Memory | `panels::draw_memory()` | ~800 lines | `MemoryBar` |
| Disk | `panels::draw_disk()` | ~600 lines | `Gauge` bars |
| Network | `panels::draw_network()` | ~1000 lines | `NetworkPanel` |
| GPU | `panels::draw_gpu()` | ~1500 lines | `BrailleGraph` + `Gauge` |
| Process | `panels::draw_process()` | ~2500 lines | `ProcessTable` |

## 3. ptop Binary Specification

### 3.1 Required Files

```rust
// crates/presentar-terminal/src/bin/ptop.rs
use presentar_terminal::ptop::{App, ui};
use crossterm::{event, terminal};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let app = App::new(cli.refresh);

    terminal::enable_raw_mode()?;
    // ... terminal setup ...

    run_app(&mut terminal, app, &cli)?;

    terminal::disable_raw_mode()?;
    Ok(())
}
```

### 3.2 Data Collectors (MUST use real system data)

```rust
// crates/presentar-terminal/src/ptop/collectors.rs
pub struct Collectors {
    sys: sysinfo::System,  // Or direct /proc parsing
}

impl Collectors {
    pub fn cpu(&mut self) -> Vec<f64>;           // Per-core usage %
    pub fn memory(&mut self) -> MemoryStats;     // Used/cached/swap
    pub fn disks(&mut self) -> Vec<DiskStats>;   // Mount, used, total
    pub fn network(&mut self) -> Vec<NetStats>;  // Interface, rx, tx
    pub fn processes(&mut self) -> Vec<Process>; // PID, user, cpu%, mem%, cmd
}
```

**NOT ACCEPTABLE:**
```rust
// This is what system_dashboard.rs does - SIMULATED DATA
fn simulate_cpu(count: usize) -> Vec<f64> {
    (0..count).map(|i| 45.0 + 25.0 * (i as f64).sin()).collect()
}
```

### 3.3 Keyboard Handling (MUST match ttop)

| Key | ttop behavior | ptop requirement |
|-----|---------------|------------------|
| `q` | Quit | Quit |
| `h` / `?` | Show help overlay | Show help overlay |
| `c` | Sort processes by CPU | Sort processes by CPU |
| `m` | Sort processes by memory | Sort processes by memory |
| `p` | Sort processes by PID | Sort processes by PID |
| `k` | Kill selected process | Kill selected process |
| `/` | Filter input | Filter input |
| `↑`/`↓` | Navigate process list | Navigate process list |
| `Enter` | Expand/collapse panel | Expand/collapse panel |
| `1-6` | Toggle panel visibility | Toggle panel visibility |

### 3.4 Layout (MUST match ttop exactly)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ ptop - Presentar System Monitor                        uptime: Xd HH:MM:SS │
├────────────────────────────────┬────────────────────────────────────────────┤
│ CPU (45% of height)            │ Memory                                     │
│ ├─ BrailleGraph (history)      │ ├─ BrailleGraph (history)                  │
│ └─ CpuGrid (per-core)          │ └─ MemoryBar (used/cached/swap)            │
├────────────────────────────────┼────────────────────────────────────────────┤
│ Network                        │ Disk                                       │
│ └─ NetworkPanel (eth0, wlan0)  │ └─ Gauge bars per mount                    │
├────────────────────────────────┴────────────────────────────────────────────┤
│ Processes (55% of height)                                                   │
│ └─ ProcessTable (PID, USER, CPU%, MEM%, COMMAND)                            │
├─────────────────────────────────────────────────────────────────────────────┤
│ [q]quit [h]help [c]cpu [m]mem [p]pid [k]kill [/]filter                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 4. Implementation Checklist

### 4.1 Phase 1: Skeleton (Day 1)

- [ ] Create `src/bin/ptop.rs` with terminal setup
- [ ] Create `src/ptop/mod.rs` module structure
- [ ] Implement basic event loop (quit on 'q')
- [ ] Render empty frame with header/footer

### 4.2 Phase 2: Data Collectors (Day 2)

- [ ] Add `sysinfo` dependency or implement /proc parsing
- [ ] Implement `Collectors::cpu()` - real per-core usage
- [ ] Implement `Collectors::memory()` - real used/cached/swap
- [ ] Implement `Collectors::disks()` - real mount points
- [ ] Implement `Collectors::network()` - real interface stats
- [ ] Implement `Collectors::processes()` - real process list

### 4.3 Phase 3: Panel Rendering (Day 3-4)

- [ ] CPU panel: `BrailleGraph` + `CpuGrid` with real data
- [ ] Memory panel: `BrailleGraph` + `MemoryBar` with real data
- [ ] Disk panel: `Gauge` bars per mount with real data
- [ ] Network panel: `NetworkPanel` with real interface data
- [ ] Process panel: `ProcessTable` with real process data

### 4.4 Phase 4: Interactivity (Day 5)

- [ ] Process selection (↑/↓ navigation)
- [ ] Sorting (c/m/p keys)
- [ ] Filtering (/ key + input)
- [ ] Process signals (k key + confirmation)
- [ ] Help overlay (h key)
- [ ] Panel toggle (1-6 keys)

### 4.5 Phase 5: Visual Parity (Day 6)

- [ ] Run `ttop` and `ptop` side-by-side
- [ ] Screenshot comparison
- [ ] Fix any layout/color differences
- [ ] Pixel diff test passes (<1% difference)

---

## 5. EXTREME FALSIFICATION PROTOCOL

**Purpose**: Definitively prove or disprove the claim "ptop is pixel-perfect identical to ttop"

### 5.1 Binary Existence Tests (F200-F204)

| ID | Test | Falsification Criterion | Command |
|----|------|------------------------|---------|
| F200 | ptop binary exists | `which ptop` returns empty | `cargo build -p presentar-terminal --bin ptop && test -x target/debug/ptop` |
| F201 | ptop runs without panic | Exit code != 0 within 1s | `timeout 1 ptop --help` |
| F202 | ptop accepts --refresh flag | `--refresh 500` rejected | `ptop --refresh 500 --help` |
| F203 | ptop accepts --deterministic | Flag not recognized | `ptop --deterministic --help` |
| F204 | ptop version matches spec | Version != "0.1.0" | `ptop --version` |

### 5.2 Terminal Capture Tests (F210-F219)

```bash
# Capture methodology: render to PTY, dump ANSI sequences
script -q -c "ttop --deterministic" /tmp/ttop_capture.txt &
sleep 2 && kill %1
script -q -c "ptop --deterministic" /tmp/ptop_capture.txt &
sleep 2 && kill %1
```

| ID | Test | Falsification Criterion | Threshold |
|----|------|------------------------|-----------|
| F210 | Header line matches | First line differs | Exact match required |
| F211 | Footer keybindings match | Last line differs | Exact match required |
| F212 | Panel border chars match | Any ─│┌┐└┘├┤┬┴┼ differs | Exact match required |
| F213 | Braille chars in CPU panel | Any ⠀-⣿ differs | Exact match required |
| F214 | Block chars in bars | Any ░▒▓█ differs | Exact match required |
| F215 | Color escape sequences | ANSI codes differ >5% | <5% difference |
| F216 | Layout dimensions | Panel sizes differ | ±1 char tolerance |
| F217 | Process table columns | Column headers differ | Exact match required |
| F218 | Numeric formatting | CPU%/MEM% format differs | Same precision |
| F219 | Uptime format | "Xd HH:MM:SS" differs | Exact format match |

### 5.3 Pixel Diff Tests (F220-F229)

```bash
# Methodology: render to virtual framebuffer, screenshot, ImageMagick compare
Xvfb :99 -screen 0 1920x1080x24 &
DISPLAY=:99 xterm -e "ttop --deterministic; sleep 2" &
import -window root /tmp/ttop.png
DISPLAY=:99 xterm -e "ptop --deterministic; sleep 2" &
import -window root /tmp/ptop.png
compare -metric AE /tmp/ttop.png /tmp/ptop.png /tmp/diff.png
```

| ID | Test | Falsification Criterion | Threshold |
|----|------|------------------------|-----------|
| F220 | Full screen pixel diff | >1% pixels differ | <1% (1920x1080 = <20,736 pixels) |
| F221 | CPU panel region diff | >0.5% pixels differ | <0.5% |
| F222 | Memory panel region diff | >0.5% pixels differ | <0.5% |
| F223 | Network panel region diff | >0.5% pixels differ | <0.5% |
| F224 | Disk panel region diff | >0.5% pixels differ | <0.5% |
| F225 | Process table region diff | >1% pixels differ | <1% (scrolling variance) |
| F226 | Header region diff | >0% pixels differ | Exact match (static) |
| F227 | Footer region diff | >0% pixels differ | Exact match (static) |
| F228 | Border region diff | >0% pixels differ | Exact match (static) |
| F229 | Color gradient accuracy | ΔE > 2 in any gradient | ΔE < 2 (perceptual) |

### 5.4 Behavioral Parity Tests (F230-F249)

| ID | Test | Falsification Criterion | Method |
|----|------|------------------------|--------|
| F230 | 'q' quits | Does not exit | Send 'q' via PTY, check exit |
| F231 | 'h' shows help | No overlay appears | Screenshot before/after 'h' |
| F232 | 'c' sorts by CPU | Order unchanged | Capture process list, verify sort |
| F233 | 'm' sorts by memory | Order unchanged | Capture process list, verify sort |
| F234 | 'p' sorts by PID | Order unchanged | Capture process list, verify sort |
| F235 | '/' enables filter | No input field | Screenshot after '/' |
| F236 | '↑' moves selection up | Selection unchanged | Capture before/after |
| F237 | '↓' moves selection down | Selection unchanged | Capture before/after |
| F238 | '1' toggles CPU panel | Panel not toggled | Screenshot before/after |
| F239 | '2' toggles Memory panel | Panel not toggled | Screenshot before/after |
| F240 | '3' toggles Disk panel | Panel not toggled | Screenshot before/after |
| F241 | '4' toggles Network panel | Panel not toggled | Screenshot before/after |
| F242 | '5' toggles GPU panel | Panel not toggled | Screenshot before/after |
| F243 | '6' toggles Process panel | Panel not toggled | Screenshot before/after |
| F244 | Enter expands panel | Panel not expanded | Screenshot before/after |
| F245 | Escape closes overlay | Overlay persists | Screenshot before/after |
| F246 | Resize handling | Crash on SIGWINCH | Send SIGWINCH, check alive |
| F247 | Ctrl+C handling | Does not exit cleanly | Send SIGINT, check cleanup |
| F248 | Mouse scroll in process | List doesn't scroll | Send mouse wheel event |
| F249 | Mouse click on process | Row not selected | Send mouse click event |

### 5.5 Data Accuracy Tests (F250-F259)

| ID | Test | Falsification Criterion | Method |
|----|------|------------------------|--------|
| F250 | CPU % matches /proc/stat | Differs >5% from ground truth | Compare to `mpstat 1 1` |
| F251 | Memory matches /proc/meminfo | Differs >1% from ground truth | Compare to `free -b` |
| F252 | Disk matches /proc/mounts | Missing mount points | Compare to `df -h` |
| F253 | Network matches /proc/net/dev | Differs >5% from ground truth | Compare to `ip -s link` |
| F254 | Process list matches /proc | Missing PIDs | Compare to `ps aux` |
| F255 | CPU core count correct | Core count wrong | Compare to `nproc` |
| F256 | Uptime matches /proc/uptime | Differs >1s | Compare to `uptime -p` |
| F257 | Load average correct | Differs >0.1 | Compare to `uptime` |
| F258 | Swap usage correct | Differs >1% | Compare to `free -b` |
| F259 | Network interface names | Missing interfaces | Compare to `ip link` |

### 5.6 Performance Parity Tests (F260-F269)

| ID | Test | Falsification Criterion | Threshold |
|----|------|------------------------|-----------|
| F260 | Frame time vs ttop | ptop >2x slower | ptop ≤ 2x ttop frame time |
| F261 | Memory usage vs ttop | ptop >2x memory | ptop ≤ 2x ttop RSS |
| F262 | CPU usage vs ttop | ptop >2x CPU | ptop ≤ 2x ttop CPU% |
| F263 | Startup time | >500ms to first frame | <500ms |
| F264 | Input latency | >100ms key response | <100ms |
| F265 | Refresh rate achieved | <30fps at 1s refresh | ≥30fps |
| F266 | No memory leak (5min) | RSS grows >10% | <10% growth |
| F267 | No CPU spike on idle | >5% CPU when idle | <5% CPU |
| F268 | Resize performance | >500ms to re-layout | <500ms |
| F269 | 1000 process handling | Frame drop <30fps | ≥30fps |

### 5.7 Automated Falsification Script

```bash
#!/bin/bash
# scripts/falsify_ptop.sh - Run ALL falsification tests

set -e

PASS=0
FAIL=0

falsify() {
    local id=$1
    local desc=$2
    local cmd=$3

    echo -n "[$id] $desc... "
    if eval "$cmd" >/dev/null 2>&1; then
        echo "PASS"
        ((PASS++))
    else
        echo "FAIL"
        ((FAIL++))
        FAILED_TESTS+=("$id: $desc")
    fi
}

# F200: Binary exists
falsify F200 "ptop binary exists" \
    "cargo build -p presentar-terminal --bin ptop && test -x target/debug/ptop"

# F201: Runs without panic
falsify F201 "ptop runs without panic" \
    "timeout 2 ./target/debug/ptop --deterministic --help"

# F210: Header matches
falsify F210 "Header line matches ttop" \
    "diff <(ttop --deterministic 2>&1 | head -1) <(ptop --deterministic 2>&1 | head -1)"

# F220: Pixel diff <1%
falsify F220 "Full screen pixel diff <1%" \
    "./scripts/pixel_diff.sh ttop ptop 1.0"

# F230: 'q' quits
falsify F230 "'q' key quits" \
    "echo 'q' | timeout 2 ./target/debug/ptop --deterministic"

# F250: CPU accuracy
falsify F250 "CPU % within 5% of mpstat" \
    "./scripts/verify_cpu_accuracy.sh ptop 5"

echo ""
echo "================================"
echo "FALSIFICATION RESULTS: $PASS passed, $FAIL failed"
echo "================================"

if [ $FAIL -gt 0 ]; then
    echo "FAILED TESTS:"
    for t in "${FAILED_TESTS[@]}"; do
        echo "  - $t"
    done
    exit 1
fi
```

### 5.8 Falsification Verdicts

| Result | Meaning | Action |
|--------|---------|--------|
| **ALL PASS** | ptop is pixel-perfect ttop clone | Spec complete, ship it |
| **F200-F204 FAIL** | Binary doesn't exist/run | Phase 1 incomplete |
| **F210-F219 FAIL** | Terminal output differs | Fix rendering |
| **F220-F229 FAIL** | Visual appearance differs | Fix layout/colors |
| **F230-F249 FAIL** | Behavior differs | Fix input handling |
| **F250-F259 FAIL** | Data inaccurate | Fix collectors |
| **F260-F269 FAIL** | Performance insufficient | Optimize |

### 5.9 Minimum Viable Falsification

Before claiming "ptop complete", these MUST pass:

```bash
# MANDATORY - No exceptions
F200  # Binary exists
F201  # Runs without panic
F210  # Header matches
F220  # <1% pixel diff
F230  # 'q' quits
F250  # CPU accuracy

# Run: ./scripts/falsify_ptop.sh --minimum
```

**If ANY of these fail, ptop is NOT complete. Period.**

---

## 6. What Already Exists (Widget Layer)

The following widgets are implemented and tested (196 falsification tests pass):

| Widget | Tests | Status |
|--------|-------|--------|
| `BrailleGraph` | F001-F020 | ✓ Renders braille patterns |
| `CpuGrid` | F041-F043 | ✓ Per-core sparkline grid |
| `MemoryBar` | F044-F045 | ✓ Segmented memory bar |
| `NetworkPanel` | F051-F052 | ✓ Interface list with sparklines |
| `ProcessTable` | F046-F050 | ✓ Sortable process table |
| `Gauge` | F056 | ✓ Horizontal bar |
| `Border` | F057 | ✓ Box drawing |

**These widgets are components.** They are not a product. ptop is the product.

## 6. Background (Preserved)

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
| F001 | Braille empty | `BRAILLE_UP[0]` is not `\u{2800}` (Space) | [ ] |
| F002 | Braille full | `BRAILLE_UP[24]` is not `\u{28FF}` (⣿) | [ ] |
| F003 | Braille length | `BRAILLE_UP.len() != 25` | [ ] |
| F004 | Block empty | `BLOCK_UP[0]` is not ` ` | [ ] |
| F005 | Block full | `BLOCK_UP[24]` is not `\u{2588}` (█) | [ ] |
| F006 | Block length | `BLOCK_UP.len() != 25` | [ ] |
| F007 | TTY Purity | Any character in `TTY_UP` has code point > 127 | [ ] |
| F008 | Sparkline levels | `SPARKLINE.len() != 8` | [ ] |
| F009 | Sparkline range | `SPARKLINE[0]` != `\u{2581}` or `SPARKLINE[7]` != `\u{2588}` | [ ] |
| F010 | Superscript | `to_superscript("0123456789")` contains non-superscript chars | [ ] |
| F011 | Subscript | `to_subscript("0123456789")` contains non-subscript chars | [ ] |
| F012 | Braille Formula | `left=2, right=3` does not yield `\u{28B6}` (⣦) | [ ] |
| F013 | Braille Corner L | `left=4, right=0` does not yield `\u{2807}` (⡇) | [ ] |
| F014 | Braille Corner R | `left=0, right=4` does not yield `\u{2838}` (⢸) | [ ] |
| F015 | Monotonicity | `BLOCK_UP[i]` visual density <= `BLOCK_UP[i-1]` | [ ] |
| F016 | Unicode Range | Any `BRAILLE_UP` char outside `U+2800`..`U+28FF` | [ ] |
| F017 | Inversion | `BRAILLE_DOWN[24]` is not `\u{28FF}` | [ ] |
| F018 | Custom Fallback | `CustomSet::None` does not render as `SymbolSet::Braille` | [ ] |
| F019 | Set Default | `SymbolSet::default()` is not `SymbolSet::Braille` | [ ] |
| F020 | Box Geometry | Missing any of: `─│┌┐└┘├┤┬┴┼╭╮╯╰` | [ ] |

### Section B: Color System (F021-F040)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F021 | LAB Midpoint | `interpolate_lab(Red, Blue, 0.5)` ΔE > 2.0 vs target | [ ] |
| F022 | Stop 0.0 | `gradient.sample(0.0)` != first stop color | [ ] |
| F023 | Stop 1.0 | `gradient.sample(1.0)` != last stop color | [ ] |
| F024 | Clamping | `sample(-1.0)` or `sample(2.0)` does not return edge stops | [ ] |
| F025 | 256 Grayscale | `Color::gray(0.5)` maps outside `232..255` range | [ ] |
| F026 | 256 Cube | `Color::RGB(255,0,0)` maps to index != 196 | [ ] |
| F027 | 16-Color | Bright variant matches normal variant exactly | [ ] |
| F028 | Env TrueColor | `COLORTERM=truecolor` results in `ColorMode::Color16` | [ ] |
| F029 | Env 256 | `TERM=xterm-256color` results in `ColorMode::Color16` | [ ] |
| F030 | Fallback Logic | Missing `TERM` results in anything other than `ColorMode::Mono` | [ ] |
| F031 | ANSI Sequence | `Color::RED.to_ansi_fg()` != `\x1b[38;2;255;0;0m` | [ ] |
| F032 | Tokyo Night | `bg` color != `#1a1b26` | [ ] |
| F033 | Dracula | `bg` color != `#282a36` | [ ] |
| F034 | Nord | `bg` color != `#2e3440` | [ ] |
| F035 | Monokai | `bg` color != `#272822` | [ ] |
| F036 | CPU Grad Order | `gradient.sample(0.1)` luminance < `sample(0.9)` luminance | [ ] |
| F037 | Gradient Dist | `CpuGrad` and `MemGrad` have same hex values | [ ] |
| F038 | Percent Match | `gradient.for_percent(50)` != `gradient.sample(0.5)` | [ ] |
| F039 | 3-Stop Logic | Middle stop at `t=0.5` does not equal input color | [ ] |
| F040 | Alpha Blending | `Alpha < 1.0` is ignored by `DiffRenderer` | [ ] |

### Section C: Widget Layout (F041-F060)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F041 | CpuGrid Grid | 16 cores with `cols=8` results in height != 2 | [ ] |
| F042 | CpuGrid Compact | `compact()` does not remove whitespace between bars | [ ] |
| F043 | CpuGrid Empty | `CpuGrid::new(vec![])` results in panic | [ ] |
| F044 | Memory Segments | Bar length sum != widget width - labels | [ ] |
| F045 | Memory Labels | Used/Free labels missing from `MemoryBar` output | [ ] |
| F046 | Proc Header | First line does not contain `PID` and `COMMAND` | [ ] |
| F047 | Proc Sep | Second line is not a separator character | [ ] |
| F048 | Selection | Selected row lacks `\x1b[7m` (Inverse) or unique bg | [ ] |
| F049 | Proc Sorting | `sort_by(Cpu)` leaves lower CPU usage above higher | [ ] |
| F050 | Scrolling | `selected=50` in `height=10` leaves `scroll_offset=0` | [ ] |
| F051 | Net Compact | `NetworkPanel::compact()` height > 1 per interface | [ ] |
| F052 | Net Directions | RX lacks `↓` or TX lacks `↑` symbol | [ ] |
| F053 | Graph Clipping | Values > 100.0 or < 0.0 cause crash | [ ] |
| F054 | Graph Bounds | `paint()` writes outside assigned `Rect` | [ ] |
| F055 | Spark Normal | `[10, 20, 30]` and `[100, 200, 300]` render different shapes | [ ] |
| F056 | Gauge 100% | `100%` renders any empty cells (`░`) | [ ] |
| F057 | Border Join | Adjacent borders show gaps or broken intersections | [ ] |
| F058 | Tree Nesting | Depth 2 node has same indentation as Depth 1 | [ ] |
| F059 | Scrollbar | Content at 50% results in thumb at top/bottom | [ ] |
| F060 | Heatmap | Cell at `(x,y)` rendered at `(y,x)` | [ ] |

### Section D: Text Rendering (F061-F075)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F061 | Default Style | `TextStyle::default()` uses black foreground on black background | [ ] |
| F062 | Column: PID | `PID` column values are missing or zero-padded incorrectly | [ ] |
| F063 | Column: USER | `USER` column contains UID instead of name | [ ] |
| F064 | Column: CMD | `COMMAND` column is empty for kernel processes | [ ] |
| F065 | Net Labels | `eth0` label contains non-printable characters | [ ] |
| F066 | Highlight Text | Selected row text color == non-selected text color | [ ] |
| F067 | Weight: Bold | `FontWeight::Bold` does not emit `\x1b[1m` | [ ] |
| F068 | Weight: Dim | `FontWeight::Dim` does not emit `\x1b[2m` | [ ] |
| F069 | Truncation | String of 100 chars in 10-wide cell doesn't end in `..` | [ ] |
| F070 | Alignment | Right-aligned numeric "5" is at `x=0` in 5-wide cell | [ ] |
| F071 | Super Map | `to_superscript('1')` != `¹` (`\u{00B9}`) | [ ] |
| F072 | Sub Map | `to_subscript('1')` != `₁` (`\u{2081}`) | [ ] |
| F073 | Unicode Width | `한` (wide) takes only 1 cell in `CellBuffer` | [ ] |
| F074 | Zero String | `""` results in index out of bounds | [ ] |
| F075 | Line Breaks | `\n` in string results in vertical line shift during `paint()` | [ ] |

### Section E: Performance (F076-F085)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F076 | Frame Budget | `paint()` + `flush()` for 80x24 area > 16.6ms | [ ] |
| F077 | Warmup Alloc | `GlobalAlloc` counter increases after frame 100 | [ ] |
| F078 | Diff Efficiency | `flush()` returns > 5% changed cells for static UI | [ ] |
| F079 | Scalability | `BrailleGraph` with 10^6 points paint() > 50ms | [ ] |
| F080 | Table Load | `ProcessTable` with 5000 rows paint() > 20ms | [ ] |
| F081 | Buffer Reuse | `CellBuffer` heap address changes between frames | [ ] |
| F082 | Color Cache | `RGB -> ANSI` conversion happens > 1 time per unique color | [ ] |
| F083 | Hot Path Allocs | `std::fmt::format!` called inside any `Widget::paint` | [ ] |
| F084 | Measurement | `widget.measure()` takes > 1.0ms | [ ] |
| F085 | Canvas Cost | `fill_rect` cost > 2ns per cell | [ ] |

### Section F: Integration (F086-F100)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F086 | App Runtime | `ptop` binary panics within 60 seconds of start | [ ] |
| F087 | Compilation | `cargo build --features ptop` fails | [ ] |
| F088 | Composition | Nested `Border` in `Border` corrupts inner title | [ ] |
| F089 | Hot Theme | Changing `Theme` at runtime leaves artifacts | [ ] |
| F090 | Mode Toggle | Switching `TrueColor -> Mono` requires app restart | [ ] |
| F091 | Resize Safety | `SIGWINCH` results in `IndexOutOfBounds` | [ ] |
| F092 | Zero Terminal | `0x0` terminal size results in divide-by-zero | [ ] |
| F093 | Min Terminal | `20x10` size results in overlapping text | [ ] |
| F094 | Event Loop | Keypress 'q' does not terminate process | [ ] |
| F095 | Mouse Trap | Mouse click on CPU graph results in panic | [ ] |
| F096 | Signal Hold | UI continues drawing during `SIGSTOP` | [ ] |
| F097 | Terminal Exit | Terminal state (Raw/Alt) not restored on `Ctrl+C` | [ ] |
| F098 | Panic Clean | App panic leaves terminal in broken state | [ ] |
| F099 | Widget Origin | `ptop` uses any widget from `tui-rs` or `ratatui` | [ ] |
| F100 | Pixel Diff | `compare -metric AE` vs `ttop` baseline > 1.0% | [ ] |

### Section G: Accessibility & Input (F101-F115)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F101 | Contrast Ratio | `Foreground` vs `Background` contrast < 4.5:1 | [ ] |
| F102 | UI Contrast | `Border` or `Graph` color vs `Background` contrast < 3.0:1 | [ ] |
| F103 | Protanopia | Red/Green gradients remain indistinguishable in Protan mode | [ ] |
| F104 | Deuteranopia | Red/Green gradients remain indistinguishable in Deutan mode | [ ] |
| F105 | Tab Navigation | `Tab` key does not cycle focus between panels | [ ] |
| F106 | Focus Cue | Focused panel has same border color as unfocused | [ ] |
| F107 | Screen Reader | `accessibility().label` is `None` for any visible widget | [ ] |
| F108 | Motion | `reduce_motion=true` does not disable graph animations | [ ] |
| F109 | Key Hints | Keybindings footer is missing or incorrect | [ ] |
| F110 | Font Scale | `TermFontSize=24` results in text truncation | [ ] |
| F111 | Input Lag | `Key -> Screen` latency > 50ms | [ ] |
| F112 | Mouse Wheel | `Scroll` event ignored by `ProcessTable` | [ ] |
| F113 | Selection | `Click` event fails to update `selected` index | [ ] |
| F114 | Artifacts | Fast dragging of terminal window leaves ghost characters | [ ] |
| F115 | Clipboard | Copying `BrailleGraph` results in replacement chars () | [ ] |

### Section H: Stability & Stress (F116-F125)

| ID | Test | Falsification Criterion | Pass |
|----|------|------------------------|------|
| F116 | Soak Test | Memory usage increases by > 1MB over 1 hour | [ ] |
| F117 | Resize Bomb | 1000 resizes/sec results in race condition or crash | [ ] |
| F118 | Data Flood | `1MHz` data update rate freezes UI thread | [ ] |
| F119 | Empty State | All widgets fail to render when data is `[]` | [ ] |
| F120 | Floating Point | `NaN` or `Inf` metrics result in app panic | [ ] |
| F121 | UTF-8 Fuzz | Random byte input stream crashes `InputHandler` | [ ] |
| F122 | Config Fuzz | 1KB random noise in `config.toml` causes panic | [ ] |
| F123 | I/O Timeout | Blocked `stdout` results in unbuffered memory growth | [ ] |
| F124 | Alt-Tab | App fails to redraw after returning from background | [ ] |
| F125 | Thread Safety | `App::on_tick` and `UI::draw` on different threads crash | [ ] |

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
| 1.0.0 | 2026-01-09 | Claude Code | Initial specification - widget-focused |
| 1.1.0 | 2026-01-09 | Claude Code | Added accessibility, error handling, concurrency sections; 196 widget tests |
| **2.0.0** | 2026-01-09 | Claude Code | **BREAKING**: Rewrote spec from widget tests to ptop binary requirements. Acknowledged that widgets exist but product (ptop binary) does not. Added: reference implementation analysis (ttop source), binary specification, data collector requirements, keyboard handling matrix, implementation checklist with phases. Status changed to INCOMPLETE. |
