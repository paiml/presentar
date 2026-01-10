# SPEC-024: ptop - A Pixel-Perfect ttop Clone Using presentar-terminal

**Status**: **FAILING** - 40% parity, 60% missing
**Author**: Claude Code
**Date**: 2026-01-10
**Version**: 4.2.0
**Revision**: Tightened performance tolerances (1.5x -> 1.0x parity) and input latency (<16ms).
**Breaking Change**: Honest gap assessment. Previous claims of "85% complete" were FALSE.

---

## 1. Executive Summary

### 1.1 The Claim We Must Prove

> "presentar-terminal can build ANYTHING that ttop/btop/htop can build, pixel-for-pixel identical."

### 1.2 Current Reality (Honest Assessment)

| Component | ttop Lines | ptop Lines | Parity | Status |
|-----------|-----------|-----------|--------|--------|
| **Core UI** | 7,619 | 2,724 | 36% | **FAILING** |
| **Analyzers** | 12,847 | 0 | 0% | **FAILING** |
| **Total** | 20,466 | 2,724 | **13%** | **FAILING** |

**Previous claim**: "85% complete" - **FALSE**
**Actual state**: 13% code parity, ~40% visual parity

### 1.3 What ttop Has That ptop Does NOT

#### Analyzers (17 modules, 370KB of intelligence)

| Analyzer | ttop Lines | ptop Status | Data Source |
|----------|-----------|-------------|-------------|
| `connections.rs` | 1,200 | **STUB** | `/proc/net/tcp`, GeoIP |
| `containers.rs` | 420 | **MISSING** | Docker/Podman API |
| `disk_entropy.rs` | 665 | **MISSING** | `/dev/urandom` sampling |
| `disk_io.rs` | 930 | **PARTIAL** | `/proc/diskstats` |
| `file_analyzer.rs` | 1,340 | **MISSING** | `walkdir`, inode stats |
| `geoip.rs` | 1,765 | **MISSING** | MaxMind GeoLite2 |
| `gpu_procs.rs` | 290 | **MISSING** | `nvidia-smi`, AMDGPU |
| `network_stats.rs` | 760 | **MISSING** | `/proc/net/dev` extended |
| `process_extra.rs` | 575 | **MISSING** | `/proc/[pid]/`, cgroups |
| `psi.rs` | 248 | **STUB** | `/proc/pressure/*` |
| `sensor_health.rs` | 1,030 | **MISSING** | `/sys/class/hwmon/` |
| `storage.rs` | 800 | **MISSING** | SMART data, FS analysis |
| `swap.rs` | 660 | **MISSING** | `/proc/swaps`, pressure |
| `treemap.rs` | 1,375 | **STUB** | File system scanning |

#### Panel Features Missing

| Panel | ttop Feature | ptop Status |
|-------|--------------|-------------|
| CPU | Per-core frequency scaling indicators | **MISSING** |
| CPU | Turbo boost detection with ⚡ icon | **PARTIAL** |
| CPU | CPU governor display | **MISSING** |
| Memory | ZRAM compression ratio | **PARTIAL** |
| Memory | Memory pressure indicator | **MISSING** |
| Memory | Huge pages tracking | **MISSING** |
| Disk | SMART health status | **MISSING** |
| Disk | I/O scheduler display | **MISSING** |
| Disk | Encryption detection | **MISSING** |
| Network | Packet drop/error rates | **MISSING** |
| Network | GeoIP for remote IPs | **MISSING** |
| Network | Connection state machine | **PARTIAL** |
| Process | cgroup membership | **MISSING** |
| Process | I/O priority (ionice) | **MISSING** |
| Process | OOM score | **MISSING** |
| Process | CPU affinity | **MISSING** |
| GPU | VRAM usage per process | **MISSING** |
| GPU | Temperature/power draw | **MISSING** |
| Containers | Docker container stats | **MISSING** |
| Containers | Podman support | **MISSING** |
| Sensors | Fan RPM | **MISSING** |
| Sensors | Voltage rails | **MISSING** |
| Treemap | Real file scanning | **MISSING** |
| Files | Hot files (inotify) | **MISSING** |
| Files | Duplicate detection | **MISSING** |

### 1.4 Acceptance Criteria (Updated)

```bash
# ALL of these must pass before claiming "pixel-perfect"
./scripts/falsify_ptop.sh --all

# Expected output:
# F500-F517: Analyzer Parity     17/17 PASS
# F600-F650: Panel Features      51/51 PASS
# F700-F730: Pixel Comparison    31/31 PASS
# F800-F820: Data Accuracy       21/21 PASS
# F900-F905: Anti-Regression     6/6 PASS
#
# TOTAL: 126/126 PASS
# VERDICT: PIXEL-PERFECT ACHIEVED
```

---

## 2. Reference Implementation Analysis

### 2.1 ttop Source Structure

```
/home/noah/src/trueno-viz/crates/ttop/src/
├── main.rs           (147 lines)    # Entry, terminal setup
├── app.rs            (1,795 lines)  # State, keybindings
├── panels.rs         (4,684 lines)  # ALL rendering
├── ui.rs             (1,140 lines)  # Layout dispatch
├── state.rs          (~500 lines)   # Type definitions
├── theme.rs          (~250 lines)   # Colors
├── ring_buffer.rs    (~450 lines)   # History storage
└── analyzers/        (12,847 lines) # THE INTELLIGENCE
    ├── mod.rs
    ├── connections.rs
    ├── containers.rs
    ├── disk_entropy.rs
    ├── disk_io.rs
    ├── file_analyzer.rs
    ├── geoip.rs
    ├── gpu_procs.rs
    ├── network_stats.rs
    ├── process_extra.rs
    ├── psi.rs
    ├── sensor_health.rs
    ├── storage.rs
    ├── swap.rs
    └── treemap.rs
```

### 2.2 ptop Source Structure (Current)

```
/home/noah/src/presentar/crates/presentar-terminal/src/ptop/
├── mod.rs      (10 lines)
├── app.rs      (557 lines)    # Basic state, sysinfo only
└── ui.rs       (2,157 lines)  # Panel rendering
                               # NO ANALYZERS DIRECTORY
```

### 2.3 Line-by-Line Gap Analysis

| File | ttop | ptop | Gap |
|------|------|------|-----|
| app.rs | 1,795 | 557 | -1,238 (69% missing) |
| panels/ui.rs | 5,824 | 2,157 | -3,667 (63% missing) |
| analyzers/ | 12,847 | 0 | -12,847 (100% missing) |
| **TOTAL** | 20,466 | 2,724 | **-17,742 (87% missing)** |

---

## 3. TUI Pixel Comparison Tooling Specification

### 3.1 Film Studio Grade Color Comparison

Following VFX industry standards (ACES, DCI-P3), we define a TUI comparison pipeline that measures:

1. **Structural Similarity (SSIM)** - Layout parity
2. **CIEDE2000 (ΔE00)** - Perceptual color difference
3. **Character-level diff** - Exact glyph matching
4. **ANSI sequence diff** - Escape code parity

### 3.2 Comparison Pipeline Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    TUI Pixel Comparison Pipeline                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐           │
│  │  ttop    │    │  ptop    │    │ Capture  │    │ Compare  │           │
│  │ --determ │───▶│ --determ │───▶│  Engine  │───▶│  Engine  │           │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘           │
│       │               │               │               │                  │
│       ▼               ▼               ▼               ▼                  │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐           │
│  │  ANSI    │    │  ANSI    │    │   Cell   │    │  Report  │           │
│  │ Capture  │    │ Capture  │    │  Buffer  │    │ + Diff   │           │
│  │  (.ans)  │    │  (.ans)  │    │  Matrix  │    │  Image   │           │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Capture Methodology

#### 3.3.1 Deterministic Mode Requirements

Both `ttop` and `ptop` MUST support `--deterministic` flag that:

- Freezes timestamps to `2026-01-01 00:00:00`
- Uses fixed seed for any random values
- Disables animations/transitions
- Uses synthetic static data:
  - CPU: `[45.0, 32.0, 67.0, 12.0, 89.0, 23.0, 56.0, 78.0]` per core
  - Memory: `18.2GB / 32.0GB`
  - Processes: Fixed list of 20 processes
  - Network: Fixed 1.2MB/s RX, 345KB/s TX

#### 3.3.2 Capture Commands

```bash
#!/bin/bash
# scripts/capture_tui.sh

# Terminal size (mandatory for comparison)
export COLUMNS=120
export LINES=40

# Capture ttop
script -q -c "ttop --deterministic 2>&1" /tmp/ttop_raw.ans &
TTOP_PID=$!
sleep 2
kill -TERM $TTOP_PID 2>/dev/null

# Capture ptop
script -q -c "ptop --deterministic 2>&1" /tmp/ptop_raw.ans &
PTOP_PID=$!
sleep 2
kill -TERM $PTOP_PID 2>/dev/null

# Strip timing artifacts
cat /tmp/ttop_raw.ans | sed 's/\x1b\[[0-9;]*[a-zA-Z]//g' > /tmp/ttop_clean.txt
cat /tmp/ptop_raw.ans | sed 's/\x1b\[[0-9;]*[a-zA-Z]//g' > /tmp/ptop_clean.txt
```

### 3.4 Comparison Metrics

#### 3.4.1 Character-Level Diff (Metric: CLD)

```rust
/// Character-Level Difference score
/// 0.0 = identical, 1.0 = completely different
fn character_level_diff(ttop: &CellBuffer, ptop: &CellBuffer) -> f64 {
    let total_cells = ttop.width() * ttop.height();
    let mut diff_count = 0;

    for y in 0..ttop.height() {
        for x in 0..ttop.width() {
            let t = ttop.get(x, y);
            let p = ptop.get(x, y);
            if t.symbol != p.symbol {
                diff_count += 1;
            }
        }
    }

    diff_count as f64 / total_cells as f64
}
```

**Threshold**: CLD < 0.01 (less than 1% character difference)

#### 3.4.2 CIEDE2000 Color Difference (Metric: ΔE00)

```rust
/// CIEDE2000 color difference (perceptual)
/// Following CIE Technical Report 142-2001
fn ciede2000(lab1: Lab, lab2: Lab) -> f64 {
    // Implementation of CIEDE2000 formula
    // Returns ΔE00 value
    // < 1.0 = imperceptible
    // 1-2 = barely perceptible
    // 2-10 = noticeable
    // > 10 = very different
}

/// Average color difference across all cells
fn average_delta_e(ttop: &CellBuffer, ptop: &CellBuffer) -> f64 {
    let mut total_de = 0.0;
    let mut count = 0;

    for y in 0..ttop.height() {
        for x in 0..ttop.width() {
            let t_fg = ttop.get(x, y).fg;
            let p_fg = ptop.get(x, y).fg;

            let t_lab = rgb_to_lab(t_fg);
            let p_lab = rgb_to_lab(p_fg);

            total_de += ciede2000(t_lab, p_lab);
            count += 1;
        }
    }

    total_de / count as f64
}
```

**Threshold**: Average ΔE00 < 2.0 (barely perceptible)

#### 3.4.3 Structural Similarity Index (SSIM)

```rust
/// SSIM for TUI comparison
/// Compares local patterns rather than pixel-by-pixel
fn tui_ssim(ttop: &CellBuffer, ptop: &CellBuffer) -> f64 {
    // Convert to luminance grid
    // Apply 8x8 sliding window
    // Compute SSIM per window
    // Return mean SSIM
}
```

**Threshold**: SSIM > 0.95 (95% structural similarity)

#### 3.4.4 ANSI Escape Sequence Diff

```rust
/// Compare raw ANSI escape sequences
fn ansi_sequence_diff(ttop_raw: &[u8], ptop_raw: &[u8]) -> AnsiDiffReport {
    // Parse escape sequences
    // Compare SGR (color) codes
    // Compare cursor positioning
    // Report differences
}
```

### 3.5 Visual Diff Output

```
╔══════════════════════════════════════════════════════════════════════════════╗
║                    TUI PIXEL COMPARISON REPORT                                ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  Reference: ttop --deterministic (120x40)                                     ║
║  Target:    ptop --deterministic (120x40)                                     ║
╠══════════════════════════════════════════════════════════════════════════════╣
║                                                                               ║
║  METRIC                    VALUE           THRESHOLD       STATUS             ║
║  ─────────────────────────────────────────────────────────────────────────── ║
║  Character Diff (CLD)      0.0023          < 0.01          ✓ PASS             ║
║  Color Diff (ΔE00)         1.45            < 2.00          ✓ PASS             ║
║  Structural (SSIM)         0.987           > 0.95          ✓ PASS             ║
║  ANSI Sequence Match       99.2%           > 98%           ✓ PASS             ║
║                                                                               ║
║  ─────────────────────────────────────────────────────────────────────────── ║
║  PANEL BREAKDOWN                                                              ║
║  ─────────────────────────────────────────────────────────────────────────── ║
║  CPU Panel                 CLD: 0.001      ΔE: 0.8         ✓ PASS             ║
║  Memory Panel              CLD: 0.002      ΔE: 1.2         ✓ PASS             ║
║  Disk Panel                CLD: 0.003      ΔE: 1.5         ✓ PASS             ║
║  Network Panel             CLD: 0.001      ΔE: 0.9         ✓ PASS             ║
║  Process Panel             CLD: 0.004      ΔE: 2.1         ⚠ WARN             ║
║  Connections Panel         CLD: 0.052      ΔE: 8.5         ✗ FAIL             ║
║  Treemap Panel             CLD: 0.089      ΔE: 12.3        ✗ FAIL             ║
║                                                                               ║
║  ─────────────────────────────────────────────────────────────────────────── ║
║  DIFF VISUALIZATION (cells with differences highlighted)                      ║
║  ─────────────────────────────────────────────────────────────────────────── ║
║                                                                               ║
║  Row 35:  ████████░░░░░░ vs ████████████░░   (8 cells differ)                ║
║  Row 36:  tcp 192.168... vs tcp 127.0.0...   (content mismatch)              ║
║  Row 37:  ░░░░░░░░░░░░░░ vs ████████████░░   (missing data)                  ║
║                                                                               ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  VERDICT: FAILING - 2 panels below threshold                                  ║
║  Action Required: Implement connections.rs and treemap.rs analyzers          ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

### 3.6 Comparison Tool Implementation

```rust
// crates/presentar-terminal/src/tools/tui_compare.rs

pub struct TuiComparisonConfig {
    /// Character-level difference threshold (0.0-1.0)
    pub cld_threshold: f64,
    /// CIEDE2000 color difference threshold
    pub delta_e_threshold: f64,
    /// Structural similarity threshold (0.0-1.0)
    pub ssim_threshold: f64,
    /// Per-panel thresholds (optional stricter limits)
    pub panel_thresholds: HashMap<String, PanelThreshold>,
}

impl Default for TuiComparisonConfig {
    fn default() -> Self {
        Self {
            cld_threshold: 0.01,      // <1% character diff
            delta_e_threshold: 2.0,   // Barely perceptible color
            ssim_threshold: 0.95,     // 95% structural match
            panel_thresholds: HashMap::new(),
        }
    }
}

pub struct TuiComparisonResult {
    pub passed: bool,
    pub cld: f64,
    pub delta_e: f64,
    pub ssim: f64,
    pub panel_results: Vec<PanelResult>,
    pub diff_cells: Vec<DiffCell>,
}

pub fn compare_tui(
    reference: &CellBuffer,
    target: &CellBuffer,
    config: &TuiComparisonConfig,
) -> TuiComparisonResult {
    // Implementation
}
```

### 3.7 CLI Tool

```bash
# Compare ttop vs ptop
cargo run --bin tui-compare -- \
    --reference "ttop --deterministic" \
    --target "ptop --deterministic" \
    --size 120x40 \
    --output report.html \
    --threshold-cld 0.01 \
    --threshold-delta-e 2.0

# Generate diff image
cargo run --bin tui-compare -- \
    --reference "ttop --deterministic" \
    --target "ptop --deterministic" \
    --diff-image /tmp/diff.png \
    --highlight-mode heatmap
```

---

## 4. Analyzer Implementation Specification

### 4.1 Analyzer Trait

```rust
// crates/presentar-terminal/src/ptop/analyzers/mod.rs

pub trait Analyzer: Send + Sync {
    /// Analyzer name for logging/display
    fn name(&self) -> &'static str;

    /// Collect data from system
    fn collect(&mut self) -> Result<(), AnalyzerError>;

    /// Get collection interval
    fn interval(&self) -> Duration;

    /// Check if analyzer is available on this system
    fn available(&self) -> bool;
}
```

### 4.2 Required Analyzers

#### 4.2.1 ConnectionsAnalyzer

```rust
pub struct ConnectionsAnalyzer {
    connections: Vec<TcpConnection>,
    geoip_db: Option<MaxMindReader>,
}

pub struct TcpConnection {
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: IpAddr,
    pub remote_port: u16,
    pub state: TcpState,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub geo_info: Option<GeoInfo>,  // Country, city, ASN
}
```

**Data sources:**
- `/proc/net/tcp` - IPv4 connections
- `/proc/net/tcp6` - IPv6 connections
- `/proc/[pid]/fd/` - Process to socket mapping
- MaxMind GeoLite2 - IP geolocation

#### 4.2.2 ContainersAnalyzer

```rust
pub struct ContainersAnalyzer {
    docker_client: Option<DockerClient>,
    podman_client: Option<PodmanClient>,
    containers: Vec<ContainerInfo>,
}

pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub cpu_percent: f64,
    pub mem_usage: u64,
    pub mem_limit: u64,
    pub net_rx: u64,
    pub net_tx: u64,
    pub pids: u32,
}
```

**Data sources:**
- Docker socket: `/var/run/docker.sock`
- Podman socket: `/run/podman/podman.sock`
- cgroup stats: `/sys/fs/cgroup/`

#### 4.2.3 DiskEntropyAnalyzer

```rust
pub struct DiskEntropyAnalyzer {
    devices: Vec<DiskEntropyInfo>,
}

pub struct DiskEntropyInfo {
    pub device: String,
    pub entropy: f64,           // 0.0-1.0 (1.0 = encrypted/compressed)
    pub is_encrypted: bool,
    pub encryption_type: Option<String>,  // LUKS, VeraCrypt, etc.
}
```

**Data sources:**
- `/dev/[device]` - Sample reads for entropy calculation
- `/sys/block/[device]/dm/` - Device mapper info
- `cryptsetup status` - LUKS detection

#### 4.2.4 ProcessExtraAnalyzer

```rust
pub struct ProcessExtraAnalyzer {
    process_extras: HashMap<u32, ProcessExtra>,
}

pub struct ProcessExtra {
    pub pid: u32,
    pub cgroup: String,
    pub io_priority: IoPriority,
    pub oom_score: i32,
    pub oom_score_adj: i32,
    pub cpu_affinity: Vec<u32>,
    pub numa_node: Option<u32>,
    pub scheduler: Scheduler,
    pub nice: i32,
}
```

**Data sources:**
- `/proc/[pid]/cgroup`
- `/proc/[pid]/oom_score`
- `/proc/[pid]/oom_score_adj`
- `/proc/[pid]/status` (Cpus_allowed)
- `sched_getaffinity()` syscall

#### 4.2.5 SensorHealthAnalyzer

```rust
pub struct SensorHealthAnalyzer {
    sensors: Vec<SensorReading>,
}

pub struct SensorReading {
    pub device: String,
    pub sensor_type: SensorType,  // Temperature, Fan, Voltage, Current, Power
    pub label: String,
    pub value: f64,
    pub unit: &'static str,
    pub critical: Option<f64>,
    pub warning: Option<f64>,
    pub status: SensorStatus,
}
```

**Data sources:**
- `/sys/class/hwmon/hwmon*/`
- `/sys/class/thermal/thermal_zone*/`
- ACPI tables

### 4.3 Analyzer Registration

```rust
// In app.rs
pub struct App {
    // ...existing fields...

    // Analyzers
    analyzers: AnalyzerRegistry,
}

pub struct AnalyzerRegistry {
    connections: Option<ConnectionsAnalyzer>,
    containers: Option<ContainersAnalyzer>,
    disk_entropy: Option<DiskEntropyAnalyzer>,
    disk_io: Option<DiskIoAnalyzer>,
    file_analyzer: Option<FileAnalyzer>,
    geoip: Option<GeoIpAnalyzer>,
    gpu_procs: Option<GpuProcsAnalyzer>,
    network_stats: Option<NetworkStatsAnalyzer>,
    process_extra: Option<ProcessExtraAnalyzer>,
    psi: Option<PsiAnalyzer>,
    sensor_health: Option<SensorHealthAnalyzer>,
    storage: Option<StorageAnalyzer>,
    swap: Option<SwapAnalyzer>,
    treemap: Option<TreemapAnalyzer>,
}

impl AnalyzerRegistry {
    pub fn new() -> Self {
        Self {
            connections: ConnectionsAnalyzer::new().ok(),
            containers: ContainersAnalyzer::new().ok(),
            // ...auto-detect available analyzers...
        }
    }

    pub fn collect_all(&mut self) {
        if let Some(ref mut a) = self.connections { let _ = a.collect(); }
        if let Some(ref mut a) = self.containers { let _ = a.collect(); }
        // ...
    }
}
```

---

## 5. Falsification Tests - Analyzer Parity (F500-F517)

| ID | Test | Falsification Criterion | Command |
|----|------|------------------------|---------|
| F500 | ConnectionsAnalyzer exists | `grep "ConnectionsAnalyzer" src/ptop/analyzers/` returns empty | `grep -r "ConnectionsAnalyzer"` |
| F501 | Connections parses /proc/net/tcp | No IPv4 connection data | Unit test |
| F502 | Connections parses /proc/net/tcp6 | No IPv6 connection data | Unit test |
| F503 | Connections maps PID to socket | Process name missing | Integration test |
| F504 | ContainersAnalyzer exists | Module missing | `test -f src/ptop/analyzers/containers.rs` |
| F505 | Docker socket detection | Fails to detect running Docker | Integration test |
| F506 | DiskEntropyAnalyzer exists | Module missing | `test -f src/ptop/analyzers/disk_entropy.rs` |
| F507 | Entropy calculation correct | Encrypted disk shows entropy < 0.9 | Unit test |
| F508 | ProcessExtraAnalyzer exists | Module missing | `test -f src/ptop/analyzers/process_extra.rs` |
| F509 | OOM score parsing | `/proc/[pid]/oom_score` not read | Unit test |
| F510 | SensorHealthAnalyzer exists | Module missing | `test -f src/ptop/analyzers/sensor_health.rs` |
| F511 | hwmon parsing | `/sys/class/hwmon/` not enumerated | Unit test |
| F512 | GpuProcsAnalyzer exists | Module missing | `test -f src/ptop/analyzers/gpu_procs.rs` |
| F513 | nvidia-smi parsing | NVIDIA GPU not detected when present | Integration test |
| F514 | TreemapAnalyzer exists | Module missing | `test -f src/ptop/analyzers/treemap.rs` |
| F515 | File scanning works | No file size data in treemap | Integration test |
| F516 | PsiAnalyzer exists | Module missing | `test -f src/ptop/analyzers/psi.rs` |
| F517 | PSI parsing correct | `/proc/pressure/*` not read | Unit test |

---

## 6. Falsification Tests - Panel Features (F600-F650)

| ID | Test | Falsification Criterion |
|----|------|------------------------|
| F600 | CPU panel shows governor | Missing `/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor` |
| F601 | CPU panel shows frequency per-core | All cores show same frequency |
| F602 | CPU panel shows turbo indicator | ⚡ missing when freq > base |
| F603 | Memory panel shows huge pages | Missing hugepages info |
| F604 | Memory panel shows memory pressure | No PSI indicator |
| F605 | Memory panel shows ZRAM ratio | ZRAM compression not shown |
| F606 | Disk panel shows SMART status | No health indicator |
| F607 | Disk panel shows I/O scheduler | Scheduler not displayed |
| F608 | Disk panel shows encryption | LUKS not detected |
| F609 | Network panel shows packet drops | No error/drop counters |
| F610 | Network panel shows GeoIP | No country/city for remote IPs |
| F611 | Process panel shows cgroup | No cgroup column |
| F612 | Process panel shows ionice | No I/O priority |
| F613 | Process panel shows OOM score | No OOM indicator |
| F614 | Process panel shows affinity | No CPU affinity display |
| F615 | GPU panel shows VRAM per-process | No process VRAM breakdown |
| F616 | GPU panel shows temperature | No temp reading |
| F617 | GPU panel shows power draw | No wattage display |
| F618 | Containers panel shows Docker | Docker containers missing |
| F619 | Containers panel shows Podman | Podman containers missing |
| F620 | Containers panel shows CPU/MEM | No per-container stats |
| F621 | Sensors panel shows fans | Fan RPM missing |
| F622 | Sensors panel shows voltages | Voltage rails missing |
| F623 | Sensors panel shows thresholds | No critical/warning indicators |
| F624 | Treemap shows real files | Hardcoded data instead of scan |
| F625 | Treemap shows sizes | File sizes not displayed |
| F626 | Treemap updates on change | No inotify integration |
| F627 | Files panel shows hot files | No recently accessed tracking |
| F628 | Files panel shows duplicates | No duplicate detection |
| F629 | Connections panel shows all states | Only ESTABLISHED shown |
| F630 | Connections panel shows process | No PID/name mapping |
| F631 | Connections panel shows GeoIP | No geolocation data |

---

## 7. Falsification Tests - Pixel Comparison (F700-F730)

| ID | Test | Falsification Criterion | Threshold |
|----|------|------------------------|-----------|
| F700 | Full screen CLD | Character difference > 1% | CLD < 0.01 |
| F701 | Full screen ΔE00 | Average color diff > 2.0 | ΔE00 < 2.0 |
| F702 | Full screen SSIM | Structural similarity < 95% | SSIM > 0.95 |
| F703 | CPU panel CLD | Character difference > 0.5% | CLD < 0.005 |
| F704 | CPU panel ΔE00 | Color diff > 1.5 | ΔE00 < 1.5 |
| F705 | Memory panel CLD | Character difference > 0.5% | CLD < 0.005 |
| F706 | Memory panel ΔE00 | Color diff > 1.5 | ΔE00 < 1.5 |
| F707 | Disk panel CLD | Character difference > 0.5% | CLD < 0.005 |
| F708 | Network panel CLD | Character difference > 0.5% | CLD < 0.005 |
| F709 | Process panel CLD | Character difference > 1% | CLD < 0.01 |
| F710 | Connections panel CLD | Character difference > 1% | CLD < 0.01 |
| F711 | Treemap panel CLD | Character difference > 1% | CLD < 0.01 |
| F712 | Header exact match | Any character differs | Exact match |
| F713 | Footer exact match | Any character differs | Exact match |
| F714 | Border chars match | Wrong box drawing chars | Exact match |
| F715 | Braille chars match | Wrong braille patterns | Exact match |
| F716 | Color gradient accuracy | ΔE > 3 in any gradient region | ΔE < 3.0 |
| F717 | Column alignment | Columns misaligned by > 1 char | ±1 char |
| F718 | Row heights match | Panel heights differ | Exact match |
| F719 | Padding consistency | Different padding | Exact match |
| F720 | Focus indicator match | Different focus style | Exact match |

---

## 8. Falsification Tests - Data Accuracy (F800-F820)

| ID | Test | Falsification Criterion | Reference |
|----|------|------------------------|-----------|
| F800 | CPU % accuracy | Differs > 5% from `mpstat` | `mpstat 1 1` |
| F801 | Memory accuracy | Differs > 1% from `free` | `free -b` |
| F802 | Swap accuracy | Differs > 1% from `free` | `free -b` |
| F803 | Disk usage accuracy | Differs > 1% from `df` | `df -B1` |
| F804 | Network RX accuracy | Differs > 5% from `/proc/net/dev` | `cat /proc/net/dev` |
| F805 | Network TX accuracy | Differs > 5% from `/proc/net/dev` | `cat /proc/net/dev` |
| F806 | Process count | Differs from `ps aux | wc -l` | `ps aux` |
| F807 | Uptime accuracy | Differs > 1s from `/proc/uptime` | `cat /proc/uptime` |
| F808 | Load average accuracy | Differs > 0.1 from `uptime` | `uptime` |
| F809 | Core count | Differs from `nproc` | `nproc` |
| F810 | Temperature accuracy | Differs > 2°C from `sensors` | `sensors` |
| F811 | Connection count | Differs from `ss -tan | wc -l` | `ss -tan` |
| F812 | Container count | Differs from `docker ps | wc -l` | `docker ps` |

---

## 9. Falsification Tests - Anti-Regression (F900-F905)

| ID | Test | Falsification Criterion |
|----|------|------------------------|
| F900 | Pure Data | `grep -r "simulate_"` matches in `src/ptop/` (Simulated data forbidden) |
| F901 | CIELAB Precision | `interpolate_lab` midpoint differs from reference by ΔE > 0.1 |
| F902 | Source Attribution | `grep -r "Reference: .*btop"` returns 0 matches in widgets |
| F903 | Symbol Integrity | `symbols.rs` missing any `Braille` or `Block` definitions |
| F904 | Dependency Gate | `ptop` feature is enabled by default (must be optional) |
| F905 | No Magic Numbers | Layout constants > 0 without named const definition |

---

## 10. Implementation Roadmap

### Phase 1: Honest Foundation (Week 1)

- [ ] Create `src/ptop/analyzers/` directory structure
- [ ] Implement `Analyzer` trait
- [ ] Implement `ConnectionsAnalyzer` with full /proc parsing
- [ ] Implement `ProcessExtraAnalyzer` with cgroup/OOM
- [ ] Update UI to use analyzer data

### Phase 2: Hardware Intelligence (Week 2)

- [ ] Implement `SensorHealthAnalyzer` with hwmon parsing
- [ ] Implement `GpuProcsAnalyzer` with nvidia-smi/AMDGPU
- [ ] Implement `DiskEntropyAnalyzer` with encryption detection
- [ ] Implement `StorageAnalyzer` with SMART data

### Phase 3: Container & Network (Week 3)

- [ ] Implement `ContainersAnalyzer` with Docker/Podman
- [ ] Implement `NetworkStatsAnalyzer` with packet stats
- [ ] Implement `GeoIpAnalyzer` with MaxMind
- [ ] Implement `PsiAnalyzer` with pressure stats

### Phase 4: File Intelligence (Week 4)

- [ ] Implement `FileAnalyzer` with walkdir scanning
- [ ] Implement `TreemapAnalyzer` with squarify algorithm
- [ ] Add inotify for hot file tracking
- [ ] Add duplicate detection

### Phase 5: Pixel Perfection (Week 5)

- [ ] Run full comparison suite
- [ ] Fix all CLD/ΔE00/SSIM failures
- [ ] Achieve 100% falsification test pass rate
- [ ] Generate final comparison report

---

## 10. Acceptance Gate

```bash
#!/bin/bash
# scripts/acceptance_gate.sh

echo "═══════════════════════════════════════════════════════════════"
echo "           PTOP PIXEL-PERFECT ACCEPTANCE GATE                  "
echo "═══════════════════════════════════════════════════════════════"

# Run all falsification tests
./scripts/falsify_ptop.sh --all

RESULT=$?

if [ $RESULT -eq 0 ]; then
    echo ""
    echo "╔═══════════════════════════════════════════════════════════════╗"
    echo "║                                                               ║"
    echo "║   ✓ ALL TESTS PASSED                                         ║"
    echo "║                                                               ║"
    echo "║   ptop is PIXEL-PERFECT identical to ttop                    ║"
    echo "║                                                               ║"
    echo "║   presentar-terminal can build ANYTHING                      ║"
    echo "║                                                               ║"
    echo "╚═══════════════════════════════════════════════════════════════╝"
else
    echo ""
    echo "╔═══════════════════════════════════════════════════════════════╗"
    echo "║                                                               ║"
    echo "║   ✗ TESTS FAILED                                             ║"
    echo "║                                                               ║"
    echo "║   ptop is NOT pixel-perfect                                  ║"
    echo "║                                                               ║"
    echo "║   DO NOT CLAIM COMPLETION                                    ║"
    echo "║                                                               ║"
    echo "╚═══════════════════════════════════════════════════════════════╝"
    exit 1
fi
```

---

## 11. Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0-3.0.0 | 2026-01-09/10 | Claude Code | See previous versions |
| **4.0.0** | 2026-01-10 | Claude Code | **BREAKING**: Honest gap assessment. Previous "85% complete" claim was FALSE. Actual: 13% code parity, 40% visual parity. Added: (1) Full ttop analyzer inventory (17 modules, 12,847 lines missing); (2) TUI pixel comparison tooling spec with CIEDE2000, SSIM, CLD metrics; (3) Film studio grade color comparison pipeline; (4) 120 new falsification tests (F500-F820); (5) Analyzer implementation specifications; (6) Acceptance gate script. Total falsification tests now: 301. |
| **4.1.0** | 2026-01-10 | Claude Code | Re-integrated "Anti-Regression" checks (F900-F905) to ban simulated data and mandate CIELAB precision. Updated acceptance gate. |
| **4.2.0** | 2026-01-10 | Claude Code | Tightened performance criteria (F260-F262) to <= 1.0x ttop parity and input latency (F264) to <16ms. |

---

## 12. Conclusion

This specification now honestly documents the gap between ttop and ptop. The claim "pixel-perfect" requires passing ALL 301 falsification tests. Until then, ptop is a **partial implementation**, not a complete clone.

**Current Status**: FAILING (40% visual parity, 13% code parity)

**Required for PASS**: Implement 17 analyzers, achieve CLD < 1%, ΔE00 < 2.0, SSIM > 95%
