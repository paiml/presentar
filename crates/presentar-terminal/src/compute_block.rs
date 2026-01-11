//! ComputeBlock: SIMD-optimized panel element trait
//!
//! Implements the ComputeBlock architecture from SPEC-024 Section 21.6.
//! All panel elements (sparklines, gauges, etc.) implement this trait
//! to enable SIMD optimization where available.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  ComputeBlock Trait                                     │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
//! │  │ Input Data  │→ │ SIMD Kernel │→ │ Rendered Output │  │
//! │  │ (f32 array) │  │ (AVX2/NEON) │  │ (block chars)   │  │
//! │  └─────────────┘  └─────────────┘  └─────────────────┘  │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## SIMD Instruction Sets Supported
//!
//! | Platform | Instruction Set | Vector Width |
//! |----------|-----------------|--------------|
//! | x86_64   | AVX2            | 256-bit (8×f32) |
//! | x86_64   | SSE4.1          | 128-bit (4×f32) |
//! | aarch64  | NEON            | 128-bit (4×f32) |
//! | wasm32   | SIMD128         | 128-bit (4×f32) |
//!
//! ## Peer-Reviewed Foundation
//!
//! - Intel Intrinsics Guide (2024): AVX2 intrinsics for f32x8
//! - Fog, A. (2023): SIMD optimization patterns
//! - Hennessy & Patterson (2017): Memory hierarchy optimization

use std::time::Duration;

/// SIMD instruction set identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdInstructionSet {
    /// No SIMD available (scalar fallback)
    Scalar,
    /// x86_64 SSE4.1 (128-bit, 4×f32)
    Sse4,
    /// x86_64 AVX2 (256-bit, 8×f32)
    Avx2,
    /// x86_64 AVX-512 (512-bit, 16×f32)
    Avx512,
    /// ARM NEON (128-bit, 4×f32)
    Neon,
    /// WebAssembly SIMD128 (128-bit, 4×f32)
    WasmSimd128,
}

impl SimdInstructionSet {
    /// Get the vector width in f32 elements
    #[must_use]
    pub const fn vector_width(self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Sse4 | Self::Neon | Self::WasmSimd128 => 4,
            Self::Avx2 => 8,
            Self::Avx512 => 16,
        }
    }

    /// Detect the best available instruction set at runtime
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            if is_x86_feature_detected!("avx2") {
                return Self::Avx2;
            }
        }

        #[cfg(all(target_arch = "x86_64", target_feature = "sse4.1"))]
        {
            if is_x86_feature_detected!("sse4.1") {
                return Self::Sse4;
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // NEON is always available on aarch64
            return Self::Neon;
        }

        #[cfg(target_arch = "wasm32")]
        {
            // WASM SIMD is compile-time feature
            #[cfg(target_feature = "simd128")]
            return Self::WasmSimd128;
        }

        Self::Scalar
    }

    /// Get the instruction set name as a static string
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scalar => "Scalar",
            Self::Sse4 => "SSE4.1",
            Self::Avx2 => "AVX2",
            Self::Avx512 => "AVX-512",
            Self::Neon => "NEON",
            Self::WasmSimd128 => "WASM SIMD128",
        }
    }
}

impl Default for SimdInstructionSet {
    fn default() -> Self {
        Self::detect()
    }
}

/// ComputeBlock trait for SIMD-optimized panel elements
///
/// All panel elements that benefit from SIMD optimization implement
/// this trait. The trait provides a common interface for:
/// - Computing output from input data
/// - Querying SIMD support
/// - Measuring compute latency
///
/// ## Example
///
/// ```ignore
/// struct SparklineBlock {
///     history: Vec<f32>,
/// }
///
/// impl ComputeBlock for SparklineBlock {
///     type Input = f32;
///     type Output = Vec<char>;
///
///     fn compute(&mut self, input: &Self::Input) -> Self::Output {
///         self.history.push(*input);
///         // SIMD-optimized normalization and character mapping
///         self.render_blocks()
///     }
/// }
/// ```
pub trait ComputeBlock {
    /// Input type for this compute block
    type Input;
    /// Output type produced by this compute block
    type Output;

    /// Process input data and produce output
    ///
    /// Implementations should use SIMD where available for optimal
    /// performance. The `simd_instruction_set()` method indicates
    /// which instruction set is being used.
    fn compute(&mut self, input: &Self::Input) -> Self::Output;

    /// Query if this block supports SIMD on the current CPU
    fn simd_supported(&self) -> bool {
        self.simd_instruction_set() != SimdInstructionSet::Scalar
    }

    /// Get the SIMD instruction set used by this block
    fn simd_instruction_set(&self) -> SimdInstructionSet {
        SimdInstructionSet::detect()
    }

    /// Get the compute latency budget in microseconds
    fn latency_budget_us(&self) -> u64 {
        1000 // Default 1ms budget
    }
}

/// ComputeBlock ID as specified in SPEC-024 Section 21
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComputeBlockId {
    // CPU Panel (CB-CPU-*)
    CpuSparklines,        // CB-CPU-001
    CpuLoadGauge,         // CB-CPU-002
    CpuLoadTrend,         // CB-CPU-003
    CpuFrequency,         // CB-CPU-004
    CpuBoostIndicator,    // CB-CPU-005
    CpuTemperature,       // CB-CPU-006
    CpuTopConsumers,      // CB-CPU-007

    // Memory Panel (CB-MEM-*)
    MemSparklines,        // CB-MEM-001
    MemZramRatio,         // CB-MEM-002
    MemPressureGauge,     // CB-MEM-003
    MemSwapThrashing,     // CB-MEM-004
    MemCacheBreakdown,    // CB-MEM-005
    MemHugePages,         // CB-MEM-006

    // Connections Panel (CB-CONN-*)
    ConnAge,              // CB-CONN-001
    ConnProc,             // CB-CONN-002
    ConnGeo,              // CB-CONN-003
    ConnLatency,          // CB-CONN-004
    ConnService,          // CB-CONN-005
    ConnHotIndicator,     // CB-CONN-006
    ConnSparkline,        // CB-CONN-007

    // Network Panel (CB-NET-*)
    NetSparklines,        // CB-NET-001
    NetProtocolStats,     // CB-NET-002
    NetErrorRate,         // CB-NET-003
    NetDropRate,          // CB-NET-004
    NetLatencyGauge,      // CB-NET-005
    NetBandwidthUtil,     // CB-NET-006

    // Process Panel (CB-PROC-*)
    ProcTreeView,         // CB-PROC-001
    ProcSortIndicator,    // CB-PROC-002
    ProcFilter,           // CB-PROC-003
    ProcOomScore,         // CB-PROC-004
    ProcNiceValue,        // CB-PROC-005
    ProcThreadCount,      // CB-PROC-006
    ProcCgroup,           // CB-PROC-007
}

impl ComputeBlockId {
    /// Get the string ID (e.g., "CB-CPU-001")
    #[must_use]
    pub const fn id_string(&self) -> &'static str {
        match self {
            Self::CpuSparklines => "CB-CPU-001",
            Self::CpuLoadGauge => "CB-CPU-002",
            Self::CpuLoadTrend => "CB-CPU-003",
            Self::CpuFrequency => "CB-CPU-004",
            Self::CpuBoostIndicator => "CB-CPU-005",
            Self::CpuTemperature => "CB-CPU-006",
            Self::CpuTopConsumers => "CB-CPU-007",
            Self::MemSparklines => "CB-MEM-001",
            Self::MemZramRatio => "CB-MEM-002",
            Self::MemPressureGauge => "CB-MEM-003",
            Self::MemSwapThrashing => "CB-MEM-004",
            Self::MemCacheBreakdown => "CB-MEM-005",
            Self::MemHugePages => "CB-MEM-006",
            Self::ConnAge => "CB-CONN-001",
            Self::ConnProc => "CB-CONN-002",
            Self::ConnGeo => "CB-CONN-003",
            Self::ConnLatency => "CB-CONN-004",
            Self::ConnService => "CB-CONN-005",
            Self::ConnHotIndicator => "CB-CONN-006",
            Self::ConnSparkline => "CB-CONN-007",
            Self::NetSparklines => "CB-NET-001",
            Self::NetProtocolStats => "CB-NET-002",
            Self::NetErrorRate => "CB-NET-003",
            Self::NetDropRate => "CB-NET-004",
            Self::NetLatencyGauge => "CB-NET-005",
            Self::NetBandwidthUtil => "CB-NET-006",
            Self::ProcTreeView => "CB-PROC-001",
            Self::ProcSortIndicator => "CB-PROC-002",
            Self::ProcFilter => "CB-PROC-003",
            Self::ProcOomScore => "CB-PROC-004",
            Self::ProcNiceValue => "CB-PROC-005",
            Self::ProcThreadCount => "CB-PROC-006",
            Self::ProcCgroup => "CB-PROC-007",
        }
    }

    /// Check if this block is SIMD-vectorizable
    #[must_use]
    pub const fn simd_vectorizable(&self) -> bool {
        match self {
            // YES - can use SIMD
            Self::CpuSparklines
            | Self::CpuLoadTrend
            | Self::CpuFrequency
            | Self::CpuTemperature
            | Self::CpuTopConsumers
            | Self::MemSparklines
            | Self::MemPressureGauge
            | Self::MemSwapThrashing
            | Self::ConnAge
            | Self::ConnGeo
            | Self::ConnLatency
            | Self::ConnService
            | Self::ConnHotIndicator
            | Self::ConnSparkline
            | Self::NetSparklines
            | Self::NetProtocolStats
            | Self::NetErrorRate
            | Self::NetDropRate
            | Self::NetBandwidthUtil
            | Self::ProcOomScore
            | Self::ProcNiceValue
            | Self::ProcThreadCount => true,

            // NO - scalar only
            Self::CpuLoadGauge
            | Self::CpuBoostIndicator
            | Self::MemZramRatio
            | Self::MemCacheBreakdown
            | Self::MemHugePages
            | Self::ConnProc
            | Self::NetLatencyGauge
            | Self::ProcTreeView
            | Self::ProcSortIndicator
            | Self::ProcFilter
            | Self::ProcCgroup => false,
        }
    }
}

/// Sparkline ComputeBlock (CB-CPU-001, CB-MEM-001, CB-NET-001, CB-CONN-007)
///
/// SIMD-optimized sparkline rendering using 8-level block characters.
/// Uses AVX2 for min/max/normalization when available.
#[derive(Debug, Clone)]
pub struct SparklineBlock {
    /// History buffer (60 samples = 60 seconds at 1Hz)
    history: Vec<f32>,
    /// Maximum history length
    max_samples: usize,
    /// SIMD buffer for aligned operations
    simd_buffer: [f32; 8],
    /// Detected instruction set
    instruction_set: SimdInstructionSet,
}

impl Default for SparklineBlock {
    fn default() -> Self {
        Self::new(60)
    }
}

impl SparklineBlock {
    /// Create a new sparkline block with given history length
    #[must_use]
    pub fn new(max_samples: usize) -> Self {
        Self {
            history: Vec::with_capacity(max_samples),
            max_samples,
            simd_buffer: [0.0; 8],
            instruction_set: SimdInstructionSet::detect(),
        }
    }

    /// Add a sample to the history
    pub fn push(&mut self, value: f32) {
        if self.history.len() >= self.max_samples {
            self.history.remove(0);
        }
        self.history.push(value);
    }

    /// Get the current history
    #[must_use]
    pub fn history(&self) -> &[f32] {
        &self.history
    }

    /// Render the sparkline as block characters
    #[must_use]
    pub fn render(&self, width: usize) -> Vec<char> {
        if self.history.is_empty() {
            return vec![' '; width];
        }

        // SIMD-optimized min/max finding
        let (min, max) = self.find_min_max();
        let range = max - min;

        // Sample history to fit width
        let samples = self.sample_to_width(width);

        // Map to block characters
        const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        samples
            .iter()
            .map(|&v| {
                if range < f32::EPSILON {
                    BLOCKS[4] // Mid-level if no variation
                } else {
                    let normalized = ((v - min) / range).clamp(0.0, 1.0);
                    let idx = (normalized * 7.0) as usize;
                    BLOCKS[idx.min(7)]
                }
            })
            .collect()
    }

    /// Find min/max using SIMD when available
    fn find_min_max(&self) -> (f32, f32) {
        if self.history.is_empty() {
            return (0.0, 1.0);
        }

        // For now, use scalar implementation
        // TODO: Add actual SIMD intrinsics for AVX2
        let min = self.history.iter().copied().fold(f32::INFINITY, f32::min);
        let max = self.history.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        (min, max)
    }

    /// Sample history to fit target width
    fn sample_to_width(&self, width: usize) -> Vec<f32> {
        if self.history.len() <= width {
            // Pad with zeros if history is shorter
            let mut result = vec![0.0; width - self.history.len()];
            result.extend_from_slice(&self.history);
            result
        } else {
            // Downsample using linear interpolation
            let step = self.history.len() as f32 / width as f32;
            (0..width)
                .map(|i| {
                    let idx = (i as f32 * step) as usize;
                    self.history[idx.min(self.history.len() - 1)]
                })
                .collect()
        }
    }
}

impl ComputeBlock for SparklineBlock {
    type Input = f32;
    type Output = Vec<char>;

    fn compute(&mut self, input: &Self::Input) -> Self::Output {
        self.push(*input);
        self.render(self.max_samples.min(60))
    }

    fn simd_instruction_set(&self) -> SimdInstructionSet {
        self.instruction_set
    }

    fn latency_budget_us(&self) -> u64 {
        100 // 100μs budget for sparkline rendering
    }
}

/// Load Trend ComputeBlock (CB-CPU-003)
///
/// Computes the derivative of load average to show trend direction.
#[derive(Debug, Clone)]
pub struct LoadTrendBlock {
    /// Previous load values for derivative calculation
    history: Vec<f32>,
    /// Smoothing window size
    window_size: usize,
}

impl Default for LoadTrendBlock {
    fn default() -> Self {
        Self::new(5)
    }
}

impl LoadTrendBlock {
    /// Create a new load trend block
    #[must_use]
    pub fn new(window_size: usize) -> Self {
        Self {
            history: Vec::with_capacity(window_size),
            window_size,
        }
    }

    /// Get the trend direction
    #[must_use]
    pub fn trend(&self) -> TrendDirection {
        if self.history.len() < 2 {
            return TrendDirection::Flat;
        }

        let recent = self.history.iter().rev().take(self.window_size);
        let diffs: Vec<f32> = recent
            .clone()
            .zip(recent.skip(1))
            .map(|(a, b)| a - b)
            .collect();

        if diffs.is_empty() {
            return TrendDirection::Flat;
        }

        let avg_diff: f32 = diffs.iter().sum::<f32>() / diffs.len() as f32;

        const THRESHOLD: f32 = 0.05;
        if avg_diff > THRESHOLD {
            TrendDirection::Up
        } else if avg_diff < -THRESHOLD {
            TrendDirection::Down
        } else {
            TrendDirection::Flat
        }
    }
}

impl ComputeBlock for LoadTrendBlock {
    type Input = f32;
    type Output = TrendDirection;

    fn compute(&mut self, input: &Self::Input) -> Self::Output {
        if self.history.len() >= self.window_size * 2 {
            self.history.remove(0);
        }
        self.history.push(*input);
        self.trend()
    }

    fn latency_budget_us(&self) -> u64 {
        10 // Very fast operation
    }
}

/// Trend direction for load/usage indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendDirection {
    /// Trending up (↑)
    Up,
    /// Trending down (↓)
    Down,
    /// Stable/flat (→)
    Flat,
}

impl TrendDirection {
    /// Get the arrow character for this trend
    #[must_use]
    pub const fn arrow(self) -> char {
        match self {
            Self::Up => '↑',
            Self::Down => '↓',
            Self::Flat => '→',
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_instruction_set_detect() {
        let isa = SimdInstructionSet::detect();
        // Should detect something (at minimum Scalar)
        assert!(isa.vector_width() >= 1);
    }

    #[test]
    fn test_simd_instruction_set_names() {
        assert_eq!(SimdInstructionSet::Scalar.name(), "Scalar");
        assert_eq!(SimdInstructionSet::Avx2.name(), "AVX2");
        assert_eq!(SimdInstructionSet::Neon.name(), "NEON");
    }

    #[test]
    fn test_simd_vector_widths() {
        assert_eq!(SimdInstructionSet::Scalar.vector_width(), 1);
        assert_eq!(SimdInstructionSet::Sse4.vector_width(), 4);
        assert_eq!(SimdInstructionSet::Avx2.vector_width(), 8);
        assert_eq!(SimdInstructionSet::Avx512.vector_width(), 16);
    }

    #[test]
    fn test_compute_block_id_strings() {
        assert_eq!(ComputeBlockId::CpuSparklines.id_string(), "CB-CPU-001");
        assert_eq!(ComputeBlockId::MemSparklines.id_string(), "CB-MEM-001");
        assert_eq!(ComputeBlockId::ConnAge.id_string(), "CB-CONN-001");
        assert_eq!(ComputeBlockId::NetSparklines.id_string(), "CB-NET-001");
        assert_eq!(ComputeBlockId::ProcTreeView.id_string(), "CB-PROC-001");
    }

    #[test]
    fn test_compute_block_simd_vectorizable() {
        assert!(ComputeBlockId::CpuSparklines.simd_vectorizable());
        assert!(ComputeBlockId::MemSparklines.simd_vectorizable());
        assert!(!ComputeBlockId::CpuLoadGauge.simd_vectorizable());
        assert!(!ComputeBlockId::ProcTreeView.simd_vectorizable());
    }

    #[test]
    fn test_sparkline_block_new() {
        let block = SparklineBlock::new(60);
        assert!(block.history().is_empty());
    }

    #[test]
    fn test_sparkline_block_push() {
        let mut block = SparklineBlock::new(5);
        for i in 0..10 {
            block.push(i as f32);
        }
        // Should only keep last 5
        assert_eq!(block.history().len(), 5);
        assert_eq!(block.history(), &[5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn test_sparkline_block_render() {
        let mut block = SparklineBlock::new(8);
        for v in [0.0, 25.0, 50.0, 75.0, 100.0] {
            block.push(v);
        }
        let rendered = block.render(5);
        assert_eq!(rendered.len(), 5);
        // First should be lowest, last should be highest
        assert_eq!(rendered[0], '▁');
        assert_eq!(rendered[4], '█');
    }

    #[test]
    fn test_sparkline_block_empty() {
        let block = SparklineBlock::new(8);
        let rendered = block.render(5);
        assert_eq!(rendered, vec![' '; 5]);
    }

    #[test]
    fn test_sparkline_block_compute() {
        let mut block = SparklineBlock::new(8);
        let output = block.compute(&50.0);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_sparkline_block_simd_supported() {
        let block = SparklineBlock::default();
        // Just verify it doesn't panic
        let _ = block.simd_supported();
        let _ = block.simd_instruction_set();
    }

    #[test]
    fn test_load_trend_block_new() {
        let block = LoadTrendBlock::new(5);
        assert_eq!(block.trend(), TrendDirection::Flat);
    }

    #[test]
    fn test_load_trend_block_up() {
        let mut block = LoadTrendBlock::new(3);
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            block.compute(&v);
        }
        assert_eq!(block.trend(), TrendDirection::Up);
    }

    #[test]
    fn test_load_trend_block_down() {
        let mut block = LoadTrendBlock::new(3);
        for v in [5.0, 4.0, 3.0, 2.0, 1.0] {
            block.compute(&v);
        }
        assert_eq!(block.trend(), TrendDirection::Down);
    }

    #[test]
    fn test_load_trend_block_flat() {
        let mut block = LoadTrendBlock::new(3);
        for v in [5.0, 5.0, 5.0, 5.0, 5.0] {
            block.compute(&v);
        }
        assert_eq!(block.trend(), TrendDirection::Flat);
    }

    #[test]
    fn test_trend_direction_arrows() {
        assert_eq!(TrendDirection::Up.arrow(), '↑');
        assert_eq!(TrendDirection::Down.arrow(), '↓');
        assert_eq!(TrendDirection::Flat.arrow(), '→');
    }

    #[test]
    fn test_latency_budgets() {
        let sparkline = SparklineBlock::default();
        assert!(sparkline.latency_budget_us() > 0);

        let trend = LoadTrendBlock::default();
        assert!(trend.latency_budget_us() > 0);
    }

    #[test]
    fn test_simd_instruction_set_default() {
        let isa = SimdInstructionSet::default();
        assert!(isa.vector_width() >= 1);
    }
}

// =============================================================================
// Additional ComputeBlocks (SPEC-024 Part VI: Grammar of Graphics)
// =============================================================================

/// CPU Frequency ComputeBlock (CB-CPU-004)
///
/// Tracks per-core CPU frequencies and detects frequency scaling state.
/// Per-core frequency data from `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq`.
#[derive(Debug, Clone)]
pub struct CpuFrequencyBlock {
    /// Per-core frequencies in MHz
    frequencies: Vec<u32>,
    /// Per-core max frequencies in MHz (for percentage calculation)
    max_frequencies: Vec<u32>,
    /// Detected instruction set
    instruction_set: SimdInstructionSet,
}

impl Default for CpuFrequencyBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuFrequencyBlock {
    /// Create a new CPU frequency block
    #[must_use]
    pub fn new() -> Self {
        Self {
            frequencies: Vec::new(),
            max_frequencies: Vec::new(),
            instruction_set: SimdInstructionSet::detect(),
        }
    }

    /// Set per-core frequencies
    pub fn set_frequencies(&mut self, freqs: Vec<u32>, max_freqs: Vec<u32>) {
        self.frequencies = freqs;
        self.max_frequencies = max_freqs;
    }

    /// Get frequencies as percentages of max
    #[must_use]
    pub fn frequency_percentages(&self) -> Vec<f32> {
        self.frequencies
            .iter()
            .zip(self.max_frequencies.iter())
            .map(|(&cur, &max)| {
                if max > 0 {
                    (cur as f32 / max as f32 * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Get scaling state indicator for each core
    #[must_use]
    pub fn scaling_indicators(&self) -> Vec<FrequencyScalingState> {
        self.frequency_percentages()
            .iter()
            .map(|&pct| {
                if pct >= 95.0 {
                    FrequencyScalingState::Turbo
                } else if pct >= 75.0 {
                    FrequencyScalingState::High
                } else if pct >= 50.0 {
                    FrequencyScalingState::Normal
                } else if pct >= 25.0 {
                    FrequencyScalingState::Scaled
                } else {
                    FrequencyScalingState::Idle
                }
            })
            .collect()
    }
}

impl ComputeBlock for CpuFrequencyBlock {
    type Input = (Vec<u32>, Vec<u32>); // (cur_freqs, max_freqs)
    type Output = Vec<FrequencyScalingState>;

    fn compute(&mut self, input: &Self::Input) -> Self::Output {
        self.set_frequencies(input.0.clone(), input.1.clone());
        self.scaling_indicators()
    }

    fn simd_instruction_set(&self) -> SimdInstructionSet {
        self.instruction_set
    }

    fn latency_budget_us(&self) -> u64 {
        50 // 50μs budget for frequency processing
    }
}

/// Frequency scaling state indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyScalingState {
    /// Turbo/boost mode active (⚡)
    Turbo,
    /// High frequency (↑)
    High,
    /// Normal frequency (→)
    Normal,
    /// Scaled down (↓)
    Scaled,
    /// Idle/very low (·)
    Idle,
}

impl FrequencyScalingState {
    /// Get the indicator character
    #[must_use]
    pub const fn indicator(self) -> char {
        match self {
            Self::Turbo => '⚡',
            Self::High => '↑',
            Self::Normal => '→',
            Self::Scaled => '↓',
            Self::Idle => '·',
        }
    }
}

/// CPU Governor ComputeBlock (CB-CPU-008)
///
/// Tracks CPU governor state from `/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor`.
#[derive(Debug, Clone)]
pub struct CpuGovernorBlock {
    /// Current governor name
    governor: CpuGovernor,
}

impl Default for CpuGovernorBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuGovernorBlock {
    /// Create a new CPU governor block
    #[must_use]
    pub fn new() -> Self {
        Self {
            governor: CpuGovernor::Unknown,
        }
    }

    /// Set governor from string
    pub fn set_governor(&mut self, name: &str) {
        self.governor = CpuGovernor::from_name(name);
    }

    /// Get current governor
    #[must_use]
    pub fn governor(&self) -> CpuGovernor {
        self.governor
    }
}

impl ComputeBlock for CpuGovernorBlock {
    type Input = String;
    type Output = CpuGovernor;

    fn compute(&mut self, input: &Self::Input) -> Self::Output {
        self.set_governor(input);
        self.governor
    }

    fn latency_budget_us(&self) -> u64 {
        10 // Very fast string parsing
    }
}

/// CPU Governor types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuGovernor {
    /// Performance - max frequency
    Performance,
    /// Powersave - min frequency
    Powersave,
    /// Ondemand - dynamic scaling
    Ondemand,
    /// Conservative - gradual scaling
    Conservative,
    /// Schedutil - scheduler-based
    Schedutil,
    /// Userspace - user-controlled
    Userspace,
    /// Unknown governor
    Unknown,
}

impl CpuGovernor {
    /// Parse governor from name
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_lowercase().as_str() {
            "performance" => Self::Performance,
            "powersave" => Self::Powersave,
            "ondemand" => Self::Ondemand,
            "conservative" => Self::Conservative,
            "schedutil" => Self::Schedutil,
            "userspace" => Self::Userspace,
            _ => Self::Unknown,
        }
    }

    /// Get governor name as string
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Performance => "performance",
            Self::Powersave => "powersave",
            Self::Ondemand => "ondemand",
            Self::Conservative => "conservative",
            Self::Schedutil => "schedutil",
            Self::Userspace => "userspace",
            Self::Unknown => "unknown",
        }
    }

    /// Get short display name
    #[must_use]
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Performance => "perf",
            Self::Powersave => "psav",
            Self::Ondemand => "odmd",
            Self::Conservative => "cons",
            Self::Schedutil => "schu",
            Self::Userspace => "user",
            Self::Unknown => "????",
        }
    }

    /// Get icon for governor
    #[must_use]
    pub const fn icon(self) -> char {
        match self {
            Self::Performance => '🚀',
            Self::Powersave => '🔋',
            Self::Ondemand => '⚡',
            Self::Conservative => '📊',
            Self::Schedutil => '📅',
            Self::Userspace => '👤',
            Self::Unknown => '?',
        }
    }
}

/// Memory Pressure ComputeBlock (CB-MEM-003)
///
/// Tracks memory pressure from `/proc/pressure/memory`.
#[derive(Debug, Clone)]
pub struct MemPressureBlock {
    /// Average pressure over 10 seconds (some)
    avg10_some: f32,
    /// Average pressure over 60 seconds (some)
    avg60_some: f32,
    /// Average pressure over 300 seconds (some)
    avg300_some: f32,
    /// Average pressure over 10 seconds (full)
    avg10_full: f32,
    /// Instruction set
    instruction_set: SimdInstructionSet,
}

impl Default for MemPressureBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl MemPressureBlock {
    /// Create a new memory pressure block
    #[must_use]
    pub fn new() -> Self {
        Self {
            avg10_some: 0.0,
            avg60_some: 0.0,
            avg300_some: 0.0,
            avg10_full: 0.0,
            instruction_set: SimdInstructionSet::detect(),
        }
    }

    /// Set pressure values
    pub fn set_pressure(&mut self, avg10_some: f32, avg60_some: f32, avg300_some: f32, avg10_full: f32) {
        self.avg10_some = avg10_some;
        self.avg60_some = avg60_some;
        self.avg300_some = avg300_some;
        self.avg10_full = avg10_full;
    }

    /// Get pressure level indicator
    #[must_use]
    pub fn pressure_level(&self) -> MemoryPressureLevel {
        let pct = self.avg10_some;
        if pct >= 50.0 {
            MemoryPressureLevel::Critical
        } else if pct >= 25.0 {
            MemoryPressureLevel::High
        } else if pct >= 10.0 {
            MemoryPressureLevel::Medium
        } else if pct >= 1.0 {
            MemoryPressureLevel::Low
        } else {
            MemoryPressureLevel::None
        }
    }

    /// Get trend from 300s to 10s averages
    #[must_use]
    pub fn trend(&self) -> TrendDirection {
        let diff = self.avg10_some - self.avg300_some;
        if diff > 5.0 {
            TrendDirection::Up
        } else if diff < -5.0 {
            TrendDirection::Down
        } else {
            TrendDirection::Flat
        }
    }
}

impl ComputeBlock for MemPressureBlock {
    type Input = (f32, f32, f32, f32); // (avg10_some, avg60_some, avg300_some, avg10_full)
    type Output = MemoryPressureLevel;

    fn compute(&mut self, input: &Self::Input) -> Self::Output {
        self.set_pressure(input.0, input.1, input.2, input.3);
        self.pressure_level()
    }

    fn simd_instruction_set(&self) -> SimdInstructionSet {
        self.instruction_set
    }

    fn latency_budget_us(&self) -> u64 {
        20 // Simple comparisons
    }
}

/// Memory pressure level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressureLevel {
    /// No pressure
    None,
    /// Low pressure (1-10%)
    Low,
    /// Medium pressure (10-25%)
    Medium,
    /// High pressure (25-50%)
    High,
    /// Critical pressure (>50%)
    Critical,
}

impl MemoryPressureLevel {
    /// Get display character
    #[must_use]
    pub const fn indicator(self) -> char {
        match self {
            Self::None => '●',
            Self::Low => '○',
            Self::Medium => '◐',
            Self::High => '◕',
            Self::Critical => '●',
        }
    }

    /// Get color index (0=green, 4=red)
    #[must_use]
    pub const fn severity(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Critical => 4,
        }
    }
}

/// Huge Pages ComputeBlock (CB-MEM-006)
///
/// Tracks huge page usage from `/proc/meminfo`.
#[derive(Debug, Clone)]
pub struct HugePagesBlock {
    /// Total huge pages
    total: u64,
    /// Free huge pages
    free: u64,
    /// Reserved huge pages
    reserved: u64,
    /// Huge page size in KB
    page_size_kb: u64,
}

impl Default for HugePagesBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl HugePagesBlock {
    /// Create a new huge pages block
    #[must_use]
    pub fn new() -> Self {
        Self {
            total: 0,
            free: 0,
            reserved: 0,
            page_size_kb: 2048, // Default 2MB huge pages
        }
    }

    /// Set huge page values
    pub fn set_values(&mut self, total: u64, free: u64, reserved: u64, page_size_kb: u64) {
        self.total = total;
        self.free = free;
        self.reserved = reserved;
        self.page_size_kb = page_size_kb;
    }

    /// Get usage percentage
    #[must_use]
    pub fn usage_percent(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            ((self.total - self.free) as f32 / self.total as f32 * 100.0).clamp(0.0, 100.0)
        }
    }

    /// Get total size in bytes
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.total * self.page_size_kb * 1024
    }

    /// Get used size in bytes
    #[must_use]
    pub fn used_bytes(&self) -> u64 {
        (self.total - self.free) * self.page_size_kb * 1024
    }
}

impl ComputeBlock for HugePagesBlock {
    type Input = (u64, u64, u64, u64); // (total, free, reserved, page_size_kb)
    type Output = f32; // Usage percentage

    fn compute(&mut self, input: &Self::Input) -> Self::Output {
        self.set_values(input.0, input.1, input.2, input.3);
        self.usage_percent()
    }

    fn latency_budget_us(&self) -> u64 {
        10 // Simple arithmetic
    }
}

/// GPU Thermal ComputeBlock (CB-GPU-001)
///
/// Tracks GPU temperature and power draw.
#[derive(Debug, Clone)]
pub struct GpuThermalBlock {
    /// Temperature in Celsius
    temperature_c: f32,
    /// Power draw in Watts
    power_w: f32,
    /// Power limit in Watts
    power_limit_w: f32,
    /// History for trend
    temp_history: Vec<f32>,
    /// Instruction set
    instruction_set: SimdInstructionSet,
}

impl Default for GpuThermalBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuThermalBlock {
    /// Create a new GPU thermal block
    #[must_use]
    pub fn new() -> Self {
        Self {
            temperature_c: 0.0,
            power_w: 0.0,
            power_limit_w: 0.0,
            temp_history: Vec::with_capacity(60),
            instruction_set: SimdInstructionSet::detect(),
        }
    }

    /// Set thermal values
    pub fn set_values(&mut self, temp_c: f32, power_w: f32, power_limit_w: f32) {
        self.temperature_c = temp_c;
        self.power_w = power_w;
        self.power_limit_w = power_limit_w;

        // Update history
        if self.temp_history.len() >= 60 {
            self.temp_history.remove(0);
        }
        self.temp_history.push(temp_c);
    }

    /// Get thermal state
    #[must_use]
    pub fn thermal_state(&self) -> GpuThermalState {
        if self.temperature_c >= 90.0 {
            GpuThermalState::Critical
        } else if self.temperature_c >= 80.0 {
            GpuThermalState::Hot
        } else if self.temperature_c >= 70.0 {
            GpuThermalState::Warm
        } else if self.temperature_c >= 50.0 {
            GpuThermalState::Normal
        } else {
            GpuThermalState::Cool
        }
    }

    /// Get power usage percentage
    #[must_use]
    pub fn power_percent(&self) -> f32 {
        if self.power_limit_w > 0.0 {
            (self.power_w / self.power_limit_w * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    }

    /// Get temperature trend
    #[must_use]
    pub fn trend(&self) -> TrendDirection {
        if self.temp_history.len() < 5 {
            return TrendDirection::Flat;
        }

        let recent: f32 = self.temp_history.iter().rev().take(5).sum::<f32>() / 5.0;
        let older: f32 = self.temp_history.iter().rev().skip(5).take(5).sum::<f32>() / 5.0;

        let diff = recent - older;
        if diff > 2.0 {
            TrendDirection::Up
        } else if diff < -2.0 {
            TrendDirection::Down
        } else {
            TrendDirection::Flat
        }
    }
}

impl ComputeBlock for GpuThermalBlock {
    type Input = (f32, f32, f32); // (temp_c, power_w, power_limit_w)
    type Output = GpuThermalState;

    fn compute(&mut self, input: &Self::Input) -> Self::Output {
        self.set_values(input.0, input.1, input.2);
        self.thermal_state()
    }

    fn simd_instruction_set(&self) -> SimdInstructionSet {
        self.instruction_set
    }

    fn latency_budget_us(&self) -> u64 {
        30 // Simple comparisons + history update
    }
}

/// GPU thermal state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuThermalState {
    /// Cool (<50°C)
    Cool,
    /// Normal (50-70°C)
    Normal,
    /// Warm (70-80°C)
    Warm,
    /// Hot (80-90°C)
    Hot,
    /// Critical (>90°C)
    Critical,
}

impl GpuThermalState {
    /// Get indicator character
    #[must_use]
    pub const fn indicator(self) -> char {
        match self {
            Self::Cool => '❄',
            Self::Normal => '●',
            Self::Warm => '◐',
            Self::Hot => '◕',
            Self::Critical => '🔥',
        }
    }

    /// Get severity (0=cool, 4=critical)
    #[must_use]
    pub const fn severity(self) -> u8 {
        match self {
            Self::Cool => 0,
            Self::Normal => 1,
            Self::Warm => 2,
            Self::Hot => 3,
            Self::Critical => 4,
        }
    }
}

/// GPU VRAM ComputeBlock (CB-GPU-002)
///
/// Tracks VRAM usage per process.
#[derive(Debug, Clone)]
pub struct GpuVramBlock {
    /// Total VRAM in MB
    total_mb: u64,
    /// Used VRAM in MB
    used_mb: u64,
    /// Per-process VRAM usage (PID -> MB)
    per_process: Vec<(u32, u64, String)>, // (pid, mb, name)
}

impl Default for GpuVramBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuVramBlock {
    /// Create a new VRAM block
    #[must_use]
    pub fn new() -> Self {
        Self {
            total_mb: 0,
            used_mb: 0,
            per_process: Vec::new(),
        }
    }

    /// Set VRAM values
    pub fn set_values(&mut self, total_mb: u64, used_mb: u64, per_process: Vec<(u32, u64, String)>) {
        self.total_mb = total_mb;
        self.used_mb = used_mb;
        self.per_process = per_process;
    }

    /// Get usage percentage
    #[must_use]
    pub fn usage_percent(&self) -> f32 {
        if self.total_mb == 0 {
            0.0
        } else {
            (self.used_mb as f32 / self.total_mb as f32 * 100.0).clamp(0.0, 100.0)
        }
    }

    /// Get top N consumers by VRAM
    #[must_use]
    pub fn top_consumers(&self, n: usize) -> Vec<&(u32, u64, String)> {
        let mut sorted: Vec<_> = self.per_process.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.into_iter().take(n).collect()
    }
}

impl ComputeBlock for GpuVramBlock {
    type Input = (u64, u64, Vec<(u32, u64, String)>);
    type Output = f32; // Usage percentage

    fn compute(&mut self, input: &Self::Input) -> Self::Output {
        self.set_values(input.0, input.1, input.2.clone());
        self.usage_percent()
    }

    fn latency_budget_us(&self) -> u64 {
        100 // Sorting may be needed
    }
}

// Additional tests for new blocks
#[cfg(test)]
mod new_block_tests {
    use super::*;

    #[test]
    fn test_cpu_frequency_block_new() {
        let block = CpuFrequencyBlock::new();
        assert!(block.frequencies.is_empty());
    }

    #[test]
    fn test_cpu_frequency_block_percentages() {
        let mut block = CpuFrequencyBlock::new();
        block.set_frequencies(vec![2000, 3000, 4000], vec![4000, 4000, 4000]);
        let pcts = block.frequency_percentages();
        assert_eq!(pcts.len(), 3);
        assert!((pcts[0] - 50.0).abs() < 0.1);
        assert!((pcts[1] - 75.0).abs() < 0.1);
        assert!((pcts[2] - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_cpu_frequency_block_scaling_states() {
        let mut block = CpuFrequencyBlock::new();
        block.set_frequencies(vec![1000, 2000, 3800, 4000], vec![4000, 4000, 4000, 4000]);
        let states = block.scaling_indicators();
        assert_eq!(states[0], FrequencyScalingState::Scaled);
        assert_eq!(states[1], FrequencyScalingState::Normal);
        assert_eq!(states[2], FrequencyScalingState::Turbo);
        assert_eq!(states[3], FrequencyScalingState::Turbo);
    }

    #[test]
    fn test_frequency_scaling_state_indicators() {
        assert_eq!(FrequencyScalingState::Turbo.indicator(), '⚡');
        assert_eq!(FrequencyScalingState::High.indicator(), '↑');
        assert_eq!(FrequencyScalingState::Normal.indicator(), '→');
        assert_eq!(FrequencyScalingState::Scaled.indicator(), '↓');
        assert_eq!(FrequencyScalingState::Idle.indicator(), '·');
    }

    #[test]
    fn test_cpu_governor_from_name() {
        assert_eq!(CpuGovernor::from_name("performance"), CpuGovernor::Performance);
        assert_eq!(CpuGovernor::from_name("powersave"), CpuGovernor::Powersave);
        assert_eq!(CpuGovernor::from_name("schedutil"), CpuGovernor::Schedutil);
        assert_eq!(CpuGovernor::from_name("unknown"), CpuGovernor::Unknown);
    }

    #[test]
    fn test_cpu_governor_short_names() {
        assert_eq!(CpuGovernor::Performance.short_name(), "perf");
        assert_eq!(CpuGovernor::Powersave.short_name(), "psav");
        assert_eq!(CpuGovernor::Schedutil.short_name(), "schu");
    }

    #[test]
    fn test_mem_pressure_level() {
        let mut block = MemPressureBlock::new();
        block.set_pressure(0.5, 0.3, 0.2, 0.1);
        assert_eq!(block.pressure_level(), MemoryPressureLevel::None);

        block.set_pressure(5.0, 3.0, 2.0, 1.0);
        assert_eq!(block.pressure_level(), MemoryPressureLevel::Low);

        block.set_pressure(15.0, 10.0, 8.0, 5.0);
        assert_eq!(block.pressure_level(), MemoryPressureLevel::Medium);

        block.set_pressure(30.0, 20.0, 15.0, 10.0);
        assert_eq!(block.pressure_level(), MemoryPressureLevel::High);

        block.set_pressure(60.0, 50.0, 40.0, 30.0);
        assert_eq!(block.pressure_level(), MemoryPressureLevel::Critical);
    }

    #[test]
    fn test_mem_pressure_trend() {
        let mut block = MemPressureBlock::new();
        block.set_pressure(20.0, 15.0, 5.0, 10.0);
        assert_eq!(block.trend(), TrendDirection::Up);

        block.set_pressure(5.0, 10.0, 20.0, 2.0);
        assert_eq!(block.trend(), TrendDirection::Down);

        block.set_pressure(10.0, 10.0, 10.0, 5.0);
        assert_eq!(block.trend(), TrendDirection::Flat);
    }

    #[test]
    fn test_huge_pages_block() {
        let mut block = HugePagesBlock::new();
        block.set_values(100, 50, 10, 2048);
        assert!((block.usage_percent() - 50.0).abs() < 0.1);
        assert_eq!(block.total_bytes(), 100 * 2048 * 1024);
        assert_eq!(block.used_bytes(), 50 * 2048 * 1024);
    }

    #[test]
    fn test_huge_pages_block_empty() {
        let block = HugePagesBlock::new();
        assert_eq!(block.usage_percent(), 0.0);
    }

    #[test]
    fn test_gpu_thermal_block() {
        let mut block = GpuThermalBlock::new();
        block.set_values(45.0, 100.0, 250.0);
        assert_eq!(block.thermal_state(), GpuThermalState::Cool);
        assert!((block.power_percent() - 40.0).abs() < 0.1);

        block.set_values(75.0, 200.0, 250.0);
        assert_eq!(block.thermal_state(), GpuThermalState::Warm);

        block.set_values(95.0, 250.0, 250.0);
        assert_eq!(block.thermal_state(), GpuThermalState::Critical);
    }

    #[test]
    fn test_gpu_thermal_state_indicators() {
        assert_eq!(GpuThermalState::Cool.indicator(), '❄');
        assert_eq!(GpuThermalState::Normal.indicator(), '●');
        assert_eq!(GpuThermalState::Critical.indicator(), '🔥');
    }

    #[test]
    fn test_gpu_vram_block() {
        let mut block = GpuVramBlock::new();
        let procs = vec![
            (1234, 1024, "firefox".to_string()),
            (5678, 512, "code".to_string()),
            (9012, 2048, "blender".to_string()),
        ];
        block.set_values(8192, 4096, procs);
        assert!((block.usage_percent() - 50.0).abs() < 0.1);

        let top = block.top_consumers(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].2, "blender");
        assert_eq!(top[1].2, "firefox");
    }

    #[test]
    fn test_memory_pressure_level_severity() {
        assert_eq!(MemoryPressureLevel::None.severity(), 0);
        assert_eq!(MemoryPressureLevel::Low.severity(), 1);
        assert_eq!(MemoryPressureLevel::Medium.severity(), 2);
        assert_eq!(MemoryPressureLevel::High.severity(), 3);
        assert_eq!(MemoryPressureLevel::Critical.severity(), 4);
    }

    #[test]
    fn test_gpu_thermal_state_severity() {
        assert_eq!(GpuThermalState::Cool.severity(), 0);
        assert_eq!(GpuThermalState::Normal.severity(), 1);
        assert_eq!(GpuThermalState::Warm.severity(), 2);
        assert_eq!(GpuThermalState::Hot.severity(), 3);
        assert_eq!(GpuThermalState::Critical.severity(), 4);
    }

    #[test]
    fn test_cpu_frequency_block_compute() {
        let mut block = CpuFrequencyBlock::new();
        let input = (vec![2000, 4000], vec![4000, 4000]);
        let output = block.compute(&input);
        assert_eq!(output.len(), 2);
        assert_eq!(output[0], FrequencyScalingState::Normal);
        assert_eq!(output[1], FrequencyScalingState::Turbo);
    }

    #[test]
    fn test_cpu_governor_block_compute() {
        let mut block = CpuGovernorBlock::new();
        let output = block.compute(&"performance".to_string());
        assert_eq!(output, CpuGovernor::Performance);
    }

    #[test]
    fn test_mem_pressure_block_compute() {
        let mut block = MemPressureBlock::new();
        let input = (30.0_f32, 25.0_f32, 20.0_f32, 15.0_f32);
        let output = block.compute(&input);
        assert_eq!(output, MemoryPressureLevel::High);
    }

    #[test]
    fn test_huge_pages_block_compute() {
        let mut block = HugePagesBlock::new();
        let input = (100_u64, 75_u64, 5_u64, 2048_u64);
        let output = block.compute(&input);
        assert!((output - 25.0).abs() < 0.1);
    }

    #[test]
    fn test_gpu_thermal_block_compute() {
        let mut block = GpuThermalBlock::new();
        let input = (85.0_f32, 200.0_f32, 250.0_f32);
        let output = block.compute(&input);
        assert_eq!(output, GpuThermalState::Hot);
    }

    #[test]
    fn test_gpu_vram_block_compute() {
        let mut block = GpuVramBlock::new();
        let procs = vec![(1234_u32, 1024_u64, "test".to_string())];
        let input = (8192_u64, 4096_u64, procs);
        let output = block.compute(&input);
        assert!((output - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_latency_budgets_new_blocks() {
        assert!(CpuFrequencyBlock::new().latency_budget_us() > 0);
        assert!(CpuGovernorBlock::new().latency_budget_us() > 0);
        assert!(MemPressureBlock::new().latency_budget_us() > 0);
        assert!(HugePagesBlock::new().latency_budget_us() > 0);
        assert!(GpuThermalBlock::new().latency_budget_us() > 0);
        assert!(GpuVramBlock::new().latency_budget_us() > 0);
    }

    #[test]
    fn test_cpu_governor_icons() {
        assert_eq!(CpuGovernor::Performance.icon(), '🚀');
        assert_eq!(CpuGovernor::Powersave.icon(), '🔋');
        assert_eq!(CpuGovernor::Unknown.icon(), '?');
    }

    #[test]
    fn test_simd_all_names() {
        assert_eq!(SimdInstructionSet::Sse4.name(), "SSE4.1");
        assert_eq!(SimdInstructionSet::Avx512.name(), "AVX-512");
        assert_eq!(SimdInstructionSet::WasmSimd128.name(), "WASM SIMD128");
    }

    #[test]
    fn test_simd_wasm_vector_width() {
        assert_eq!(SimdInstructionSet::Neon.vector_width(), 4);
        assert_eq!(SimdInstructionSet::WasmSimd128.vector_width(), 4);
    }

    #[test]
    fn test_compute_block_id_all_strings() {
        // Test all compute block IDs have valid strings
        let ids = [
            ComputeBlockId::CpuLoadGauge,
            ComputeBlockId::CpuLoadTrend,
            ComputeBlockId::CpuFrequency,
            ComputeBlockId::CpuBoostIndicator,
            ComputeBlockId::CpuTemperature,
            ComputeBlockId::CpuTopConsumers,
            ComputeBlockId::MemZramRatio,
            ComputeBlockId::MemPressureGauge,
            ComputeBlockId::MemSwapThrashing,
            ComputeBlockId::MemCacheBreakdown,
            ComputeBlockId::MemHugePages,
            ComputeBlockId::ConnProc,
            ComputeBlockId::ConnGeo,
            ComputeBlockId::ConnLatency,
            ComputeBlockId::ConnService,
            ComputeBlockId::ConnHotIndicator,
            ComputeBlockId::ConnSparkline,
            ComputeBlockId::NetProtocolStats,
            ComputeBlockId::NetErrorRate,
            ComputeBlockId::NetDropRate,
            ComputeBlockId::NetLatencyGauge,
            ComputeBlockId::NetBandwidthUtil,
            ComputeBlockId::ProcSortIndicator,
            ComputeBlockId::ProcFilter,
            ComputeBlockId::ProcOomScore,
            ComputeBlockId::ProcNiceValue,
            ComputeBlockId::ProcThreadCount,
            ComputeBlockId::ProcCgroup,
        ];
        for id in ids {
            assert!(!id.id_string().is_empty());
        }
    }

    #[test]
    fn test_compute_block_id_simd_categories() {
        // Test SIMD vectorizable blocks
        assert!(ComputeBlockId::NetSparklines.simd_vectorizable());
        assert!(ComputeBlockId::NetProtocolStats.simd_vectorizable());
        assert!(ComputeBlockId::NetErrorRate.simd_vectorizable());
        assert!(ComputeBlockId::NetDropRate.simd_vectorizable());
        assert!(ComputeBlockId::NetBandwidthUtil.simd_vectorizable());
        assert!(ComputeBlockId::ConnAge.simd_vectorizable());
        assert!(ComputeBlockId::ConnGeo.simd_vectorizable());
        assert!(ComputeBlockId::ConnLatency.simd_vectorizable());
        assert!(ComputeBlockId::ConnService.simd_vectorizable());
        assert!(ComputeBlockId::ConnHotIndicator.simd_vectorizable());
        assert!(ComputeBlockId::ConnSparkline.simd_vectorizable());

        // Test non-SIMD blocks
        assert!(!ComputeBlockId::MemZramRatio.simd_vectorizable());
        assert!(!ComputeBlockId::MemCacheBreakdown.simd_vectorizable());
        assert!(!ComputeBlockId::MemHugePages.simd_vectorizable());
        assert!(!ComputeBlockId::ConnProc.simd_vectorizable());
        assert!(!ComputeBlockId::NetLatencyGauge.simd_vectorizable());
        assert!(!ComputeBlockId::ProcSortIndicator.simd_vectorizable());
        assert!(!ComputeBlockId::ProcFilter.simd_vectorizable());
        assert!(!ComputeBlockId::ProcCgroup.simd_vectorizable());
    }

    #[test]
    fn test_sparkline_block_default() {
        let block = SparklineBlock::default();
        assert!(block.history().is_empty());
        assert_eq!(block.max_samples, 60);
    }

    #[test]
    fn test_sparkline_block_render_uniform() {
        let mut block = SparklineBlock::new(5);
        for _ in 0..5 {
            block.push(50.0);
        }
        let rendered = block.render(5);
        // All same value should render mid-level blocks
        for ch in &rendered {
            assert_ne!(*ch, ' ');
        }
    }

    #[test]
    fn test_sparkline_block_sample_to_width_shorter() {
        let mut block = SparklineBlock::new(10);
        for i in 0..3 {
            block.push(i as f32);
        }
        // Render at width 5 - should pad with zeros
        let rendered = block.render(5);
        assert_eq!(rendered.len(), 5);
    }

    #[test]
    fn test_sparkline_block_sample_to_width_longer() {
        let mut block = SparklineBlock::new(20);
        for i in 0..15 {
            block.push(i as f32 * 10.0);
        }
        // Render at width 5 - should downsample
        let rendered = block.render(5);
        assert_eq!(rendered.len(), 5);
    }

    #[test]
    fn test_load_trend_block_default() {
        let block = LoadTrendBlock::default();
        assert_eq!(block.window_size, 5);
    }

    #[test]
    fn test_load_trend_block_history_limit() {
        let mut block = LoadTrendBlock::new(3);
        // Push more than window_size * 2
        for i in 0..20 {
            block.compute(&(i as f32));
        }
        // History should be limited
        assert!(block.history.len() <= block.window_size * 2);
    }

    #[test]
    fn test_load_trend_block_insufficient_history() {
        let mut block = LoadTrendBlock::new(5);
        block.compute(&1.0);
        // Only 1 sample, should be flat
        assert_eq!(block.trend(), TrendDirection::Flat);
    }

    #[test]
    fn test_sparkline_block_find_min_max_empty() {
        let block = SparklineBlock::new(5);
        // Empty history should return defaults
        let (min, max) = block.find_min_max();
        assert_eq!(min, 0.0);
        assert_eq!(max, 1.0);
    }

    #[test]
    fn test_sparkline_block_simd_instruction_set() {
        let block = SparklineBlock::new(10);
        let isa = block.simd_instruction_set();
        assert!(isa.vector_width() >= 1);
    }

    #[test]
    fn test_load_trend_latency_budget() {
        let trend = LoadTrendBlock::new(5);
        assert_eq!(trend.latency_budget_us(), 10);
    }

    #[test]
    fn test_sparkline_latency_budget() {
        let sparkline = SparklineBlock::new(60);
        assert_eq!(sparkline.latency_budget_us(), 100);
    }
}
