//! Core UI rendering modules for ptop.
//!
//! This module contains the exploded components of the original ui/core.rs,
//! organized into focused, testable modules following trueno-viz patterns.
//!
//! # Module Structure
//!
//! ```text
//! core/
//! ├── mod.rs           - This file, re-exports
//! ├── constants.rs     - Panel colors, status colors
//! ├── format.rs        - Formatting utilities
//! ├── border.rs        - Panel border creation
//! ├── render.rs        - Main rendering logic (legacy core.rs)
//! ```

pub mod border;
pub mod constants;
pub mod format;
mod render;

// Re-export rendering functions from render.rs
pub use render::{draw, panel_border_color, read_gpu_info, GpuInfo};

// Re-export border functions
pub use border::{
    blend_with_accent, brighten_color, create_panel_border, darken_color, dim_color, lerp_color,
    FOCUS_ACCENT_COLOR,
};
