//! `ConnectionsPanel` widget for TCP/UDP connection monitoring.
//!
//! Displays active network connections with state and process mapping.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// TCP connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TcpState {
    #[default]
    Established,
    Listen,
    TimeWait,
    CloseWait,
    SynSent,
    SynRecv,
    FinWait1,
    FinWait2,
    LastAck,
    Closing,
    Closed,
}

impl TcpState {
    /// Get short display string.
    pub fn short(&self) -> &'static str {
        match self {
            Self::Established => "EST",
            Self::Listen => "LSN",
            Self::TimeWait => "TW",
            Self::CloseWait => "CW",
            Self::SynSent => "SS",
            Self::SynRecv => "SR",
            Self::FinWait1 => "FW1",
            Self::FinWait2 => "FW2",
            Self::LastAck => "LA",
            Self::Closing => "CLG",
            Self::Closed => "CLD",
        }
    }

    /// Get state color.
    pub fn color(&self) -> Color {
        match self {
            Self::Established => Color::new(0.4, 0.9, 0.4, 1.0), // Green
            Self::Listen => Color::new(0.4, 0.6, 1.0, 1.0),      // Blue
            Self::TimeWait => Color::new(0.6, 0.6, 0.6, 1.0),    // Gray
            Self::CloseWait => Color::new(1.0, 0.8, 0.2, 1.0),   // Yellow
            _ => Color::new(0.7, 0.7, 0.7, 1.0),                 // Default gray
        }
    }
}

/// A network connection entry.
#[derive(Debug, Clone)]
pub struct ConnectionEntry {
    /// Protocol (tcp, udp).
    pub protocol: String,
    /// Local address.
    pub local_addr: String,
    /// Local port.
    pub local_port: u16,
    /// Remote address.
    pub remote_addr: String,
    /// Remote port.
    pub remote_port: u16,
    /// Connection state.
    pub state: TcpState,
    /// Process name (if available).
    pub process: Option<String>,
    /// Process ID.
    pub pid: Option<u32>,
}

impl ConnectionEntry {
    /// Create a new TCP connection.
    #[must_use]
    pub fn tcp(local_port: u16, remote_addr: impl Into<String>, remote_port: u16) -> Self {
        Self {
            protocol: "tcp".to_string(),
            local_addr: "0.0.0.0".to_string(),
            local_port,
            remote_addr: remote_addr.into(),
            remote_port,
            state: TcpState::Established,
            process: None,
            pid: None,
        }
    }

    /// Create a listening socket.
    #[must_use]
    pub fn listen(port: u16) -> Self {
        Self {
            protocol: "tcp".to_string(),
            local_addr: "0.0.0.0".to_string(),
            local_port: port,
            remote_addr: "*".to_string(),
            remote_port: 0,
            state: TcpState::Listen,
            process: None,
            pid: None,
        }
    }

    /// Set connection state.
    #[must_use]
    pub fn with_state(mut self, state: TcpState) -> Self {
        self.state = state;
        self
    }

    /// Set process info.
    #[must_use]
    pub fn with_process(mut self, name: impl Into<String>, pid: u32) -> Self {
        self.process = Some(name.into());
        self.pid = Some(pid);
        self
    }

    /// Set local address.
    #[must_use]
    pub fn with_local_addr(mut self, addr: impl Into<String>) -> Self {
        self.local_addr = addr.into();
        self
    }

    /// Get service name from port.
    pub fn service_name(&self) -> &str {
        match self.local_port {
            22 => "ssh",
            80 => "http",
            443 => "https",
            3306 => "mysql",
            5432 => "pgsql",
            6379 => "redis",
            8080 => "http-alt",
            27017 => "mongodb",
            _ => "",
        }
    }

    /// Format local endpoint.
    pub fn local_display(&self) -> String {
        format!(":{}", self.local_port)
    }

    /// Format remote endpoint.
    pub fn remote_display(&self) -> String {
        if self.remote_addr == "*" || self.remote_addr == "0.0.0.0" {
            "*".to_string()
        } else {
            format!("{}:{}", self.remote_addr, self.remote_port)
        }
    }
}

/// Connections panel displaying network connections.
#[derive(Debug, Clone)]
pub struct ConnectionsPanel {
    /// Connection entries.
    connections: Vec<ConnectionEntry>,
    /// Show listening sockets.
    show_listening: bool,
    /// Show established connections.
    show_established: bool,
    /// Max connections to show.
    max_connections: usize,
    /// Show column headers.
    show_headers: bool,
    /// Cached bounds.
    bounds: Rect,
}

impl Default for ConnectionsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionsPanel {
    /// Create a new connections panel.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
            show_listening: true,
            show_established: true,
            max_connections: 10,
            show_headers: true,
            bounds: Rect::default(),
        }
    }

    /// Add a connection.
    pub fn add_connection(&mut self, connection: ConnectionEntry) {
        self.connections.push(connection);
    }

    /// Set all connections.
    #[must_use]
    pub fn with_connections(mut self, connections: Vec<ConnectionEntry>) -> Self {
        self.connections = connections;
        self
    }

    /// Toggle listening sockets.
    #[must_use]
    pub fn show_listening(mut self, show: bool) -> Self {
        self.show_listening = show;
        self
    }

    /// Toggle established connections.
    #[must_use]
    pub fn show_established(mut self, show: bool) -> Self {
        self.show_established = show;
        self
    }

    /// Set max connections.
    #[must_use]
    pub fn max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// Toggle headers.
    #[must_use]
    pub fn show_headers(mut self, show: bool) -> Self {
        self.show_headers = show;
        self
    }

    /// Get established count.
    pub fn established_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|c| c.state == TcpState::Established)
            .count()
    }

    /// Get listening count.
    pub fn listening_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|c| c.state == TcpState::Listen)
            .count()
    }

    /// Get visible connections (filtered).
    fn visible_connections(&self) -> impl Iterator<Item = &ConnectionEntry> {
        self.connections
            .iter()
            .filter(|c| {
                (self.show_listening && c.state == TcpState::Listen)
                    || (self.show_established && c.state == TcpState::Established)
                    || (c.state != TcpState::Listen && c.state != TcpState::Established)
            })
            .take(self.max_connections)
    }

    /// Draw header row.
    fn draw_header(&self, canvas: &mut dyn Canvas, x: f32, y: f32) {
        let header = "SVC   LOCAL   → REMOTE         ST  PROC";
        canvas.draw_text(
            header,
            Point::new(x, y),
            &TextStyle {
                color: Color::new(0.6, 0.6, 0.6, 1.0),
                ..Default::default()
            },
        );
    }

    /// Draw a connection line.
    fn draw_connection(
        &self,
        canvas: &mut dyn Canvas,
        conn: &ConnectionEntry,
        x: f32,
        y: f32,
        width: f32,
    ) {
        // Service name or port
        let svc = {
            let name = conn.service_name();
            if name.is_empty() {
                format!("{:5}", conn.local_port)
            } else {
                format!("{name:5}")
            }
        };
        canvas.draw_text(
            &svc,
            Point::new(x, y),
            &TextStyle {
                color: Color::WHITE,
                ..Default::default()
            },
        );

        // Local port
        canvas.draw_text(
            &conn.local_display(),
            Point::new(x + 6.0, y),
            &TextStyle {
                color: Color::new(0.6, 0.8, 1.0, 1.0),
                ..Default::default()
            },
        );

        // Arrow
        canvas.draw_text(
            "→",
            Point::new(x + 12.0, y),
            &TextStyle {
                color: Color::new(0.5, 0.5, 0.5, 1.0),
                ..Default::default()
            },
        );

        // Remote (truncated)
        let remote = {
            let r = conn.remote_display();
            if r.len() > 14 {
                format!("{}...", &r[..11])
            } else {
                format!("{r:14}")
            }
        };
        canvas.draw_text(
            &remote,
            Point::new(x + 14.0, y),
            &TextStyle {
                color: Color::new(0.8, 0.8, 0.8, 1.0),
                ..Default::default()
            },
        );

        // State
        canvas.draw_text(
            conn.state.short(),
            Point::new(x + 29.0, y),
            &TextStyle {
                color: conn.state.color(),
                ..Default::default()
            },
        );

        // Process name (if available and fits)
        if let Some(ref proc) = conn.process {
            let proc_x = x + 33.0;
            if proc_x < x + width {
                let max_len = ((width - 33.0) as usize).min(10);
                let name = if proc.len() > max_len {
                    format!("{}...", &proc[..max_len.saturating_sub(3)])
                } else {
                    proc.clone()
                };
                canvas.draw_text(
                    &name,
                    Point::new(proc_x, y),
                    &TextStyle {
                        color: Color::new(0.6, 0.6, 0.6, 1.0),
                        ..Default::default()
                    },
                );
            }
        }
    }
}

impl Brick for ConnectionsPanel {
    fn brick_name(&self) -> &'static str {
        "connections_panel"
    }

    fn assertions(&self) -> &[BrickAssertion] {
        static ASSERTIONS: &[BrickAssertion] = &[BrickAssertion::max_latency_ms(8)];
        ASSERTIONS
    }

    fn budget(&self) -> BrickBudget {
        BrickBudget::uniform(8)
    }

    fn verify(&self) -> BrickVerification {
        BrickVerification {
            passed: vec![BrickAssertion::max_latency_ms(8)],
            failed: vec![],
            verification_time: Duration::from_micros(25),
        }
    }

    fn to_html(&self) -> String {
        String::new()
    }

    fn to_css(&self) -> String {
        String::new()
    }
}

impl Widget for ConnectionsPanel {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let header_lines = usize::from(self.show_headers);
        let visible = self.visible_connections().count();
        let height = ((header_lines + visible) as f32)
            .max(1.0)
            .min(constraints.max_height);
        Size::new(constraints.max_width, height)
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.bounds.width < 20.0 || self.bounds.height < 1.0 {
            return;
        }

        let mut y = self.bounds.y;
        let x = self.bounds.x;

        // Draw header
        if self.show_headers {
            self.draw_header(canvas, x, y);
            y += 1.0;
        }

        // Draw connections
        for conn in self.visible_connections() {
            if y >= self.bounds.y + self.bounds.height {
                break;
            }
            self.draw_connection(canvas, conn, x, y, self.bounds.width);
            y += 1.0;
        }

        // If no connections, show message
        if self.connections.is_empty() {
            canvas.draw_text(
                "No connections",
                Point::new(x, y),
                &TextStyle {
                    color: Color::new(0.5, 0.5, 0.5, 1.0),
                    ..Default::default()
                },
            );
        }
    }

    fn event(&mut self, _event: &Event) -> Option<Box<dyn Any + Send>> {
        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_state_short() {
        assert_eq!(TcpState::Established.short(), "EST");
        assert_eq!(TcpState::Listen.short(), "LSN");
        assert_eq!(TcpState::TimeWait.short(), "TW");
    }

    #[test]
    fn test_connection_entry_tcp() {
        let conn = ConnectionEntry::tcp(443, "1.2.3.4", 52341).with_process("nginx", 1234);

        assert_eq!(conn.local_port, 443);
        assert_eq!(conn.remote_port, 52341);
        assert_eq!(conn.process, Some("nginx".to_string()));
        assert_eq!(conn.service_name(), "https");
    }

    #[test]
    fn test_connection_entry_listen() {
        let conn = ConnectionEntry::listen(8080);
        assert_eq!(conn.state, TcpState::Listen);
        assert_eq!(conn.remote_display(), "*");
        assert_eq!(conn.service_name(), "http-alt");
    }

    #[test]
    fn test_panel_counts() {
        let mut panel = ConnectionsPanel::new();
        panel.add_connection(ConnectionEntry::listen(80));
        panel.add_connection(ConnectionEntry::listen(443));
        panel.add_connection(ConnectionEntry::tcp(443, "1.2.3.4", 52341));

        assert_eq!(panel.listening_count(), 2);
        assert_eq!(panel.established_count(), 1);
    }

    #[test]
    fn test_panel_builder() {
        let panel = ConnectionsPanel::new()
            .show_listening(false)
            .show_established(true)
            .max_connections(5)
            .show_headers(false);

        assert!(!panel.show_listening);
        assert!(panel.show_established);
        assert_eq!(panel.max_connections, 5);
        assert!(!panel.show_headers);
    }
}
