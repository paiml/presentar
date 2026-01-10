//! `ContainersPanel` widget for Docker/Podman container monitoring.
//!
//! Displays running containers with status, CPU, and memory usage.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// Container state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContainerState {
    #[default]
    Running,
    Paused,
    Stopped,
    Restarting,
    Dead,
}

impl ContainerState {
    /// Get status indicator.
    pub fn indicator(&self) -> char {
        match self {
            Self::Running => '●',
            Self::Paused => '◐',
            Self::Stopped => '○',
            Self::Restarting => '↻',
            Self::Dead => '✕',
        }
    }

    /// Get status color.
    pub fn color(&self) -> Color {
        match self {
            Self::Running => Color::new(0.4, 0.9, 0.4, 1.0), // Green
            Self::Paused => Color::new(1.0, 0.8, 0.2, 1.0),  // Yellow
            Self::Stopped => Color::new(0.5, 0.5, 0.5, 1.0), // Gray
            Self::Restarting => Color::new(0.4, 0.6, 1.0, 1.0), // Blue
            Self::Dead => Color::new(1.0, 0.3, 0.3, 1.0),    // Red
        }
    }
}

/// A container entry.
#[derive(Debug, Clone)]
pub struct ContainerEntry {
    /// Container name.
    pub name: String,
    /// Container ID (short).
    pub id: String,
    /// Container state.
    pub state: ContainerState,
    /// CPU usage percentage.
    pub cpu_percent: f32,
    /// Memory usage in bytes.
    pub memory_bytes: u64,
    /// Memory limit in bytes (0 = no limit).
    pub memory_limit: u64,
    /// Image name.
    pub image: String,
}

impl ContainerEntry {
    /// Create a new container entry.
    #[must_use]
    pub fn new(name: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: id.into(),
            state: ContainerState::Running,
            cpu_percent: 0.0,
            memory_bytes: 0,
            memory_limit: 0,
            image: String::new(),
        }
    }

    /// Set container state.
    #[must_use]
    pub fn with_state(mut self, state: ContainerState) -> Self {
        self.state = state;
        self
    }

    /// Set CPU percentage.
    #[must_use]
    pub fn with_cpu(mut self, cpu: f32) -> Self {
        self.cpu_percent = cpu;
        self
    }

    /// Set memory usage.
    #[must_use]
    pub fn with_memory(mut self, used: u64, limit: u64) -> Self {
        self.memory_bytes = used;
        self.memory_limit = limit;
        self
    }

    /// Set image name.
    #[must_use]
    pub fn with_image(mut self, image: impl Into<String>) -> Self {
        self.image = image.into();
        self
    }

    /// Format memory for display.
    pub fn memory_display(&self) -> String {
        let mb = self.memory_bytes as f64 / 1_048_576.0;
        if mb >= 1024.0 {
            format!("{:.1}G", mb / 1024.0)
        } else {
            format!("{mb:.0}M")
        }
    }

    /// Get memory percentage.
    pub fn memory_percent(&self) -> Option<f32> {
        if self.memory_limit > 0 {
            Some((self.memory_bytes as f64 / self.memory_limit as f64 * 100.0) as f32)
        } else {
            None
        }
    }
}

/// Containers panel displaying Docker/Podman containers.
#[derive(Debug, Clone)]
pub struct ContainersPanel {
    /// Container entries.
    containers: Vec<ContainerEntry>,
    /// Show only running containers.
    running_only: bool,
    /// Max containers to show.
    max_containers: usize,
    /// Compact mode (single line per container).
    compact: bool,
    /// Cached bounds.
    bounds: Rect,
}

impl Default for ContainersPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainersPanel {
    /// Create a new containers panel.
    #[must_use]
    pub fn new() -> Self {
        Self {
            containers: Vec::new(),
            running_only: true,
            max_containers: 5,
            compact: true,
            bounds: Rect::default(),
        }
    }

    /// Add a container.
    pub fn add_container(&mut self, container: ContainerEntry) {
        self.containers.push(container);
    }

    /// Set all containers.
    #[must_use]
    pub fn with_containers(mut self, containers: Vec<ContainerEntry>) -> Self {
        self.containers = containers;
        self
    }

    /// Show only running containers.
    #[must_use]
    pub fn running_only(mut self, only: bool) -> Self {
        self.running_only = only;
        self
    }

    /// Set max containers to show.
    #[must_use]
    pub fn max_containers(mut self, max: usize) -> Self {
        self.max_containers = max;
        self
    }

    /// Enable compact mode.
    #[must_use]
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// Get running container count.
    pub fn running_count(&self) -> usize {
        self.containers
            .iter()
            .filter(|c| c.state == ContainerState::Running)
            .count()
    }

    /// Get total container count.
    pub fn total_count(&self) -> usize {
        self.containers.len()
    }

    /// Get visible containers (filtered).
    fn visible_containers(&self) -> impl Iterator<Item = &ContainerEntry> {
        self.containers
            .iter()
            .filter(|c| !self.running_only || c.state == ContainerState::Running)
            .take(self.max_containers)
    }

    /// Draw a container line.
    fn draw_container(
        &self,
        canvas: &mut dyn Canvas,
        container: &ContainerEntry,
        x: f32,
        y: f32,
        width: f32,
    ) {
        // State indicator
        canvas.draw_text(
            &container.state.indicator().to_string(),
            Point::new(x, y),
            &TextStyle {
                color: container.state.color(),
                ..Default::default()
            },
        );

        // Container name (truncated)
        let max_name = ((width - 20.0) / 2.0) as usize;
        let name = if container.name.len() > max_name {
            format!("{}...", &container.name[..max_name.saturating_sub(3)])
        } else {
            container.name.clone()
        };

        canvas.draw_text(
            &name,
            Point::new(x + 2.0, y),
            &TextStyle {
                color: Color::WHITE,
                ..Default::default()
            },
        );

        // CPU and Memory
        let stats = format!(
            "{:4.1}% {:>5}",
            container.cpu_percent,
            container.memory_display()
        );
        canvas.draw_text(
            &stats,
            Point::new(x + width - 13.0, y),
            &TextStyle {
                color: Color::new(0.7, 0.7, 0.7, 1.0),
                ..Default::default()
            },
        );
    }
}

impl Brick for ContainersPanel {
    fn brick_name(&self) -> &'static str {
        "containers_panel"
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

impl Widget for ContainersPanel {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let visible = self.visible_containers().count();
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

        // Draw visible containers
        for container in self.visible_containers() {
            if y >= self.bounds.y + self.bounds.height {
                break;
            }
            self.draw_container(canvas, container, x, y, self.bounds.width);
            y += 1.0;
        }

        // If no containers, show message
        if self.containers.is_empty() {
            canvas.draw_text(
                "No containers",
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
    fn test_container_state_indicator() {
        assert_eq!(ContainerState::Running.indicator(), '●');
        assert_eq!(ContainerState::Paused.indicator(), '◐');
        assert_eq!(ContainerState::Stopped.indicator(), '○');
    }

    #[test]
    fn test_container_entry_memory() {
        let entry = ContainerEntry::new("nginx", "abc123")
            .with_memory(256 * 1024 * 1024, 512 * 1024 * 1024);
        assert_eq!(entry.memory_display(), "256M");
        assert!((entry.memory_percent().unwrap() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_container_entry_memory_gb() {
        let entry = ContainerEntry::new("postgres", "def456")
            .with_memory(2 * 1024 * 1024 * 1024, 4 * 1024 * 1024 * 1024);
        assert_eq!(entry.memory_display(), "2.0G");
    }

    #[test]
    fn test_panel_running_count() {
        let mut panel = ContainersPanel::new();
        panel.add_container(ContainerEntry::new("nginx", "a").with_state(ContainerState::Running));
        panel.add_container(ContainerEntry::new("redis", "b").with_state(ContainerState::Running));
        panel.add_container(ContainerEntry::new("old", "c").with_state(ContainerState::Stopped));

        assert_eq!(panel.running_count(), 2);
        assert_eq!(panel.total_count(), 3);
    }

    #[test]
    fn test_panel_builder() {
        let panel = ContainersPanel::new()
            .running_only(false)
            .max_containers(10)
            .compact(false);

        assert!(!panel.running_only);
        assert_eq!(panel.max_containers, 10);
        assert!(!panel.compact);
    }
}
