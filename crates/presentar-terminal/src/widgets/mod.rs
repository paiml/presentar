//! Standard terminal widgets implementing Brick + Widget traits.
//!
//! All widgets in this module follow PROBAR-SPEC-009:
//! - Non-empty assertions (tests define interface)
//! - Performance budgets
//! - Jidoka verification gates

mod box_plot;
mod confusion_matrix;
mod gauge;
mod graph;
mod heatmap;
mod meter;
mod sparkline;
mod table;
mod tree;

pub use box_plot::{BoxPlot, BoxStats, Orientation};
pub use confusion_matrix::{ConfusionMatrix, MatrixPalette, Normalization};
pub use gauge::{Gauge, GaugeMode};
pub use graph::{BrailleGraph, GraphMode};
pub use heatmap::{Heatmap, HeatmapCell, HeatmapPalette};
pub use meter::Meter;
pub use sparkline::{Sparkline, TrendDirection};
pub use table::Table;
pub use tree::{NodeId, Tree, TreeNode};
