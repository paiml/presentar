//! ptop: Pixel-perfect ttop clone using presentar-terminal widgets.
//!
//! This module provides the application logic for ptop, mirroring ttop's structure:
//! - `app`: Application state and data collectors
//! - `config`: YAML configuration and layout algorithms (SPEC-024 v5.0 Features A, B)
//! - `ui`: Layout and rendering using presentar-terminal widgets (v1 - raw `draw_text`)
//! - `ui_v2`: Pure widget composition (v2 - zero `draw_text`)
//! - `analyzers`: System analyzers for detailed metrics

pub mod analyzers;
pub mod app;
pub mod config;
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
pub use ui_v2::PtopView;
