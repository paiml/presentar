//! UX Utilities for presentar widgets.
//!
//! Implements requirements from lltop UX falsification checklist:
//! - UX-001: Text truncation with ellipsis
//! - UX-002: Health status indicators
//! - UX-003: Empty state widget

use std::borrow::Cow;

// ============================================================================
// UX-001: Text Truncation
// ============================================================================

/// Truncate text with ellipsis when it exceeds max characters.
///
/// # Examples
/// ```
/// use presentar_terminal::widgets::truncate;
/// assert_eq!(truncate("Hello World", 8), "Hello W…");
/// assert_eq!(truncate("Short", 10), "Short");
/// ```
#[inline]
pub fn truncate(s: &str, max: usize) -> Cow<'_, str> {
    let char_count = s.chars().count();
    if char_count <= max {
        Cow::Borrowed(s)
    } else if max == 0 {
        Cow::Borrowed("")
    } else if max == 1 {
        Cow::Borrowed("…")
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        Cow::Owned(format!("{truncated}…"))
    }
}

/// Truncate text from the middle, preserving start and end.
///
/// Useful for file paths: `/home/user/very/long/path` -> `/hom…ng/path`
///
/// # Examples
/// ```
/// use presentar_terminal::widgets::truncate_middle;
/// // 25 char input, max 15: start=4 "/hom", end=10 "ects/myapp"
/// assert_eq!(truncate_middle("/home/user/projects/myapp", 15), "/hom…ects/myapp");
/// ```
pub fn truncate_middle(s: &str, max: usize) -> Cow<'_, str> {
    let char_count = s.chars().count();
    if char_count <= max {
        return Cow::Borrowed(s);
    }
    if max <= 3 {
        return truncate(s, max);
    }

    // Split: keep more of the end (filename usually more important)
    let start_len = (max - 1) / 3; // ~1/3 for start
    let end_len = max - 1 - start_len; // ~2/3 for end

    let start: String = s.chars().take(start_len).collect();
    let end: String = s.chars().skip(char_count - end_len).collect();

    Cow::Owned(format!("{start}…{end}"))
}

/// Truncate text with custom ellipsis string.
pub fn truncate_with<'a>(s: &'a str, max: usize, ellipsis: &str) -> Cow<'a, str> {
    let char_count = s.chars().count();
    let ellipsis_len = ellipsis.chars().count();

    if char_count <= max {
        Cow::Borrowed(s)
    } else if max <= ellipsis_len {
        Cow::Owned(ellipsis.chars().take(max).collect())
    } else {
        let truncated: String = s.chars().take(max - ellipsis_len).collect();
        Cow::Owned(format!("{truncated}{ellipsis}"))
    }
}

// ============================================================================
// UX-002: Health Status Indicators
// ============================================================================

/// Health status for visual indicators.
///
/// Uses distinct Unicode symbols for accessibility:
/// - Healthy: ✓ (check mark)
/// - Warning: ⚠ (warning sign)
/// - Critical: ✗ (x mark)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HealthStatus {
    /// System is healthy - displays ✓
    Healthy,
    /// System has warnings - displays ⚠
    Warning,
    /// System is critical - displays ✗
    Critical,
    /// Status unknown - displays ?
    Unknown,
}

impl HealthStatus {
    /// Get the Unicode symbol for this status.
    #[inline]
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Healthy => "✓",
            Self::Warning => "⚠",
            Self::Critical => "✗",
            Self::Unknown => "?",
        }
    }

    /// Get a colored symbol (ANSI escape codes).
    /// Returns symbol with appropriate color prefix.
    pub fn colored_symbol(&self) -> &'static str {
        match self {
            Self::Healthy => "\x1b[32m✓\x1b[0m",  // Green
            Self::Warning => "\x1b[33m⚠\x1b[0m",  // Yellow
            Self::Critical => "\x1b[31m✗\x1b[0m", // Red
            Self::Unknown => "\x1b[90m?\x1b[0m",  // Gray
        }
    }

    /// Get the label for this status.
    #[inline]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Healthy => "Healthy",
            Self::Warning => "Warning",
            Self::Critical => "Critical",
            Self::Unknown => "Unknown",
        }
    }

    /// Create from a percentage (0-100).
    /// - >= 80%: Healthy
    /// - >= 50%: Warning
    /// - < 50%: Critical
    pub fn from_percentage(pct: f64) -> Self {
        if pct >= 80.0 {
            Self::Healthy
        } else if pct >= 50.0 {
            Self::Warning
        } else {
            Self::Critical
        }
    }

    /// Create from a score and maximum.
    pub fn from_score(score: u32, max: u32) -> Self {
        if max == 0 {
            return Self::Unknown;
        }
        let pct = (score as f64 / max as f64) * 100.0;
        Self::from_percentage(pct)
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

// ============================================================================
// UX-003: Empty State Widget
// ============================================================================

/// Empty state display for panes with no data.
///
/// Renders a centered message with:
/// - Optional icon (emoji or Unicode)
/// - Title text
/// - Action hint (how to get data)
///
/// # Example
/// ```
/// use presentar_terminal::widgets::EmptyState;
///
/// let empty = EmptyState::new("No traces available")
///     .icon("📊")
///     .hint("Enable tracing with --trace flag");
/// ```
#[derive(Debug, Clone)]
pub struct EmptyState {
    /// Icon to display (emoji or Unicode symbol)
    pub icon: Option<String>,
    /// Main message title
    pub title: String,
    /// Action hint for user
    pub hint: Option<String>,
    /// Whether to center vertically
    pub center_vertical: bool,
}

impl EmptyState {
    /// Create a new empty state with title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            icon: None,
            title: title.into(),
            hint: None,
            center_vertical: true,
        }
    }

    /// Add an icon.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Add an action hint.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Disable vertical centering.
    pub fn top_aligned(mut self) -> Self {
        self.center_vertical = false;
        self
    }

    /// Render to lines for display.
    ///
    /// Returns lines that should be rendered, with the starting y offset
    /// for vertical centering.
    pub fn render_lines(&self, available_height: u16) -> (Vec<String>, u16) {
        let mut lines = Vec::new();

        // Add icon line
        if let Some(ref icon) = self.icon {
            lines.push(icon.clone());
            lines.push(String::new()); // Spacer
        }

        // Add title
        lines.push(self.title.clone());

        // Add hint
        if let Some(ref hint) = self.hint {
            lines.push(String::new()); // Spacer
            lines.push(hint.clone());
        }

        // Calculate y offset for centering
        let y_offset = if self.center_vertical {
            let content_height = lines.len() as u16;
            if available_height > content_height {
                (available_height - content_height) / 2
            } else {
                0
            }
        } else {
            1 // Small top margin
        };

        (lines, y_offset)
    }
}

impl Default for EmptyState {
    fn default() -> Self {
        Self::new("No data available")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("Hello", 10), "Hello");
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(truncate("Hello", 5), "Hello");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("Hello World", 8), "Hello W…");
        assert_eq!(truncate("Hello World", 6), "Hello…");
        assert_eq!(truncate("Hello World", 1), "…");
        assert_eq!(truncate("Hello World", 0), "");
    }

    #[test]
    fn test_truncate_middle() {
        assert_eq!(truncate_middle("/home/user/path", 20), "/home/user/path");
        // 28 char input -> max 15: start=4 "/hom", end=10 "th/file.rs"
        assert_eq!(
            truncate_middle("/home/user/long/path/file.rs", 15),
            "/hom…th/file.rs"
        );
    }

    #[test]
    fn test_health_status_symbol() {
        assert_eq!(HealthStatus::Healthy.symbol(), "✓");
        assert_eq!(HealthStatus::Warning.symbol(), "⚠");
        assert_eq!(HealthStatus::Critical.symbol(), "✗");
        assert_eq!(HealthStatus::Unknown.symbol(), "?");
    }

    #[test]
    fn test_health_from_percentage() {
        assert_eq!(HealthStatus::from_percentage(100.0), HealthStatus::Healthy);
        assert_eq!(HealthStatus::from_percentage(80.0), HealthStatus::Healthy);
        assert_eq!(HealthStatus::from_percentage(79.0), HealthStatus::Warning);
        assert_eq!(HealthStatus::from_percentage(50.0), HealthStatus::Warning);
        assert_eq!(HealthStatus::from_percentage(49.0), HealthStatus::Critical);
        assert_eq!(HealthStatus::from_percentage(0.0), HealthStatus::Critical);
    }

    #[test]
    fn test_health_from_score() {
        assert_eq!(HealthStatus::from_score(20, 20), HealthStatus::Healthy);
        assert_eq!(HealthStatus::from_score(16, 20), HealthStatus::Healthy);
        assert_eq!(HealthStatus::from_score(15, 20), HealthStatus::Warning);
        assert_eq!(HealthStatus::from_score(10, 20), HealthStatus::Warning);
        assert_eq!(HealthStatus::from_score(9, 20), HealthStatus::Critical);
        assert_eq!(HealthStatus::from_score(0, 0), HealthStatus::Unknown);
    }

    #[test]
    fn test_empty_state_render() {
        let empty = EmptyState::new("No data").icon("📊").hint("Try refreshing");

        let (lines, offset) = empty.render_lines(20);
        assert_eq!(lines.len(), 5); // icon, spacer, title, spacer, hint
        assert!(offset > 0); // Should be centered
    }

    #[test]
    fn test_empty_state_top_aligned() {
        let empty = EmptyState::new("No data").top_aligned();
        let (_, offset) = empty.render_lines(20);
        assert_eq!(offset, 1);
    }
}
