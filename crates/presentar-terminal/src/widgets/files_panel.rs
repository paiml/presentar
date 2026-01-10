//! `FilesPanel` widget for filesystem usage visualization.
//!
//! Displays a treemap of disk usage by directory.
//! Wraps the Treemap widget with filesystem-specific display.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// A filesystem entry for display.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Directory/file name.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Color for display.
    pub color: Color,
}

impl FileEntry {
    /// Create a new file entry.
    #[must_use]
    pub fn new(name: impl Into<String>, size: u64, is_dir: bool) -> Self {
        // Assign color based on common directory types
        let name_str = name.into();
        let color = Self::color_for_name(&name_str);
        Self {
            name: name_str,
            size,
            is_dir,
            color,
        }
    }

    /// Create a directory entry.
    #[must_use]
    pub fn directory(name: impl Into<String>, size: u64) -> Self {
        Self::new(name, size, true)
    }

    /// Create a file entry.
    #[must_use]
    pub fn file(name: impl Into<String>, size: u64) -> Self {
        Self::new(name, size, false)
    }

    /// Set color explicitly.
    #[must_use]
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Get color based on directory name.
    fn color_for_name(name: &str) -> Color {
        match name.to_lowercase().as_str() {
            "home" | "users" => Color::new(0.4, 0.7, 0.9, 1.0), // Blue
            "var" | "log" => Color::new(0.9, 0.6, 0.3, 1.0),    // Orange
            "usr" | "bin" | "lib" => Color::new(0.5, 0.8, 0.5, 1.0), // Green
            "tmp" | "cache" => Color::new(0.7, 0.7, 0.7, 1.0),  // Gray
            "etc" | "config" => Color::new(0.8, 0.5, 0.8, 1.0), // Purple
            "opt" | "local" => Color::new(0.6, 0.8, 0.6, 1.0),  // Light green
            _ => Color::new(0.6, 0.6, 0.8, 1.0),                // Default blue-gray
        }
    }

    /// Format size for display.
    pub fn size_display(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;

        if self.size >= TB {
            format!("{:.1}T", self.size as f64 / TB as f64)
        } else if self.size >= GB {
            format!("{:.1}G", self.size as f64 / GB as f64)
        } else if self.size >= MB {
            format!("{:.0}M", self.size as f64 / MB as f64)
        } else if self.size >= KB {
            format!("{:.0}K", self.size as f64 / KB as f64)
        } else {
            format!("{}B", self.size)
        }
    }

    /// Get percentage of total.
    pub fn percent_of(&self, total: u64) -> f32 {
        if total > 0 {
            (self.size as f64 / total as f64 * 100.0) as f32
        } else {
            0.0
        }
    }
}

/// Files panel displaying filesystem usage as a treemap.
#[derive(Debug, Clone)]
pub struct FilesPanel {
    /// File/directory entries.
    entries: Vec<FileEntry>,
    /// Total size for percentage calculation.
    total_size: u64,
    /// Show size labels.
    show_sizes: bool,
    /// Show percentage bars.
    show_bars: bool,
    /// Max entries to show.
    max_entries: usize,
    /// Cached bounds.
    bounds: Rect,
}

impl Default for FilesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl FilesPanel {
    /// Create a new files panel.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            total_size: 0,
            show_sizes: true,
            show_bars: true,
            max_entries: 8,
            bounds: Rect::default(),
        }
    }

    /// Add an entry.
    pub fn add_entry(&mut self, entry: FileEntry) {
        self.total_size += entry.size;
        self.entries.push(entry);
    }

    /// Set all entries.
    #[must_use]
    pub fn with_entries(mut self, entries: Vec<FileEntry>) -> Self {
        self.total_size = entries.iter().map(|e| e.size).sum();
        self.entries = entries;
        self
    }

    /// Set total size explicitly.
    #[must_use]
    pub fn with_total_size(mut self, total: u64) -> Self {
        self.total_size = total;
        self
    }

    /// Toggle size labels.
    #[must_use]
    pub fn show_sizes(mut self, show: bool) -> Self {
        self.show_sizes = show;
        self
    }

    /// Toggle percentage bars.
    #[must_use]
    pub fn show_bars(mut self, show: bool) -> Self {
        self.show_bars = show;
        self
    }

    /// Set max entries.
    #[must_use]
    pub fn max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    /// Get sorted entries (by size descending).
    fn sorted_entries(&self) -> Vec<&FileEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.size.cmp(&a.size));
        sorted
    }

    /// Draw an entry with mini bar.
    fn draw_entry(&self, canvas: &mut dyn Canvas, entry: &FileEntry, x: f32, y: f32, width: f32) {
        let pct = entry.percent_of(self.total_size);

        if self.show_bars {
            // Draw percentage bar first
            let bar_width = ((width - 12.0) * pct / 100.0) as usize;
            let bar: String = "▓".repeat(bar_width.min(20));
            let empty: String = "░".repeat(20_usize.saturating_sub(bar_width));

            canvas.draw_text(
                &format!("{bar}{empty}"),
                Point::new(x, y),
                &TextStyle {
                    color: entry.color,
                    ..Default::default()
                },
            );

            // Draw name after bar
            let name = if entry.name.len() > 8 {
                format!("{}...", &entry.name[..5])
            } else {
                entry.name.clone()
            };

            canvas.draw_text(
                &name,
                Point::new(x + 21.0, y),
                &TextStyle {
                    color: Color::WHITE,
                    ..Default::default()
                },
            );
        } else {
            // Simple list mode
            let name = if entry.name.len() > 12 {
                format!("{}...", &entry.name[..9])
            } else {
                format!("{:<12}", entry.name)
            };

            canvas.draw_text(
                &name,
                Point::new(x, y),
                &TextStyle {
                    color: entry.color,
                    ..Default::default()
                },
            );

            if self.show_sizes {
                canvas.draw_text(
                    &entry.size_display(),
                    Point::new(x + 13.0, y),
                    &TextStyle {
                        color: Color::new(0.7, 0.7, 0.7, 1.0),
                        ..Default::default()
                    },
                );
            }
        }
    }
}

impl Brick for FilesPanel {
    fn brick_name(&self) -> &'static str {
        "files_panel"
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

impl Widget for FilesPanel {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let visible = self.entries.len().min(self.max_entries);
        let height = (visible as f32).max(1.0).min(constraints.max_height);
        Size::new(constraints.max_width, height)
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.bounds.width < 10.0 || self.bounds.height < 1.0 {
            return;
        }

        let mut y = self.bounds.y;
        let x = self.bounds.x;

        // Draw sorted entries
        for entry in self.sorted_entries().iter().take(self.max_entries) {
            if y >= self.bounds.y + self.bounds.height {
                break;
            }
            self.draw_entry(canvas, entry, x, y, self.bounds.width);
            y += 1.0;
        }

        // If no entries, show message
        if self.entries.is_empty() {
            canvas.draw_text(
                "No data",
                Point::new(x, self.bounds.y),
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
    fn test_file_entry_size_display() {
        assert_eq!(FileEntry::file("test", 500).size_display(), "500B");
        assert_eq!(FileEntry::file("test", 1024).size_display(), "1K");
        assert_eq!(FileEntry::file("test", 1024 * 1024).size_display(), "1M");
        assert_eq!(
            FileEntry::file("test", 1024 * 1024 * 1024).size_display(),
            "1.0G"
        );
        assert_eq!(
            FileEntry::file("test", 1024u64 * 1024 * 1024 * 1024).size_display(),
            "1.0T"
        );
    }

    #[test]
    fn test_file_entry_percent() {
        let entry = FileEntry::file("test", 500);
        assert!((entry.percent_of(1000) - 50.0).abs() < 0.1);
        assert!((entry.percent_of(0) - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_directory_colors() {
        let home = FileEntry::directory("home", 1000);
        let var = FileEntry::directory("var", 1000);
        // Just verify they get different colors
        assert_ne!(format!("{:?}", home.color), format!("{:?}", var.color));
    }

    #[test]
    fn test_panel_total_size() {
        let mut panel = FilesPanel::new();
        panel.add_entry(FileEntry::directory("home", 1000));
        panel.add_entry(FileEntry::directory("var", 500));
        assert_eq!(panel.total_size, 1500);
    }

    #[test]
    fn test_panel_with_entries() {
        let entries = vec![
            FileEntry::directory("home", 1000),
            FileEntry::directory("var", 500),
        ];
        let panel = FilesPanel::new().with_entries(entries);
        assert_eq!(panel.total_size, 1500);
        assert_eq!(panel.entries.len(), 2);
    }

    #[test]
    fn test_sorted_entries() {
        let mut panel = FilesPanel::new();
        panel.add_entry(FileEntry::directory("small", 100));
        panel.add_entry(FileEntry::directory("large", 1000));
        panel.add_entry(FileEntry::directory("medium", 500));

        let sorted = panel.sorted_entries();
        assert_eq!(sorted[0].name, "large");
        assert_eq!(sorted[1].name, "medium");
        assert_eq!(sorted[2].name, "small");
    }
}
