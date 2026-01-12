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
//! - `ContainersAnalyzer`: Docker/Podman container stats

#![allow(clippy::redundant_closure_for_method_calls)]

use std::time::Duration;

mod connections;
mod containers;
mod disk_entropy;
mod disk_io;
mod file_analyzer;
mod gpu_procs;
mod network_stats;
mod process_extra;
mod psi;
mod sensor_health;
mod storage;
mod swap;
mod treemap;

pub use connections::{ConnectionsAnalyzer, ConnectionsData, TcpConnection, TcpState};
pub use containers::{
    Container, ContainerRuntime, ContainerState, ContainerStats, ContainersAnalyzer, ContainersData,
};
pub use disk_entropy::{DiskEntropyAnalyzer, DiskEntropyData, DiskEntropyInfo, EncryptionType};
pub use disk_io::{DiskIoAnalyzer, DiskIoData, DiskIoRates, DiskIoStats};
pub use file_analyzer::{FileAnalyzer, FileAnalyzerData, FileCategory, InodeStats, TrackedFile};
pub use gpu_procs::{GpuInfo, GpuProcess, GpuProcsAnalyzer, GpuProcsData, GpuVendor};
pub use network_stats::{InterfaceRates, InterfaceStats, NetworkStatsAnalyzer, NetworkStatsData};
pub use process_extra::{IoPriorityClass, ProcessExtra, ProcessExtraAnalyzer, ProcessExtraData};
pub use psi::{PsiAnalyzer, PsiAverages, PsiData, PsiResource};
pub use sensor_health::{
    SensorHealthAnalyzer, SensorHealthData, SensorReading, SensorStatus, SensorType,
};
pub use storage::{MountInfo, StorageAnalyzer, StorageData};
pub use swap::{SwapAnalyzer, SwapData, SwapDevice, SwapType};
pub use treemap::{TreemapAnalyzer, TreemapConfig, TreemapData, TreemapNode};

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
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::ParseError(msg) => write!(f, "Parse error: {msg}"),
            Self::NotAvailable(msg) => write!(f, "Not available: {msg}"),
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
    /// Container stats (Docker/Podman)
    pub containers: Option<ContainersAnalyzer>,
    /// GPU process stats
    pub gpu_procs: Option<GpuProcsAnalyzer>,
    /// Filesystem treemap
    pub treemap: Option<TreemapAnalyzer>,
    /// Disk I/O statistics
    pub disk_io: Option<DiskIoAnalyzer>,
    /// Network interface statistics
    pub network_stats: Option<NetworkStatsAnalyzer>,
    /// Swap statistics
    pub swap: Option<SwapAnalyzer>,
    /// Storage/filesystem information
    pub storage: Option<StorageAnalyzer>,
    /// Disk entropy/encryption detection
    pub disk_entropy: Option<DiskEntropyAnalyzer>,
    /// File activity and inode stats
    pub file_analyzer: Option<FileAnalyzer>,
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

        let containers = {
            let analyzer = ContainersAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let gpu_procs = {
            let analyzer = GpuProcsAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let treemap = {
            let analyzer = TreemapAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let disk_io = {
            let analyzer = DiskIoAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let network_stats = {
            let analyzer = NetworkStatsAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let swap = {
            let analyzer = SwapAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let storage = {
            let analyzer = StorageAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let disk_entropy = {
            let analyzer = DiskEntropyAnalyzer::new();
            if analyzer.available() {
                Some(analyzer)
            } else {
                None
            }
        };

        let file_analyzer = {
            let analyzer = FileAnalyzer::new();
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
            containers,
            gpu_procs,
            treemap,
            disk_io,
            network_stats,
            swap,
            storage,
            disk_entropy,
            file_analyzer,
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
        if let Some(ref mut containers) = self.containers {
            let _ = containers.collect();
        }
        if let Some(ref mut gpu_procs) = self.gpu_procs {
            let _ = gpu_procs.collect();
        }
        if let Some(ref mut treemap) = self.treemap {
            let _ = treemap.collect();
        }
        if let Some(ref mut disk_io) = self.disk_io {
            let _ = disk_io.collect();
        }
        if let Some(ref mut network_stats) = self.network_stats {
            let _ = network_stats.collect();
        }
        if let Some(ref mut swap) = self.swap {
            let _ = swap.collect();
        }
        if let Some(ref mut storage) = self.storage {
            let _ = storage.collect();
        }
        if let Some(ref mut disk_entropy) = self.disk_entropy {
            let _ = disk_entropy.collect();
        }
        if let Some(ref mut file_analyzer) = self.file_analyzer {
            let _ = file_analyzer.collect();
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

    /// Get containers data if available
    pub fn containers_data(&self) -> Option<&ContainersData> {
        self.containers.as_ref().map(|c| c.data())
    }

    /// Get GPU processes data if available
    pub fn gpu_procs_data(&self) -> Option<&GpuProcsData> {
        self.gpu_procs.as_ref().map(|g| g.data())
    }

    /// Get treemap data if available
    pub fn treemap_data(&self) -> Option<&TreemapData> {
        self.treemap.as_ref().map(|t| t.data())
    }

    /// Get disk I/O data if available
    pub fn disk_io_data(&self) -> Option<&DiskIoData> {
        self.disk_io.as_ref().map(|d| d.data())
    }

    /// Get network stats data if available
    pub fn network_stats_data(&self) -> Option<&NetworkStatsData> {
        self.network_stats.as_ref().map(|n| n.data())
    }

    /// Get swap data if available
    pub fn swap_data(&self) -> Option<&SwapData> {
        self.swap.as_ref().map(|s| s.data())
    }

    /// Get storage data if available
    pub fn storage_data(&self) -> Option<&StorageData> {
        self.storage.as_ref().map(|s| s.data())
    }

    /// Get disk entropy data if available
    pub fn disk_entropy_data(&self) -> Option<&DiskEntropyData> {
        self.disk_entropy.as_ref().map(|d| d.data())
    }

    /// Get file analyzer data if available
    pub fn file_analyzer_data(&self) -> Option<&FileAnalyzerData> {
        self.file_analyzer.as_ref().map(|f| f.data())
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
