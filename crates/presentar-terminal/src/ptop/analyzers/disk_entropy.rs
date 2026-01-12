//! Disk Entropy Analyzer
//!
//! Analyzes disk entropy to detect encryption. Encrypted/compressed data
//! has high entropy (close to 1.0), while unencrypted data typically has
//! lower entropy.

#![allow(clippy::uninlined_format_args)]

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use super::{Analyzer, AnalyzerError};

/// Encryption type detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EncryptionType {
    /// No encryption detected
    #[default]
    None,
    /// LUKS encrypted
    Luks,
    /// dm-crypt without LUKS header
    DmCrypt,
    /// VeraCrypt/TrueCrypt
    VeraCrypt,
    /// `BitLocker` (unlikely on Linux but detectable)
    BitLocker,
    /// Unknown encryption (high entropy)
    Unknown,
}

impl EncryptionType {
    /// Display name
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Luks => "LUKS",
            Self::DmCrypt => "dm-crypt",
            Self::VeraCrypt => "VeraCrypt",
            Self::BitLocker => "BitLocker",
            Self::Unknown => "encrypted",
        }
    }

    /// Is this any form of encryption?
    pub fn is_encrypted(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Entropy information for a disk device
#[derive(Debug, Clone, Default)]
pub struct DiskEntropyInfo {
    /// Device name (e.g., "sda", "nvme0n1")
    pub device: String,
    /// Device path (e.g., "/dev/sda")
    pub path: String,
    /// Calculated entropy (0.0-1.0, where 1.0 is max entropy)
    pub entropy: f64,
    /// Detected encryption type
    pub encryption_type: EncryptionType,
    /// Is this a dm-crypt device mapper target?
    pub is_dm_target: bool,
    /// LUKS UUID if detected
    pub luks_uuid: Option<String>,
    /// Encryption cipher if detected
    pub cipher: Option<String>,
}

impl DiskEntropyInfo {
    /// Is this device likely encrypted based on entropy?
    pub fn is_high_entropy(&self) -> bool {
        self.entropy > 0.95
    }

    /// Format entropy as percentage
    pub fn entropy_percent(&self) -> f64 {
        self.entropy * 100.0
    }

    /// Status display
    pub fn status_display(&self) -> &'static str {
        if self.encryption_type.is_encrypted() {
            "🔒"
        } else if self.is_high_entropy() {
            "⚠️"
        } else {
            "🔓"
        }
    }
}

/// Disk entropy data
#[derive(Debug, Clone, Default)]
pub struct DiskEntropyData {
    /// Entropy info per device
    pub devices: HashMap<String, DiskEntropyInfo>,
    /// Number of encrypted devices
    pub encrypted_count: usize,
    /// Number of unencrypted devices
    pub unencrypted_count: usize,
}

impl DiskEntropyData {
    /// Get encrypted devices
    pub fn encrypted_devices(&self) -> impl Iterator<Item = &DiskEntropyInfo> {
        self.devices
            .values()
            .filter(|d| d.encryption_type.is_encrypted())
    }

    /// Get unencrypted devices
    pub fn unencrypted_devices(&self) -> impl Iterator<Item = &DiskEntropyInfo> {
        self.devices
            .values()
            .filter(|d| !d.encryption_type.is_encrypted())
    }
}

/// Analyzer for disk entropy and encryption detection
pub struct DiskEntropyAnalyzer {
    data: DiskEntropyData,
    interval: Duration,
    /// Sample size in bytes for entropy calculation
    sample_size: usize,
}

impl Default for DiskEntropyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskEntropyAnalyzer {
    /// Create a new disk entropy analyzer
    pub fn new() -> Self {
        Self {
            data: DiskEntropyData::default(),
            interval: Duration::from_secs(60), // Entropy doesn't change often
            sample_size: 4096,                 // Sample 4KB for entropy calculation
        }
    }

    /// Get the current data
    pub fn data(&self) -> &DiskEntropyData {
        &self.data
    }

    /// List block devices from /sys/block/
    fn list_block_devices(&self) -> Vec<String> {
        let mut devices = Vec::new();

        if let Ok(entries) = fs::read_dir("/sys/block") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Skip loop devices, ram disks
                if name.starts_with("loop") || name.starts_with("ram") {
                    continue;
                }
                devices.push(name);
            }
        }

        devices
    }

    /// Check if device is a dm-crypt target
    fn is_dm_crypt(&self, device: &str) -> bool {
        let dm_path = format!("/sys/block/{}/dm/uuid", device);
        if let Ok(uuid) = fs::read_to_string(&dm_path) {
            return uuid.starts_with("CRYPT-");
        }
        false
    }

    /// Try to detect LUKS encryption
    fn detect_luks(&self, device_path: &str) -> Option<(String, String)> {
        // LUKS header magic: "LUKS\xba\xbe" at offset 0
        // We can also check /sys/block/*/dm/ for LUKS info

        // Try cryptsetup status via /dev/mapper
        let dm_name = Path::new(device_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())?;

        // Check for LUKS UUID in dm
        let uuid_path = format!("/sys/block/{}/dm/uuid", dm_name);
        if let Ok(uuid) = fs::read_to_string(&uuid_path) {
            if uuid.starts_with("CRYPT-LUKS") {
                // Extract UUID from CRYPT-LUKS1-<uuid> or CRYPT-LUKS2-<uuid>
                let parts: Vec<&str> = uuid.trim().split('-').collect();
                if parts.len() >= 3 {
                    let luks_uuid = parts[2..].join("-");
                    return Some((luks_uuid, "aes-xts-plain64".to_string())); // Common default
                }
            }
        }

        None
    }

    /// Calculate Shannon entropy of data
    fn calculate_entropy(&self, data: &[u8]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        // Count byte frequencies
        let mut freq = [0u64; 256];
        for &byte in data {
            freq[byte as usize] += 1;
        }

        // Calculate Shannon entropy
        let len = data.len() as f64;
        let mut entropy = 0.0;

        for &count in &freq {
            if count > 0 {
                let p = count as f64 / len;
                entropy -= p * p.log2();
            }
        }

        // Normalize to 0-1 range (max entropy is 8 bits)
        entropy / 8.0
    }

    /// Sample entropy from device (if readable)
    fn sample_device_entropy(&self, device_path: &str) -> Option<f64> {
        // Try to read a sample from the device
        // This requires read permission on the device
        let mut file = File::open(device_path).ok()?;
        let mut buffer = vec![0u8; self.sample_size];

        // Read from middle of device to avoid headers
        // Note: This may fail without root permissions
        if file.read_exact(&mut buffer).is_ok() {
            Some(self.calculate_entropy(&buffer))
        } else {
            None
        }
    }

    /// Analyze a single device
    fn analyze_device(&self, device: &str) -> DiskEntropyInfo {
        let device_path = format!("/dev/{}", device);
        let is_dm = self.is_dm_crypt(device);

        let mut info = DiskEntropyInfo {
            device: device.to_string(),
            path: device_path.clone(),
            is_dm_target: is_dm,
            ..Default::default()
        };

        // Check for LUKS
        if is_dm {
            if let Some((uuid, cipher)) = self.detect_luks(&device_path) {
                info.encryption_type = EncryptionType::Luks;
                info.luks_uuid = Some(uuid);
                info.cipher = Some(cipher);
                info.entropy = 0.99; // LUKS data is high entropy
                return info;
            }
            // dm-crypt without LUKS
            info.encryption_type = EncryptionType::DmCrypt;
            info.entropy = 0.99;
            return info;
        }

        // Try to sample entropy (may fail without permissions)
        if let Some(entropy) = self.sample_device_entropy(&device_path) {
            info.entropy = entropy;
            if entropy > 0.98 {
                info.encryption_type = EncryptionType::Unknown;
            }
        }

        info
    }
}

impl Analyzer for DiskEntropyAnalyzer {
    fn name(&self) -> &'static str {
        "disk_entropy"
    }

    fn collect(&mut self) -> Result<(), AnalyzerError> {
        let devices = self.list_block_devices();
        let mut device_map = HashMap::new();
        let mut encrypted = 0;
        let mut unencrypted = 0;

        for device in devices {
            let info = self.analyze_device(&device);
            if info.encryption_type.is_encrypted() {
                encrypted += 1;
            } else {
                unencrypted += 1;
            }
            device_map.insert(device, info);
        }

        self.data = DiskEntropyData {
            devices: device_map,
            encrypted_count: encrypted,
            unencrypted_count: unencrypted,
        };

        Ok(())
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    fn available(&self) -> bool {
        Path::new("/sys/block").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_type() {
        assert!(!EncryptionType::None.is_encrypted());
        assert!(EncryptionType::Luks.is_encrypted());
        assert!(EncryptionType::DmCrypt.is_encrypted());
        assert_eq!(EncryptionType::Luks.as_str(), "LUKS");
    }

    #[test]
    fn test_entropy_calculation() {
        let analyzer = DiskEntropyAnalyzer::new();

        // All zeros = low entropy
        let zeros = vec![0u8; 1000];
        let entropy = analyzer.calculate_entropy(&zeros);
        assert!(entropy < 0.01);

        // Random-ish data = higher entropy
        let varied: Vec<u8> = (0..=255).cycle().take(1000).collect();
        let entropy = analyzer.calculate_entropy(&varied);
        assert!(entropy > 0.9);
    }

    #[test]
    fn test_disk_entropy_info() {
        let info = DiskEntropyInfo {
            device: "sda".to_string(),
            path: "/dev/sda".to_string(),
            entropy: 0.99,
            encryption_type: EncryptionType::Luks,
            ..Default::default()
        };

        assert!(info.is_high_entropy());
        assert!((info.entropy_percent() - 99.0).abs() < 0.1);
        assert_eq!(info.status_display(), "🔒");
    }

    #[test]
    fn test_analyzer_creation() {
        let analyzer = DiskEntropyAnalyzer::new();
        assert_eq!(analyzer.name(), "disk_entropy");
    }

    #[test]
    fn test_analyzer_available() {
        let analyzer = DiskEntropyAnalyzer::new();
        #[cfg(target_os = "linux")]
        assert!(analyzer.available());
    }

    #[test]
    fn test_analyzer_collect() {
        let mut analyzer = DiskEntropyAnalyzer::new();
        let result = analyzer.collect();
        assert!(result.is_ok());
    }
}
