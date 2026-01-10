//! Tools for TUI comparison and verification
//!
//! This module contains utilities for pixel-perfect TUI comparison:
//! - CIEDE2000 color difference (ΔE00)
//! - Character-level diff (CLD)
//! - Structural similarity (SSIM)

mod color_diff;
#[cfg(feature = "tui-compare")]
mod tui_compare;

pub use color_diff::{average_delta_e, ciede2000, rgb_to_lab, DeltaECategory, Lab, Rgb};
#[cfg(feature = "tui-compare")]
pub use tui_compare::{
    compare_tui, generate_report, DiffCell, PanelResult, PanelThreshold, TuiComparisonConfig,
    TuiComparisonResult,
};
