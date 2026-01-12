//! ptop: Pixel-perfect ttop clone using presentar-terminal widgets.
//!
//! This module provides the application logic for ptop, mirroring ttop's structure:
//! - `app`: Application state and data collectors
//! - `config`: YAML configuration and layout algorithms (SPEC-024 v5.0 Features A, B)
//! - `ui`: Layout and rendering using presentar-terminal widgets (v1 - raw `draw_text`)
//! - `ui_v2`: Pure widget composition (v2 - zero `draw_text`)
//! - `analyzers`: System analyzers for detailed metrics
//!
//! # SPEC-024 Architectural Enforcement
//!
//! This module CANNOT be compiled without its interface tests.
//! The enforcement below causes a compile error if tests don't exist.
//!
//! **TESTS DEFINE INTERFACE. IMPLEMENTATION FOLLOWS.**

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

// =============================================================================
// SPEC-024 ARCHITECTURAL ENFORCEMENT
// =============================================================================
//
// These constants require interface test files to exist at compile time.
// If a test file is missing, the build FAILS with a clear error message.
//
// This is NOT optional. This is NOT advisory. This is ARCHITECTURAL.
//
// To add a new feature to ptop:
// 1. Create the interface test FIRST in tests/
// 2. Add the include_str! line here
// 3. Implement the feature
//
// Without step 1 and 2, step 3 is IMPOSSIBLE.

/// ENFORCEMENT: Async data flow interface test MUST exist
#[doc(hidden)]
pub const _ENFORCE_ASYNC_INTERFACE: &str = include_str!("../../tests/cpu_exploded_async.rs");

/// ENFORCEMENT: Panel visibility tests MUST exist
#[doc(hidden)]
pub const _ENFORCE_VISIBILITY_TESTS: &str = include_str!("../../tests/cbtop_visibility.rs");

/// ENFORCEMENT: App/MetricsSnapshot interface tests MUST exist
#[doc(hidden)]
pub const _ENFORCE_APP_INTERFACE: &str = include_str!("../../tests/ptop_app_interface.rs");

/// ENFORCEMENT: Panel interface tests MUST exist
#[doc(hidden)]
pub const _ENFORCE_PANELS_INTERFACE: &str = include_str!("../../tests/ptop_panels_interface.rs");

/// ENFORCEMENT: Widget interface tests MUST exist
#[doc(hidden)]
pub const _ENFORCE_WIDGET_INTERFACE: &str = include_str!("../../tests/widget_interface_tests.rs");

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
