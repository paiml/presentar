#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::disallowed_methods)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::similar_names)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::doc_markdown)]
//! Layout engine for Presentar UI framework.
//!
//! Implements Flexbox-inspired and CSS Grid layout with SIMD acceleration.

mod cache;
mod engine;
mod flex;
mod grid;

pub use cache::LayoutCache;
pub use engine::LayoutEngine;
pub use flex::{FlexAlign, FlexDirection, FlexItem, FlexJustify};
pub use grid::{
    auto_place_items, compute_grid_layout, GridAlign, GridArea, GridAutoFlow, GridItem,
    GridLayout, GridTemplate, TrackSize,
};
