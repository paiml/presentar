//! Layout engine for Presentar UI framework.
//!
//! Implements Flexbox-inspired layout with SIMD acceleration.

mod cache;
mod engine;
mod flex;

pub use cache::LayoutCache;
pub use engine::LayoutEngine;
pub use flex::{FlexAlign, FlexDirection, FlexItem, FlexJustify};
