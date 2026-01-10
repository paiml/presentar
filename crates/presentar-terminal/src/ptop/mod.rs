//! ptop: Pixel-perfect ttop clone using presentar-terminal widgets.
//!
//! This module provides the application logic for ptop, mirroring ttop's structure:
//! - `app`: Application state and data collectors
//! - `ui`: Layout and rendering using presentar-terminal widgets
//! - `analyzers`: System analyzers for detailed metrics

pub mod analyzers;
pub mod app;
pub mod ui;

pub use analyzers::{
    AnalyzerRegistry, ConnectionsAnalyzer, ConnectionsData, PsiAnalyzer, PsiData, TcpConnection,
    TcpState,
};
pub use app::App;
