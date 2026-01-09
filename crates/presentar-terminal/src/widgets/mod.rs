//! Standard terminal widgets implementing Brick + Widget traits.
//!
//! All widgets in this module follow PROBAR-SPEC-009:
//! - Non-empty assertions (tests define interface)
//! - Performance budgets
//! - Jidoka verification gates

mod border;
mod box_plot;
mod collapsible_panel;
mod confusion_matrix;
mod cpu_grid;
mod gauge;
mod graph;
mod heatmap;
mod memory_bar;
mod meter;
mod multi_bar;
mod network_panel;
mod process_table;
mod scrollbar;
mod segmented_meter;
mod sparkline;
mod symbols;
mod table;
mod text_input;
mod tree;

pub use border::{Border, BorderStyle};
pub use box_plot::{BoxPlot, BoxStats, Orientation};
pub use collapsible_panel::{CollapseDirection, CollapseIndicators, CollapsiblePanel};
pub use confusion_matrix::{ConfusionMatrix, MatrixPalette, Normalization};
pub use cpu_grid::CpuGrid;
pub use gauge::{Gauge, GaugeMode};
pub use graph::{BrailleGraph, GraphMode};
pub use heatmap::{Heatmap, HeatmapCell, HeatmapPalette};
pub use memory_bar::{MemoryBar, MemorySegment};
pub use meter::Meter;
pub use multi_bar::{MultiBarGraph, MultiBarMode};
pub use network_panel::{NetworkInterface, NetworkPanel};
pub use process_table::{ProcessEntry, ProcessSort, ProcessTable};
pub use scrollbar::{ScrollOrientation, Scrollbar, ScrollbarChars};
pub use segmented_meter::{Segment, SegmentedMeter};
pub use sparkline::{Sparkline, TrendDirection};
pub use symbols::{
    BrailleSymbols, CustomSymbols, SymbolSet, BLOCK_DOWN, BLOCK_UP, BRAILLE_DOWN, BRAILLE_UP,
    SPARKLINE, SUBSCRIPT, SUPERSCRIPT, TTY_DOWN, TTY_UP,
};
pub use table::Table;
pub use text_input::TextInput;
pub use tree::{NodeId, Tree, TreeNode};
