//! Testing harness for Presentar applications.
//!
//! Zero external dependencies. Pure Rust + WASM only.

mod a11y;
pub mod grade;
mod harness;
mod selector;
mod snapshot;

pub use a11y::{A11yChecker, A11yReport, A11yViolation};
pub use grade::{Criterion, EvaluationBuilder, Grade, ReportCard};
pub use harness::Harness;
pub use selector::{Selector, SelectorParser};
pub use snapshot::Snapshot;
