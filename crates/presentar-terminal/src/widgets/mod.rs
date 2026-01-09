//! Standard terminal widgets implementing Brick + Widget traits.
//!
//! All widgets in this module follow PROBAR-SPEC-009:
//! - Non-empty assertions (tests define interface)
//! - Performance budgets
//! - Jidoka verification gates

mod graph;
mod meter;
mod table;

pub use graph::{BrailleGraph, GraphMode};
pub use meter::Meter;
pub use table::Table;
