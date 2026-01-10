//! Network Connections Analyzer
//!
//! Parses `/proc/net/tcp` and `/proc/net/tcp6` to show active network connections.
//! Maps sockets to processes via `/proc/[pid]/fd/` for PID/process name resolution.

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_closure_for_method_calls)]

use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::time::Duration;

use super::{Analyzer, AnalyzerError};

/// TCP connection state (from Linux kernel)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TcpState {
    Established = 1,
    SynSent = 2,
    SynRecv = 3,
    FinWait1 = 4,
    FinWait2 = 5,
    TimeWait = 6,
    Close = 7,
    CloseWait = 8,
    LastAck = 9,
    Listen = 10,
    Closing = 11,
    Unknown = 0,
}

impl TcpState {
    /// Parse state from hex string
    pub fn from_hex(hex: &str) -> Self {
        match u8::from_str_radix(hex, 16) {
            Ok(1) => Self::Established,
            Ok(2) => Self::SynSent,
            Ok(3) => Self::SynRecv,
            Ok(4) => Self::FinWait1,
            Ok(5) => Self::FinWait2,
            Ok(6) => Self::TimeWait,
            Ok(7) => Self::Close,
            Ok(8) => Self::CloseWait,
            Ok(9) => Self::LastAck,
            Ok(10) => Self::Listen,
            Ok(11) => Self::Closing,
            _ => Self::Unknown,
        }
    }

    /// Get display name
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Established => "ESTABLISHED",
            Self::SynSent => "SYN_SENT",
            Self::SynRecv => "SYN_RECV",
            Self::FinWait1 => "FIN_WAIT1",
            Self::FinWait2 => "FIN_WAIT2",
            Self::TimeWait => "TIME_WAIT",
            Self::Close => "CLOSE",
            Self::CloseWait => "CLOSE_WAIT",
            Self::LastAck => "LAST_ACK",
            Self::Listen => "LISTEN",
            Self::Closing => "CLOSING",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Short form for display
    pub fn short(&self) -> &'static str {
        match self {
            Self::Established => "ESTAB",
            Self::SynSent => "SYN_S",
            Self::SynRecv => "SYN_R",
            Self::FinWait1 => "FIN1",
            Self::FinWait2 => "FIN2",
            Self::TimeWait => "TIME_W",
            Self::Close => "CLOSE",
            Self::CloseWait => "CLOSEW",
            Self::LastAck => "LACK",
            Self::Listen => "LISTEN",
            Self::Closing => "CLOSNG",
            Self::Unknown => "UNK",
        }
    }
}

/// A single TCP connection
#[derive(Debug, Clone)]
pub struct TcpConnection {
    /// Local IP address
    pub local_addr: IpAddr,
    /// Local port
    pub local_port: u16,
    /// Remote IP address
    pub remote_addr: IpAddr,
    /// Remote port
    pub remote_port: u16,
    /// Connection state
    pub state: TcpState,
    /// Socket inode (for process mapping)
    pub inode: u64,
    /// User ID
    pub uid: u32,
    /// Process ID (if resolved)
    pub pid: Option<u32>,
    /// Process name (if resolved)
    pub process_name: Option<String>,
    /// IPv6 connection
    pub is_ipv6: bool,
}

impl TcpConnection {
    /// Format local address for display
    pub fn local_display(&self) -> String {
        format!("{}:{}", self.local_addr, self.local_port)
    }

    /// Format remote address for display
    pub fn remote_display(&self) -> String {
        if self.state == TcpState::Listen {
            "*:*".to_string()
        } else {
            format!("{}:{}", self.remote_addr, self.remote_port)
        }
    }

    /// Format process info for display
    pub fn process_display(&self) -> String {
        match (&self.pid, &self.process_name) {
            (Some(pid), Some(name)) => format!("{}/{}", pid, name),
            (Some(pid), None) => format!("{}", pid),
            _ => "-".to_string(),
        }
    }
}

/// Connections data
#[derive(Debug, Clone, Default)]
pub struct ConnectionsData {
    /// All TCP connections
    pub connections: Vec<TcpConnection>,
    /// Count by state
    pub state_counts: HashMap<TcpState, usize>,
}

impl ConnectionsData {
    /// Get connections filtered by state
    pub fn by_state(&self, state: TcpState) -> impl Iterator<Item = &TcpConnection> {
        self.connections.iter().filter(move |c| c.state == state)
    }

    /// Get established connections only
    pub fn established(&self) -> impl Iterator<Item = &TcpConnection> {
        self.by_state(TcpState::Established)
    }

    /// Get listening sockets only
    pub fn listening(&self) -> impl Iterator<Item = &TcpConnection> {
        self.by_state(TcpState::Listen)
    }

    /// Total connection count
    pub fn total(&self) -> usize {
        self.connections.len()
    }
}

/// Analyzer for network connections
pub struct ConnectionsAnalyzer {
    data: ConnectionsData,
    interval: Duration,
    /// Cache of inode -> (pid, name) mappings
    inode_cache: HashMap<u64, (u32, String)>,
}

impl Default for ConnectionsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionsAnalyzer {
    /// Create a new connections analyzer
    pub fn new() -> Self {
        Self {
            data: ConnectionsData::default(),
            interval: Duration::from_secs(2),
            inode_cache: HashMap::new(),
        }
    }

    /// Get the current connections data
    pub fn data(&self) -> &ConnectionsData {
        &self.data
    }

    /// Parse /proc/net/tcp or /proc/net/tcp6
    fn parse_tcp_file(
        &mut self,
        path: &str,
        is_ipv6: bool,
    ) -> Result<Vec<TcpConnection>, AnalyzerError> {
        let contents = fs::read_to_string(path)
            .map_err(|e| AnalyzerError::IoError(format!("Failed to read {}: {}", path, e)))?;

        let mut connections = Vec::new();

        for line in contents.lines().skip(1) {
            // Skip header
            if let Some(conn) = self.parse_tcp_line(line, is_ipv6) {
                connections.push(conn);
            }
        }

        Ok(connections)
    }

    /// Parse a single line from /proc/net/tcp
    fn parse_tcp_line(&self, line: &str, is_ipv6: bool) -> Option<TcpConnection> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            return None;
        }

        // local_address: ip:port in hex
        let (local_addr, local_port) = Self::parse_address(parts[1], is_ipv6)?;
        let (remote_addr, remote_port) = Self::parse_address(parts[2], is_ipv6)?;

        let state = TcpState::from_hex(parts[3]);
        let uid: u32 = parts[7].parse().ok()?;
        let inode: u64 = parts[9].parse().ok()?;

        // Try to resolve process from cache
        let (pid, process_name) = self.inode_cache.get(&inode).cloned().unzip();

        Some(TcpConnection {
            local_addr,
            local_port,
            remote_addr,
            remote_port,
            state,
            inode,
            uid,
            pid,
            process_name,
            is_ipv6,
        })
    }

    /// Parse hex address:port
    fn parse_address(s: &str, is_ipv6: bool) -> Option<(IpAddr, u16)> {
        let mut parts = s.split(':');
        let addr_hex = parts.next()?;
        let port_hex = parts.next()?;

        let port = u16::from_str_radix(port_hex, 16).ok()?;

        let addr = if is_ipv6 {
            Self::parse_ipv6(addr_hex)?
        } else {
            Self::parse_ipv4(addr_hex)?
        };

        Some((addr, port))
    }

    /// Parse IPv4 from hex (little-endian)
    fn parse_ipv4(hex: &str) -> Option<IpAddr> {
        if hex.len() != 8 {
            return None;
        }
        let bytes = u32::from_str_radix(hex, 16).ok()?;
        // Network byte order is big-endian, but /proc shows little-endian
        Some(IpAddr::V4(Ipv4Addr::from(bytes.to_be())))
    }

    /// Parse IPv6 from hex
    fn parse_ipv6(hex: &str) -> Option<IpAddr> {
        if hex.len() != 32 {
            return None;
        }

        // IPv6 in /proc is stored as 4 32-bit words in little-endian
        let mut segments = [0u16; 8];
        for i in 0..4 {
            let word_hex = &hex[i * 8..(i + 1) * 8];
            let word = u32::from_str_radix(word_hex, 16).ok()?.to_be();
            segments[i * 2] = (word >> 16) as u16;
            segments[i * 2 + 1] = word as u16;
        }

        Some(IpAddr::V6(Ipv6Addr::from(segments)))
    }

    /// Build inode to process mapping from /proc/[pid]/fd/
    fn build_inode_cache(&mut self) {
        self.inode_cache.clear();

        let proc_path = Path::new("/proc");
        let Ok(entries) = fs::read_dir(proc_path) else {
            return;
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Only process numeric directories (PIDs)
            let Ok(pid) = name_str.parse::<u32>() else {
                continue;
            };

            // Read process name
            let pid_path = proc_path.join(name_str.as_ref());
            let comm_path = pid_path.join("comm");
            let process_name = fs::read_to_string(&comm_path)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            // Scan fd directory for socket inodes
            let fd_path = pid_path.join("fd");
            let Ok(fd_entries) = fs::read_dir(&fd_path) else {
                continue;
            };

            for fd_entry in fd_entries.flatten() {
                let link = fd_entry.path();
                let Ok(target) = fs::read_link(&link) else {
                    continue;
                };

                let target_str = target.to_string_lossy();
                // Socket links look like: socket:[12345]
                if target_str.starts_with("socket:[") {
                    if let Some(inode_str) = target_str
                        .strip_prefix("socket:[")
                        .and_then(|s| s.strip_suffix(']'))
                    {
                        if let Ok(inode) = inode_str.parse::<u64>() {
                            self.inode_cache.insert(inode, (pid, process_name.clone()));
                        }
                    }
                }
            }
        }
    }

    /// Resolve process info for a connection
    fn resolve_process(&self, conn: &mut TcpConnection) {
        if let Some((pid, name)) = self.inode_cache.get(&conn.inode) {
            conn.pid = Some(*pid);
            conn.process_name = Some(name.clone());
        }
    }
}

impl Analyzer for ConnectionsAnalyzer {
    fn name(&self) -> &'static str {
        "connections"
    }

    fn collect(&mut self) -> Result<(), AnalyzerError> {
        // First, build the inode cache
        self.build_inode_cache();

        let mut all_connections = Vec::new();

        // Parse IPv4 connections
        if Path::new("/proc/net/tcp").exists() {
            if let Ok(conns) = self.parse_tcp_file("/proc/net/tcp", false) {
                all_connections.extend(conns);
            }
        }

        // Parse IPv6 connections
        if Path::new("/proc/net/tcp6").exists() {
            if let Ok(conns) = self.parse_tcp_file("/proc/net/tcp6", true) {
                all_connections.extend(conns);
            }
        }

        // Resolve process info for each connection
        for conn in &mut all_connections {
            self.resolve_process(conn);
        }

        // Count by state
        let mut state_counts: HashMap<TcpState, usize> = HashMap::new();
        for conn in &all_connections {
            *state_counts.entry(conn.state).or_insert(0) += 1;
        }

        self.data = ConnectionsData {
            connections: all_connections,
            state_counts,
        };

        Ok(())
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    fn available(&self) -> bool {
        Path::new("/proc/net/tcp").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_state_parsing() {
        assert_eq!(TcpState::from_hex("01"), TcpState::Established);
        assert_eq!(TcpState::from_hex("0A"), TcpState::Listen);
        assert_eq!(TcpState::from_hex("06"), TcpState::TimeWait);
        assert_eq!(TcpState::from_hex("FF"), TcpState::Unknown);
    }

    #[test]
    fn test_ipv4_parsing() {
        // 127.0.0.1 in little-endian hex is 0100007F
        let addr = ConnectionsAnalyzer::parse_ipv4("0100007F");
        assert!(addr.is_some());
        if let Some(IpAddr::V4(v4)) = addr {
            assert_eq!(v4, Ipv4Addr::new(127, 0, 0, 1));
        }

        // 0.0.0.0
        let addr = ConnectionsAnalyzer::parse_ipv4("00000000");
        assert!(addr.is_some());
        if let Some(IpAddr::V4(v4)) = addr {
            assert_eq!(v4, Ipv4Addr::new(0, 0, 0, 0));
        }
    }

    #[test]
    fn test_address_parsing() {
        // 127.0.0.1:53 (port 0x0035 = 53)
        let result = ConnectionsAnalyzer::parse_address("0100007F:0035", false);
        assert!(result.is_some());
        let (addr, port) = result.unwrap();
        assert_eq!(port, 53);
        if let IpAddr::V4(v4) = addr {
            assert_eq!(v4, Ipv4Addr::new(127, 0, 0, 1));
        }
    }

    #[test]
    fn test_analyzer_available() {
        let analyzer = ConnectionsAnalyzer::new();
        // Should be available on Linux
        #[cfg(target_os = "linux")]
        assert!(analyzer.available());
    }

    #[test]
    fn test_analyzer_collect() {
        let mut analyzer = ConnectionsAnalyzer::new();
        let result = analyzer.collect();

        // Should not fail even if we don't have access
        assert!(result.is_ok());

        // On Linux, we should have some connections
        #[cfg(target_os = "linux")]
        {
            let data = analyzer.data();
            // At least the test process should have sockets
            // (but we might not have permission to see them)
            let _ = data.total();
        }
    }

    #[test]
    fn test_tcp_state_display() {
        assert_eq!(TcpState::Established.as_str(), "ESTABLISHED");
        assert_eq!(TcpState::Listen.short(), "LISTEN");
        assert_eq!(TcpState::TimeWait.short(), "TIME_W");
    }

    #[test]
    fn test_connection_display() {
        let conn = TcpConnection {
            local_addr: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            local_port: 8080,
            remote_addr: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            remote_port: 443,
            state: TcpState::Established,
            inode: 12345,
            uid: 1000,
            pid: Some(1234),
            process_name: Some("firefox".to_string()),
            is_ipv6: false,
        };

        assert_eq!(conn.local_display(), "127.0.0.1:8080");
        assert_eq!(conn.remote_display(), "192.168.1.1:443");
        assert_eq!(conn.process_display(), "1234/firefox");
    }
}
