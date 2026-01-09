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
pub mod direct;
mod error;
mod input;
pub mod theme;
pub mod widgets;

// Re-export main types
pub use app::{TuiApp, TuiConfig};
pub use color::ColorMode;
pub use direct::{Cell, CellBuffer, DiffRenderer, DirectTerminalCanvas, Modifiers};
pub use error::TuiError;
pub use input::{InputHandler, KeyBinding};
pub use theme::{Gradient, Theme};

// Re-export widget types
pub use widgets::{
    Border, BorderStyle, BoxPlot, BoxStats, BrailleGraph, ConfusionMatrix, CpuGrid, Gauge,
    GaugeMode, GraphMode, Heatmap, HeatmapCell, HeatmapPalette, MatrixPalette, MemoryBar,
    MemorySegment, Meter, MultiBarGraph, MultiBarMode, NetworkInterface, NetworkPanel, NodeId,
    Normalization, Orientation, ProcessEntry, ProcessSort, ProcessTable, Segment, SegmentedMeter,
    Sparkline, Table, Tree, TreeNode, TrendDirection,
};

// Re-export core types for convenience
pub use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Point, Rect,
    Size, TextStyle, Widget,
};
