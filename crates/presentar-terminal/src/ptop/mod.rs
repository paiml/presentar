//! ptop: Pixel-perfect ttop clone using presentar-terminal widgets.
//!
//! This module provides the application logic for ptop, mirroring ttop's structure:
//! - `app`: Application state and data collectors
//! - `config`: YAML configuration and layout algorithms (SPEC-024 v5.0 Features A, B)
//! - `ui`: Layout and rendering using presentar-terminal widgets (v1 - raw `draw_text`)
//! - `ui_v2`: Pure widget composition (v2 - zero `draw_text`)
//! - `analyzers`: System analyzers for detailed metrics

// Module-wide clippy allows for style-only warnings (SPEC-024 v5.8.0 quality compliance)
#![allow(clippy::too_many_lines)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::suboptimal_flops)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::needless_pass_by_ref_mut)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::no_effect_underscore_binding)]

pub mod analyzers;
pub mod app;
pub mod config;
pub mod input;
pub mod ui;
pub mod ui_v2;

pub use analyzers::{
    AnalyzerRegistry, ConnectionsAnalyzer, ConnectionsData, PsiAnalyzer, PsiData, TcpConnection,
    TcpState,
};
pub use app::App;
pub use config::{
    calculate_grid_layout, snap_to_grid, DetailLevel, FocusStyle, LayoutConfig, PanelConfig,
    PanelRect, PanelType, PtopConfig,
};
pub use input::{InputHandler, TimestampedKey};
pub use ui_v2::PtopView;
