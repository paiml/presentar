//! Terminal backend for Presentar UI framework.
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::suboptimal_flops)]
#![allow(clippy::cast_lossless)]
//!
//! This crate bridges `presentar_core` abstractions (Canvas, Widget, Brick) to
//! the terminal using `crossterm` directly.
//!
//! # Architecture (PROBAR-SPEC-009)
//!
//! The crate follows the Brick Architecture from PROBAR-SPEC-009:
//!
//! - All widgets implement `Brick` trait (tests define interface)
//! - Jidoka gate prevents rendering if assertions fail
//! - Performance budgets are enforced
//! - Zero-allocation steady-state rendering via direct crossterm backend
//!
//! # Example
//!
//! ```ignore
//! use presentar_terminal::{TuiApp, TuiConfig};
//! use presentar_core::{Brick, Widget};
//!
//! // Create your root widget (must implement Widget + Brick)
//! let root = MyRootWidget::new();
//!
//! // Run the application
//! TuiApp::new(root)?.run()?;
//! ```

mod app;
mod color;
pub mod compute_block;
pub mod direct;
mod error;
mod input;
pub mod perf_trace;
pub mod random_seed;
pub mod seed;
pub mod theme;
pub mod tools;
pub mod widgets;

#[cfg(feature = "ptop")]
pub mod ptop;

// Re-export main types
pub use app::{AsyncCollector, QaTimings, Snapshot, SnapshotReceiver, TuiApp, TuiConfig};
pub use color::ColorMode;
pub use direct::{Cell, CellBuffer, DiffRenderer, DirectTerminalCanvas, Modifiers};
pub use error::TuiError;
pub use input::{InputHandler, KeyBinding};
pub use theme::{Gradient, Theme};

// Re-export widget types
pub use widgets::{
    truncate,
    // Data science widgets (sklearn/ggplot style)
    Axis,
    BarStyle,
    BinStrategy,
    // Legacy widgets (being phased out)
    Border,
    BorderStyle,
    BoxPlot,
    BoxStats,
    BrailleGraph,
    ConfusionMatrix,
    CpuGrid,
    CurveData,
    CurveMode,
    EmaConfig,
    // UX utilities
    EmptyState,
    ForceGraph,
    ForceParams,
    Gauge,
    GaugeMode,
    GraphEdge,
    GraphMode,
    GraphNode,
    HealthStatus,
    Heatmap,
    HeatmapCell,
    HeatmapPalette,
    Histogram,
    HistogramOrientation,
    HorizonGraph,
    HorizonScheme,
    HugePages,
    LegendPosition,
    LineChart,
    LineStyle,
    LossCurve,
    LossSeries,
    MarkerStyle,
    MatrixPalette,
    MemoryBar,
    MemorySegment,
    Meter,
    MultiBarGraph,
    MultiBarMode,
    NetworkInterface,
    NetworkPanel,
    NodeId,
    Normalization,
    Orientation,
    ProcessEntry,
    ProcessSort,
    ProcessState,
    ProcessTable,
    RocPrCurve,
    ScatterAxis,
    ScatterPlot,
    Segment,
    SegmentedMeter,
    Series,
    Simplification,
    Sparkline,
    Table,
    TitleBar,
    TitleBarPosition,
    TitleBarStyle,
    Tree,
    TreeNode,
    Treemap,
    TreemapLayout,
    TreemapNode,
    TrendDirection,
    ViolinData,
    ViolinOrientation,
    ViolinPlot,
    ViolinStats,
};

// Re-export core types for convenience
pub use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Point, Rect,
    Size, TextStyle, Widget,
};

// Re-export random seed types (F1: Popper falsifiability)
pub use random_seed::{
    get_seed, init_from_env, set_global_seed, with_seed, SeededRng, DEFAULT_SEED,
};

// Re-export ComputeBlock types (SPEC-024 Section 15, 20)
pub use compute_block::{
    ComputeBlock, ComputeBlockId, CpuFrequencyBlock, CpuGovernor, CpuGovernorBlock,
    FrequencyScalingState, GpuThermalBlock, GpuThermalState, GpuVramBlock, HugePagesBlock,
    LoadTrendBlock, MemPressureBlock, MemoryPressureLevel, SimdInstructionSet, SparklineBlock,
    TrendDirection as ComputeTrendDirection,
};
