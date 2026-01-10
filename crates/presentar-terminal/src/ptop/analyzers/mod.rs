//! System analyzers for ptop
//!
//! This module contains specialized analyzers that extract detailed system
//! information beyond what sysinfo provides. Each analyzer focuses on a
//! specific subsystem:
//!
//! - `PsiAnalyzer`: Pressure Stall Information (`/proc/pressure/*`)
//! - `ConnectionsAnalyzer`: Network connections (`/proc/net/tcp*`)
//! - `ProcessExtraAnalyzer`: Extended process info (cgroup, OOM, affinity)
//! - `SensorHealthAnalyzer`: Hardware sensors (`/sys/class/hwmon/`)

#![allow(clippy::redundant_closure_for_method_calls)]

use std::time::Duration;

mod connections;
mod process_extra;
mod psi;
mod sensor_health;

pub use connections::{ConnectionsAnalyzer, ConnectionsData, TcpConnection, TcpState};
pub use process_extra::{IoPriorityClass, ProcessExtra, ProcessExtraAnalyzer, ProcessExtraData};
pub use psi::{PsiAnalyzer, PsiAverages, PsiData, PsiResource};
pub use sensor_health::{
    SensorHealthAnalyzer, SensorHealthData, SensorReading, SensorStatus, SensorType,
};

/// Error type for analyzer operations
#[derive(Debug)]
pub enum AnalyzerError {
    /// I/O error reading system files
    IoError(String),
    /// Parse error in system data
    ParseError(String),
    /// Analyzer not available on this system
    NotAvailable(String),
}

impl std::fmt::Display for AnalyzerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(msg) => write!(f, "I/O error: {}", msg),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::NotAvailable(msg) => write!(f, "Not available: {}", msg),
        }
    }
}

impl std::error::Error for AnalyzerError {}

/// Trait for system analyzers
///
/// Each analyzer is responsible for collecting specific system metrics
/// from /proc, /sys, or other data sources.
pub trait Analyzer: Send + Sync {
    /// Analyzer name for logging/display
    fn name(&self) -> &'static str;

    /// Collect data from the system
    fn collect(&mut self) -> Result<(), AnalyzerError>;

    /// Get the recommended collection interval
    fn interval(&self) -> Duration;

    /// Check if this analyzer is available on this system
    fn available(&self) -> bool;
}

/// Registry of all analyzers
///
/// Manages lifecycle and collection for all system analyzers.
/// Analyzers that aren't available on the current system are set to None.
pub struct AnalyzerRegistry {
    /// PSI metrics
    pub psi: Option<PsiAnalyzer>,
    /// Network connections
    pub connections: Option<ConnectionsAnalyzer>,
    /// Extended process info
    pub process_extra: Option<ProcessExtraAnalyzer>,
    /// Hardware sensors
    pub sensor_health: Option<SensorHealthAnalyzer>,
}

impl Default for AnalyzerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzerRegistry {
    /// Create a new registry, auto-detecting available analyzers
    pub fn new() -> Self {
        let psi = {
            let analyzer = PsiAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let connections = {
            let analyzer = ConnectionsAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let process_extra = {
            let analyzer = ProcessExtraAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let sensor_health = {
            let analyzer = SensorHealthAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        Self {
            psi,
            connections,
            process_extra,
            sensor_health,
        }
    }

    /// Collect data from all available analyzers
    pub fn collect_all(&mut self) {
        if let Some(ref mut psi) = self.psi {
            let _ = psi.collect();
        }
        if let Some(ref mut connections) = self.connections {
            let _ = connections.collect();
        }
        if let Some(ref mut process_extra) = self.process_extra {
            let _ = process_extra.collect();
        }
        if let Some(ref mut sensor_health) = self.sensor_health {
            let _ = sensor_health.collect();
        }
    }

    /// Get PSI data if available
    pub fn psi_data(&self) -> Option<&PsiData> {
        self.psi.as_ref().map(|p| p.data())
    }

    /// Get connections data if available
    pub fn connections_data(&self) -> Option<&ConnectionsData> {
        self.connections.as_ref().map(|c| c.data())
    }

    /// Get process extra data if available
    pub fn process_extra_data(&self) -> Option<&ProcessExtraData> {
        self.process_extra.as_ref().map(|p| p.data())
    }

    /// Get sensor health data if available
    pub fn sensor_health_data(&self) -> Option<&SensorHealthData> {
        self.sensor_health.as_ref().map(|s| s.data())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = AnalyzerRegistry::new();
        // Just verify it doesn't panic
        let _ = registry.psi;
    }

    #[test]
    fn test_registry_collect() {
        let mut registry = AnalyzerRegistry::new();
        registry.collect_all();
        // Should not panic
    }
}
