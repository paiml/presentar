//! File Analyzer
//!
//! Tracks file system activity, hot files (recently accessed), and provides
//! inode statistics. Uses /proc/[pid]/fd and atime for activity tracking.

#![allow(clippy::uninlined_format_args)]

use std::collections::HashMap;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{Analyzer, AnalyzerError};

/// Information about a tracked file
#[derive(Debug, Clone)]
pub struct TrackedFile {
    /// File path
    pub path: PathBuf,
    /// File size in bytes
    pub size: u64,
    /// Last access time
    pub accessed: Option<SystemTime>,
    /// Last modification time
    pub modified: Option<SystemTime>,
    /// Number of processes with this file open
    pub open_count: u32,
    /// PIDs of processes with this file open
    pub open_by: Vec<u32>,
    /// Inode number
    pub inode: u64,
    /// Device ID
    pub device: u64,
}

impl TrackedFile {
    /// Format size for display
    pub fn size_display(&self) -> String {
        format_size(self.size)
    }

    /// Is this file currently open by any process?
    pub fn is_open(&self) -> bool {
        self.open_count > 0
    }

    /// Time since last access
    pub fn time_since_access(&self) -> Option<Duration> {
        self.accessed.and_then(|t| t.elapsed().ok())
    }

    /// Is this a "hot" file (accessed recently)?
    pub fn is_hot(&self, threshold: Duration) -> bool {
        self.time_since_access().is_some_and(|d| d < threshold)
    }
}

/// File type category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileCategory {
    /// Regular file
    Regular,
    /// Directory
    Directory,
    /// Symbolic link
    Symlink,
    /// Socket
    Socket,
    /// FIFO/pipe
    Fifo,
    /// Block device
    BlockDevice,
    /// Character device
    CharDevice,
    /// Unknown
    Unknown,
}

impl FileCategory {
    /// Parse from file metadata
    pub fn from_metadata(meta: &Metadata) -> Self {
        use std::os::unix::fs::FileTypeExt;
        let ft = meta.file_type();
        if ft.is_file() {
            Self::Regular
        } else if ft.is_dir() {
            Self::Directory
        } else if ft.is_symlink() {
            Self::Symlink
        } else if ft.is_socket() {
            Self::Socket
        } else if ft.is_fifo() {
            Self::Fifo
        } else if ft.is_block_device() {
            Self::BlockDevice
        } else if ft.is_char_device() {
            Self::CharDevice
        } else {
            Self::Unknown
        }
    }

    /// Display name
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Regular => "file",
            Self::Directory => "dir",
            Self::Symlink => "link",
            Self::Socket => "sock",
            Self::Fifo => "fifo",
            Self::BlockDevice => "blk",
            Self::CharDevice => "chr",
            Self::Unknown => "?",
        }
    }
}

/// Inode statistics for a filesystem
#[derive(Debug, Clone, Default)]
pub struct InodeStats {
    /// Total inodes
    pub total: u64,
    /// Used inodes
    pub used: u64,
    /// Free inodes
    pub free: u64,
    /// Mount point
    pub mount_point: String,
}

impl InodeStats {
    /// Usage percentage
    pub fn usage_percent(&self) -> f64 {
        if self.total > 0 {
            self.used as f64 / self.total as f64 * 100.0
        } else {
            0.0
        }
    }

    /// Is inode usage critical (>90%)?
    pub fn is_critical(&self) -> bool {
        self.usage_percent() > 90.0
    }
}

/// File analyzer data
#[derive(Debug, Clone, Default)]
pub struct FileAnalyzerData {
    /// Hot files (recently accessed)
    pub hot_files: Vec<TrackedFile>,
    /// Open files by path
    pub open_files: HashMap<PathBuf, TrackedFile>,
    /// Inode stats per mount point
    pub inode_stats: HashMap<String, InodeStats>,
    /// Total open files
    pub total_open_files: usize,
    /// Total hot files
    pub total_hot_files: usize,
}

impl FileAnalyzerData {
    /// Get top N hot files by access time
    pub fn top_hot_files(&self, n: usize) -> impl Iterator<Item = &TrackedFile> {
        self.hot_files.iter().take(n)
    }

    /// Get files open by a specific process
    pub fn files_by_pid(&self, pid: u32) -> impl Iterator<Item = &TrackedFile> {
        self.open_files
            .values()
            .filter(move |f| f.open_by.contains(&pid))
    }
}

/// Analyzer for file activity and inode stats
pub struct FileAnalyzer {
    data: FileAnalyzerData,
    interval: Duration,
    /// Hot file threshold (files accessed within this duration)
    hot_threshold: Duration,
    /// Maximum files to track
    max_tracked: usize,
}

impl Default for FileAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FileAnalyzer {
    /// Create a new file analyzer
    pub fn new() -> Self {
        Self {
            data: FileAnalyzerData::default(),
            interval: Duration::from_secs(5),
            hot_threshold: Duration::from_secs(60), // Files accessed in last minute
            max_tracked: 100,
        }
    }

    /// Get the current data
    pub fn data(&self) -> &FileAnalyzerData {
        &self.data
    }

    /// Set hot file threshold
    pub fn set_hot_threshold(&mut self, threshold: Duration) {
        self.hot_threshold = threshold;
    }

    /// Scan /proc/[pid]/fd for open files
    fn scan_open_files(&self) -> HashMap<PathBuf, TrackedFile> {
        let mut files: HashMap<PathBuf, TrackedFile> = HashMap::new();

        let Ok(proc_entries) = fs::read_dir("/proc") else {
            return files;
        };

        for entry in proc_entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Only process numeric directories (PIDs)
            let Ok(pid) = name_str.parse::<u32>() else {
                continue;
            };

            let fd_path = entry.path().join("fd");
            let Ok(fd_entries) = fs::read_dir(&fd_path) else {
                continue;
            };

            for fd_entry in fd_entries.flatten() {
                let link = fd_entry.path();
                let Ok(target) = fs::read_link(&link) else {
                    continue;
                };

                // Skip non-file paths (sockets, pipes, etc.)
                if !target.starts_with("/")
                    || target.starts_with("/proc")
                    || target.starts_with("/dev")
                {
                    continue;
                }

                // Get or create file entry
                let file = files.entry(target.clone()).or_insert_with(|| {
                    let mut tracked = TrackedFile {
                        path: target.clone(),
                        size: 0,
                        accessed: None,
                        modified: None,
                        open_count: 0,
                        open_by: Vec::new(),
                        inode: 0,
                        device: 0,
                    };

                    // Get file metadata
                    if let Ok(meta) = fs::metadata(&target) {
                        use std::os::unix::fs::MetadataExt;
                        tracked.size = meta.len();
                        tracked.accessed = meta.accessed().ok();
                        tracked.modified = meta.modified().ok();
                        tracked.inode = meta.ino();
                        tracked.device = meta.dev();
                    }

                    tracked
                });

                file.open_count += 1;
                if !file.open_by.contains(&pid) {
                    file.open_by.push(pid);
                }
            }
        }

        files
    }

    /// Get inode stats from df -i
    fn get_inode_stats(&self) -> HashMap<String, InodeStats> {
        let mut stats = HashMap::new();

        let Ok(output) = std::process::Command::new("df")
            .arg("-i")
            .arg("--output=target,itotal,iused,iavail")
            .output()
        else {
            return stats;
        };

        if !output.status.success() {
            return stats;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let mount_point = parts[0].to_string();
                let total: u64 = parts[1].parse().unwrap_or(0);
                let used: u64 = parts[2].parse().unwrap_or(0);
                let free: u64 = parts[3].parse().unwrap_or(0);

                stats.insert(
                    mount_point.clone(),
                    InodeStats {
                        total,
                        used,
                        free,
                        mount_point,
                    },
                );
            }
        }

        stats
    }

    /// Identify hot files from open files
    fn identify_hot_files(&self, open_files: &HashMap<PathBuf, TrackedFile>) -> Vec<TrackedFile> {
        let mut hot: Vec<TrackedFile> = open_files
            .values()
            .filter(|f| f.is_hot(self.hot_threshold))
            .cloned()
            .collect();

        // Sort by access time (most recent first)
        hot.sort_by(|a, b| {
            let a_time = a.time_since_access().unwrap_or(Duration::MAX);
            let b_time = b.time_since_access().unwrap_or(Duration::MAX);
            a_time.cmp(&b_time)
        });

        // Limit to max_tracked
        hot.truncate(self.max_tracked);
        hot
    }
}

impl Analyzer for FileAnalyzer {
    fn name(&self) -> &'static str {
        "file_analyzer"
    }

    fn collect(&mut self) -> Result<(), AnalyzerError> {
        let open_files = self.scan_open_files();
        let hot_files = self.identify_hot_files(&open_files);
        let inode_stats = self.get_inode_stats();

        let total_open = open_files.len();
        let total_hot = hot_files.len();

        self.data = FileAnalyzerData {
            hot_files,
            open_files,
            inode_stats,
            total_open_files: total_open,
            total_hot_files: total_hot,
        };

        Ok(())
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    fn available(&self) -> bool {
        Path::new("/proc").exists()
    }
}

/// Format size for display
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_category() {
        assert_eq!(FileCategory::Regular.as_str(), "file");
        assert_eq!(FileCategory::Directory.as_str(), "dir");
        assert_eq!(FileCategory::Socket.as_str(), "sock");
    }

    #[test]
    fn test_inode_stats() {
        let stats = InodeStats {
            total: 1000,
            used: 950,
            free: 50,
            mount_point: "/".to_string(),
        };

        assert!((stats.usage_percent() - 95.0).abs() < 0.1);
        assert!(stats.is_critical());
    }

    #[test]
    fn test_tracked_file() {
        let file = TrackedFile {
            path: PathBuf::from("/tmp/test"),
            size: 1024,
            accessed: Some(SystemTime::now()),
            modified: Some(SystemTime::now()),
            open_count: 2,
            open_by: vec![1234, 5678],
            inode: 12345,
            device: 1,
        };

        assert!(file.is_open());
        assert_eq!(file.size_display(), "1.0K");
        assert!(file.is_hot(Duration::from_secs(60)));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1536), "1.5K");
        assert_eq!(format_size(1_500_000), "1.4M");
    }

    #[test]
    fn test_analyzer_creation() {
        let analyzer = FileAnalyzer::new();
        assert_eq!(analyzer.name(), "file_analyzer");
    }

    #[test]
    fn test_analyzer_available() {
        let analyzer = FileAnalyzer::new();
        #[cfg(target_os = "linux")]
        assert!(analyzer.available());
    }

    #[test]
    fn test_analyzer_collect() {
        let mut analyzer = FileAnalyzer::new();
        let result = analyzer.collect();
        assert!(result.is_ok());
    }
}
