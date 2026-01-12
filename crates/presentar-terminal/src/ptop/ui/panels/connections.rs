//! Connections panel rendering and utilities.
//!
//! Provides connections panel title building, connection formatting,
//! and helper functions for rendering network connection metrics.

use presentar_core::Color;

// =============================================================================
// CONNECTIONS TITLE BUILDING
// =============================================================================

/// Build connections panel title string.
///
/// Format: "Connections │ 45 │ ESTAB: 30 │ LISTEN: 5"
#[must_use]
pub fn build_connections_title(total: usize, established: usize, listening: usize) -> String {
    format!(
        "Connections │ {} │ ESTAB: {} │ LISTEN: {}",
        total, established, listening
    )
}

/// Build compact connections title for narrow panels.
///
/// Format: "Conn │ 45"
#[must_use]
pub fn build_connections_title_compact(total: usize) -> String {
    format!("Conn │ {}", total)
}

// =============================================================================
// CONNECTION STATE
// =============================================================================

/// TCP connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TcpConnectionState {
    /// Connection established
    #[default]
    Established,
    /// Listening for connections
    Listen,
    /// Connection is in TIME_WAIT state
    TimeWait,
    /// Connection is in CLOSE_WAIT state
    CloseWait,
    /// SYN sent, waiting for response
    SynSent,
    /// SYN received, responding
    SynRecv,
    /// FIN_WAIT_1 state
    FinWait1,
    /// FIN_WAIT_2 state
    FinWait2,
    /// CLOSING state
    Closing,
    /// LAST_ACK state
    LastAck,
    /// Connection closed
    Closed,
}

impl TcpConnectionState {
    /// Get display name for connection state.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Established => "ESTABLISHED",
            Self::Listen => "LISTEN",
            Self::TimeWait => "TIME_WAIT",
            Self::CloseWait => "CLOSE_WAIT",
            Self::SynSent => "SYN_SENT",
            Self::SynRecv => "SYN_RECV",
            Self::FinWait1 => "FIN_WAIT_1",
            Self::FinWait2 => "FIN_WAIT_2",
            Self::Closing => "CLOSING",
            Self::LastAck => "LAST_ACK",
            Self::Closed => "CLOSED",
        }
    }

    /// Get short display name.
    #[must_use]
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Established => "ESTAB",
            Self::Listen => "LSTN",
            Self::TimeWait => "TIME_W",
            Self::CloseWait => "CLOSE_W",
            Self::SynSent => "SYN_S",
            Self::SynRecv => "SYN_R",
            Self::FinWait1 => "FIN_W1",
            Self::FinWait2 => "FIN_W2",
            Self::Closing => "CLOSE",
            Self::LastAck => "LAST_A",
            Self::Closed => "CLOSED",
        }
    }

    /// Get color for connection state.
    #[must_use]
    pub fn color(&self) -> Color {
        match self {
            Self::Established => Color::new(0.3, 0.9, 0.4, 1.0), // Green
            Self::Listen => Color::new(0.4, 0.7, 1.0, 1.0),      // Blue
            Self::TimeWait | Self::CloseWait => Color::new(1.0, 0.8, 0.3, 1.0), // Yellow
            Self::SynSent | Self::SynRecv => Color::new(0.9, 0.6, 0.3, 1.0), // Orange
            Self::FinWait1 | Self::FinWait2 | Self::Closing => Color::new(0.7, 0.5, 0.3, 1.0), // Brown
            Self::LastAck => Color::new(0.6, 0.4, 0.4, 1.0), // Dark red
            Self::Closed => Color::new(0.5, 0.5, 0.5, 1.0),  // Gray
        }
    }

    /// Check if connection is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Established | Self::Listen)
    }

    /// Check if connection is closing.
    #[must_use]
    pub fn is_closing(&self) -> bool {
        matches!(
            self,
            Self::TimeWait
                | Self::CloseWait
                | Self::FinWait1
                | Self::FinWait2
                | Self::Closing
                | Self::LastAck
        )
    }
}

// =============================================================================
// PORT/SERVICE MAPPING
// =============================================================================

/// Map well-known port to service name.
#[must_use]
pub fn port_to_service_name(port: u16) -> &'static str {
    match port {
        20 => "FTP-D",
        21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        25 => "SMTP",
        53 => "DNS",
        80 => "HTTP",
        110 => "POP3",
        119 => "NNTP",
        123 => "NTP",
        143 => "IMAP",
        161 => "SNMP",
        443 => "HTTPS",
        445 => "SMB",
        465 => "SMTPS",
        587 => "Subm",
        993 => "IMAPS",
        995 => "POP3S",
        1433 => "MSSQL",
        3306 => "MySQL",
        3389 => "RDP",
        5432 => "PgSQL",
        5900..=5999 => "VNC",
        6379 => "Redis",
        8080 => "Alt-HTTP",
        8443 => "Alt-HTTPS",
        9000..=9099 => "App",
        27017 => "MongoDB",
        _ => "",
    }
}

/// Check if port is a well-known service port.
#[must_use]
pub fn is_well_known_port(port: u16) -> bool {
    port <= 1023
}

/// Check if port is ephemeral (dynamic).
#[must_use]
pub fn is_ephemeral_port(port: u16) -> bool {
    port >= 49152
}

// =============================================================================
// ADDRESS FORMATTING
// =============================================================================

/// Format IP address for display (truncate if needed).
#[must_use]
pub fn format_address(addr: &str, max_width: usize) -> String {
    if addr.len() <= max_width {
        return addr.to_string();
    }

    if max_width < 4 {
        return addr.chars().take(max_width).collect();
    }

    // Truncate with ellipsis
    let truncated: String = addr.chars().take(max_width - 1).collect();
    format!("{}~", truncated)
}

/// Format socket address (IP:port) for display.
#[must_use]
pub fn format_socket_address(ip: &str, port: u16, max_width: usize) -> String {
    let full = format!("{}:{}", ip, port);
    format_address(&full, max_width)
}

/// Abbreviate localhost addresses.
#[must_use]
pub fn abbreviate_localhost(addr: &str) -> &str {
    match addr {
        "127.0.0.1" => "localhost",
        "::1" => "localhost",
        "0.0.0.0" => "*",
        "::" => "*",
        _ => addr,
    }
}

// =============================================================================
// CONNECTION PROTOCOL
// =============================================================================

/// Network protocol type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Protocol {
    #[default]
    Tcp,
    Udp,
    Tcp6,
    Udp6,
}

impl Protocol {
    /// Get display name.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
            Self::Tcp6 => "TCP6",
            Self::Udp6 => "UDP6",
        }
    }

    /// Check if IPv6.
    #[must_use]
    pub fn is_ipv6(&self) -> bool {
        matches!(self, Self::Tcp6 | Self::Udp6)
    }

    /// Check if TCP.
    #[must_use]
    pub fn is_tcp(&self) -> bool {
        matches!(self, Self::Tcp | Self::Tcp6)
    }
}

// =============================================================================
// CONNECTION COLUMN WIDTHS
// =============================================================================

/// Column widths for connection display.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionColumnWidths {
    /// Protocol column width
    pub proto: usize,
    /// Local address column width
    pub local: usize,
    /// Remote address column width
    pub remote: usize,
    /// State column width
    pub state: usize,
    /// Process column width
    pub process: usize,
}

impl ConnectionColumnWidths {
    /// Calculate column widths for available width.
    #[must_use]
    pub fn calculate(available_width: usize) -> Self {
        const PROTO_WIDTH: usize = 5;
        const STATE_WIDTH: usize = 8;
        const FIXED: usize = PROTO_WIDTH + STATE_WIDTH;

        let remaining = available_width.saturating_sub(FIXED);
        let addr_width = remaining / 3;
        let process_width = remaining - (addr_width * 2);

        Self {
            proto: PROTO_WIDTH,
            local: addr_width.max(10),
            remote: addr_width.max(10),
            state: STATE_WIDTH,
            process: process_width.max(8),
        }
    }

    /// Get total width.
    #[must_use]
    pub fn total(&self) -> usize {
        self.proto + self.local + self.remote + self.state + self.process
    }
}

impl Default for ConnectionColumnWidths {
    fn default() -> Self {
        Self::calculate(80)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // build_connections_title tests
    // =========================================================================

    #[test]
    fn test_build_connections_title_basic() {
        let title = build_connections_title(45, 30, 5);
        assert!(title.contains("Connections"));
        assert!(title.contains("45"));
        assert!(title.contains("ESTAB: 30"));
        assert!(title.contains("LISTEN: 5"));
    }

    #[test]
    fn test_build_connections_title_zero() {
        let title = build_connections_title(0, 0, 0);
        assert!(title.contains("0"));
    }

    // =========================================================================
    // build_connections_title_compact tests
    // =========================================================================

    #[test]
    fn test_build_connections_title_compact_basic() {
        let title = build_connections_title_compact(45);
        assert!(title.contains("Conn"));
        assert!(title.contains("45"));
        assert!(!title.contains("ESTAB"));
    }

    // =========================================================================
    // TcpConnectionState tests
    // =========================================================================

    #[test]
    fn test_tcp_connection_state_display_name() {
        assert_eq!(TcpConnectionState::Established.display_name(), "ESTABLISHED");
        assert_eq!(TcpConnectionState::Listen.display_name(), "LISTEN");
        assert_eq!(TcpConnectionState::TimeWait.display_name(), "TIME_WAIT");
    }

    #[test]
    fn test_tcp_connection_state_short_name() {
        assert_eq!(TcpConnectionState::Established.short_name(), "ESTAB");
        assert_eq!(TcpConnectionState::Listen.short_name(), "LSTN");
    }

    #[test]
    fn test_tcp_connection_state_color() {
        let color = TcpConnectionState::Established.color();
        assert!(color.g > 0.8, "Established should be green");

        let color = TcpConnectionState::Listen.color();
        assert!(color.b > 0.9, "Listen should be blue");

        let color = TcpConnectionState::Closed.color();
        assert!((color.r - color.g).abs() < 0.1, "Closed should be gray");
    }

    #[test]
    fn test_tcp_connection_state_is_active() {
        assert!(TcpConnectionState::Established.is_active());
        assert!(TcpConnectionState::Listen.is_active());
        assert!(!TcpConnectionState::TimeWait.is_active());
        assert!(!TcpConnectionState::Closed.is_active());
    }

    #[test]
    fn test_tcp_connection_state_is_closing() {
        assert!(TcpConnectionState::TimeWait.is_closing());
        assert!(TcpConnectionState::CloseWait.is_closing());
        assert!(TcpConnectionState::FinWait1.is_closing());
        assert!(!TcpConnectionState::Established.is_closing());
    }

    #[test]
    fn test_tcp_connection_state_default() {
        assert_eq!(TcpConnectionState::default(), TcpConnectionState::Established);
    }

    #[test]
    fn test_tcp_connection_state_derive_debug() {
        let state = TcpConnectionState::Listen;
        let debug = format!("{:?}", state);
        assert!(debug.contains("Listen"));
    }

    // =========================================================================
    // port_to_service_name tests
    // =========================================================================

    #[test]
    fn test_port_to_service_name_common() {
        assert_eq!(port_to_service_name(22), "SSH");
        assert_eq!(port_to_service_name(80), "HTTP");
        assert_eq!(port_to_service_name(443), "HTTPS");
    }

    #[test]
    fn test_port_to_service_name_databases() {
        assert_eq!(port_to_service_name(3306), "MySQL");
        assert_eq!(port_to_service_name(5432), "PgSQL");
        assert_eq!(port_to_service_name(6379), "Redis");
        assert_eq!(port_to_service_name(27017), "MongoDB");
    }

    #[test]
    fn test_port_to_service_name_vnc_range() {
        assert_eq!(port_to_service_name(5900), "VNC");
        assert_eq!(port_to_service_name(5901), "VNC");
        assert_eq!(port_to_service_name(5999), "VNC");
    }

    #[test]
    fn test_port_to_service_name_unknown() {
        assert_eq!(port_to_service_name(12345), "");
        assert_eq!(port_to_service_name(0), "");
    }

    // =========================================================================
    // port classification tests
    // =========================================================================

    #[test]
    fn test_is_well_known_port() {
        assert!(is_well_known_port(80));
        assert!(is_well_known_port(443));
        assert!(is_well_known_port(1023));
        assert!(!is_well_known_port(1024));
        assert!(!is_well_known_port(8080));
    }

    #[test]
    fn test_is_ephemeral_port() {
        assert!(!is_ephemeral_port(80));
        assert!(!is_ephemeral_port(49151));
        assert!(is_ephemeral_port(49152));
        assert!(is_ephemeral_port(65535));
    }

    // =========================================================================
    // format_address tests
    // =========================================================================

    #[test]
    fn test_format_address_fits() {
        let result = format_address("192.168.1.1", 20);
        assert_eq!(result, "192.168.1.1");
    }

    #[test]
    fn test_format_address_truncates() {
        let result = format_address("192.168.1.100", 10);
        assert_eq!(result.len(), 10);
        assert!(result.ends_with('~'));
    }

    #[test]
    fn test_format_address_very_short() {
        let result = format_address("192.168.1.1", 3);
        assert_eq!(result, "192");
    }

    #[test]
    fn test_format_socket_address() {
        let result = format_socket_address("192.168.1.1", 8080, 20);
        assert!(result.contains("192.168.1.1:8080"));
    }

    // =========================================================================
    // abbreviate_localhost tests
    // =========================================================================

    #[test]
    fn test_abbreviate_localhost_ipv4() {
        assert_eq!(abbreviate_localhost("127.0.0.1"), "localhost");
    }

    #[test]
    fn test_abbreviate_localhost_ipv6() {
        assert_eq!(abbreviate_localhost("::1"), "localhost");
    }

    #[test]
    fn test_abbreviate_localhost_any() {
        assert_eq!(abbreviate_localhost("0.0.0.0"), "*");
        assert_eq!(abbreviate_localhost("::"), "*");
    }

    #[test]
    fn test_abbreviate_localhost_other() {
        assert_eq!(abbreviate_localhost("192.168.1.1"), "192.168.1.1");
    }

    // =========================================================================
    // Protocol tests
    // =========================================================================

    #[test]
    fn test_protocol_display_name() {
        assert_eq!(Protocol::Tcp.display_name(), "TCP");
        assert_eq!(Protocol::Udp.display_name(), "UDP");
        assert_eq!(Protocol::Tcp6.display_name(), "TCP6");
    }

    #[test]
    fn test_protocol_is_ipv6() {
        assert!(!Protocol::Tcp.is_ipv6());
        assert!(!Protocol::Udp.is_ipv6());
        assert!(Protocol::Tcp6.is_ipv6());
        assert!(Protocol::Udp6.is_ipv6());
    }

    #[test]
    fn test_protocol_is_tcp() {
        assert!(Protocol::Tcp.is_tcp());
        assert!(Protocol::Tcp6.is_tcp());
        assert!(!Protocol::Udp.is_tcp());
        assert!(!Protocol::Udp6.is_tcp());
    }

    #[test]
    fn test_protocol_default() {
        assert_eq!(Protocol::default(), Protocol::Tcp);
    }

    // =========================================================================
    // ConnectionColumnWidths tests
    // =========================================================================

    #[test]
    fn test_connection_column_widths_default() {
        let widths = ConnectionColumnWidths::default();
        assert_eq!(widths.proto, 5);
        assert_eq!(widths.state, 8);
    }

    #[test]
    fn test_connection_column_widths_calculate() {
        let widths = ConnectionColumnWidths::calculate(100);
        assert!(widths.total() <= 100);
    }

    #[test]
    fn test_connection_column_widths_narrow() {
        let widths = ConnectionColumnWidths::calculate(40);
        // Should have minimum widths
        assert!(widths.local >= 10);
        assert!(widths.remote >= 10);
    }

    #[test]
    fn test_connection_column_widths_derive_debug() {
        let widths = ConnectionColumnWidths::default();
        let debug = format!("{:?}", widths);
        assert!(debug.contains("ConnectionColumnWidths"));
    }

    #[test]
    fn test_connection_column_widths_derive_clone() {
        let widths = ConnectionColumnWidths::calculate(100);
        let cloned = widths.clone();
        assert_eq!(widths, cloned);
    }
}
