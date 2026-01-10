//! Pressure Stall Information (PSI) Analyzer
//!
//! Reads Linux PSI metrics from `/proc/pressure/{cpu,memory,io}`.
//! PSI was introduced in Linux 4.20 and provides metrics for:
//! - CPU pressure (tasks waiting for CPU time)
//! - Memory pressure (tasks reclaiming memory)
//! - I/O pressure (tasks waiting for I/O)
//!
//! Each resource has two metrics:
//! - `some`: % of time at least one task was stalled
//! - `full`: % of time ALL tasks were stalled (not for CPU)

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unnecessary_wraps)]

use std::fs;
use std::time::Duration;

use super::{Analyzer, AnalyzerError};

/// PSI averaging windows
#[derive(Debug, Clone, Copy, Default)]
pub struct PsiAverages {
    /// 10-second average
    pub avg10: f64,
    /// 60-second average
    pub avg60: f64,
    /// 5-minute (300-second) average
    pub avg300: f64,
    /// Total stall time in microseconds
    pub total_us: u64,
}

/// PSI data for a single resource
#[derive(Debug, Clone, Copy, Default)]
pub struct PsiResource {
    /// "some" metrics - at least one task stalled
    pub some: PsiAverages,
    /// "full" metrics - all tasks stalled (None for CPU which has no full)
    pub full: Option<PsiAverages>,
}

/// All PSI data
#[derive(Debug, Clone, Default)]
pub struct PsiData {
    /// CPU pressure (some only, no full)
    pub cpu: PsiResource,
    /// Memory pressure (some and full)
    pub memory: PsiResource,
    /// I/O pressure (some and full)
    pub io: PsiResource,
    /// Whether PSI is available on this system
    pub available: bool,
}

impl PsiData {
    /// Check if any resource is under significant pressure (>5% some avg10)
    pub fn is_under_pressure(&self) -> bool {
        self.cpu.some.avg10 > 5.0 || self.memory.some.avg10 > 5.0 || self.io.some.avg10 > 5.0
    }

    /// Get the most pressured resource
    pub fn highest_pressure(&self) -> (&'static str, f64) {
        let cpu = self.cpu.some.avg10;
        let mem = self.memory.some.avg10;
        let io = self.io.some.avg10;

        if cpu >= mem && cpu >= io {
            ("cpu", cpu)
        } else if mem >= io {
            ("memory", mem)
        } else {
            ("io", io)
        }
    }
}

/// Analyzer for PSI metrics
pub struct PsiAnalyzer {
    data: PsiData,
    interval: Duration,
}

impl Default for PsiAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl PsiAnalyzer {
    /// Create a new PSI analyzer
    pub fn new() -> Self {
        Self {
            data: PsiData::default(),
            interval: Duration::from_secs(1),
        }
    }

    /// Get the current PSI data
    pub fn data(&self) -> &PsiData {
        &self.data
    }

    /// Parse a PSI file (cpu, memory, or io)
    fn parse_psi_file(path: &str) -> Result<PsiResource, AnalyzerError> {
        let contents = fs::read_to_string(path)
            .map_err(|e| AnalyzerError::IoError(format!("Failed to read {}: {}", path, e)))?;

        let mut resource = PsiResource::default();

        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let metric_type = parts[0];
            let avgs = Self::parse_averages(&parts[1..])?;

            match metric_type {
                "some" => resource.some = avgs,
                "full" => resource.full = Some(avgs),
                _ => {}
            }
        }

        Ok(resource)
    }

    /// Parse avg10=X avg60=Y avg300=Z total=T from parts
    fn parse_averages(parts: &[&str]) -> Result<PsiAverages, AnalyzerError> {
        let mut avgs = PsiAverages::default();

        for part in parts {
            if let Some((key, value)) = part.split_once('=') {
                match key {
                    "avg10" => avgs.avg10 = value.parse().unwrap_or(0.0),
                    "avg60" => avgs.avg60 = value.parse().unwrap_or(0.0),
                    "avg300" => avgs.avg300 = value.parse().unwrap_or(0.0),
                    "total" => avgs.total_us = value.parse().unwrap_or(0),
                    _ => {}
                }
            }
        }

        Ok(avgs)
    }
}

impl Analyzer for PsiAnalyzer {
    fn name(&self) -> &'static str {
        "psi"
    }

    fn collect(&mut self) -> Result<(), AnalyzerError> {
        // Check if PSI is available
        if !std::path::Path::new("/proc/pressure/cpu").exists() {
            self.data.available = false;
            return Ok(());
        }

        self.data.available = true;

        // Parse each PSI file
        if let Ok(cpu) = Self::parse_psi_file("/proc/pressure/cpu") {
            self.data.cpu = cpu;
            // CPU never has "full" metric
            self.data.cpu.full = None;
        }

        if let Ok(memory) = Self::parse_psi_file("/proc/pressure/memory") {
            self.data.memory = memory;
        }

        if let Ok(io) = Self::parse_psi_file("/proc/pressure/io") {
            self.data.io = io;
        }

        Ok(())
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    fn available(&self) -> bool {
        std::path::Path::new("/proc/pressure/cpu").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_averages() {
        let parts = [
            "avg10=3.89",
            "avg60=0.87",
            "avg300=0.61",
            "total=3052778300",
        ];
        let avgs = PsiAnalyzer::parse_averages(&parts).unwrap();

        assert!((avgs.avg10 - 3.89).abs() < 0.01);
        assert!((avgs.avg60 - 0.87).abs() < 0.01);
        assert!((avgs.avg300 - 0.61).abs() < 0.01);
        assert_eq!(avgs.total_us, 3052778300);
    }

    #[test]
    fn test_psi_data_pressure_check() {
        let mut data = PsiData::default();

        // No pressure
        assert!(!data.is_under_pressure());

        // CPU pressure
        data.cpu.some.avg10 = 10.0;
        assert!(data.is_under_pressure());

        // Highest pressure
        let (resource, value) = data.highest_pressure();
        assert_eq!(resource, "cpu");
        assert!((value - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_analyzer_available() {
        let analyzer = PsiAnalyzer::new();
        // This will be true on Linux 4.20+ with PSI enabled
        let available = analyzer.available();
        // Just ensure it doesn't panic
        let _ = available;
    }

    #[test]
    fn test_analyzer_collect() {
        let mut analyzer = PsiAnalyzer::new();
        // Should not panic even if PSI is not available
        let result = analyzer.collect();
        assert!(result.is_ok());

        // If PSI is available, data should be populated
        if analyzer.data().available {
            // CPU should have some but not full
            assert!(analyzer.data().cpu.full.is_none());
            // Memory and IO should have both
            assert!(analyzer.data().memory.full.is_some());
            assert!(analyzer.data().io.full.is_some());
        }
    }
}
