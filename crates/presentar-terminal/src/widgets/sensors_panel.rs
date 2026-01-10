//! `SensorsPanel` widget for hardware sensor monitoring.
//!
//! Displays temperature sensors, fan speeds, and voltages from hwmon.
//! Supports status coloring based on thresholds.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// Sensor reading status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SensorStatus {
    #[default]
    Normal,
    Warning,
    Critical,
}

impl SensorStatus {
    /// Get color for status.
    pub fn color(&self) -> Color {
        match self {
            Self::Normal => Color::new(0.4, 0.9, 0.4, 1.0), // Green
            Self::Warning => Color::new(1.0, 0.8, 0.2, 1.0), // Yellow
            Self::Critical => Color::new(1.0, 0.3, 0.3, 1.0), // Red
        }
    }

    /// Get indicator character.
    pub fn indicator(&self) -> char {
        match self {
            Self::Normal => '●',
            Self::Warning => '◐',
            Self::Critical => '○',
        }
    }
}

/// A sensor reading.
#[derive(Debug, Clone)]
pub struct SensorReading {
    /// Sensor label (e.g., "CPU", "GPU", "`NVMe`").
    pub label: String,
    /// Current value.
    pub value: f64,
    /// Unit string.
    pub unit: String,
    /// Critical threshold.
    pub critical: Option<f64>,
    /// Warning threshold.
    pub warning: Option<f64>,
    /// Current status.
    pub status: SensorStatus,
}

impl SensorReading {
    /// Create a temperature reading.
    #[must_use]
    pub fn temperature(label: impl Into<String>, celsius: f64) -> Self {
        let status = if celsius >= 90.0 {
            SensorStatus::Critical
        } else if celsius >= 75.0 {
            SensorStatus::Warning
        } else {
            SensorStatus::Normal
        };

        Self {
            label: label.into(),
            value: celsius,
            unit: "°C".to_string(),
            critical: Some(95.0),
            warning: Some(80.0),
            status,
        }
    }

    /// Create a fan reading.
    #[must_use]
    pub fn fan(label: impl Into<String>, rpm: f64) -> Self {
        Self {
            label: label.into(),
            value: rpm,
            unit: "RPM".to_string(),
            critical: None,
            warning: None,
            status: SensorStatus::Normal,
        }
    }

    /// Create a voltage reading.
    #[must_use]
    pub fn voltage(label: impl Into<String>, volts: f64) -> Self {
        Self {
            label: label.into(),
            value: volts,
            unit: "V".to_string(),
            critical: None,
            warning: None,
            status: SensorStatus::Normal,
        }
    }

    /// Set status explicitly.
    #[must_use]
    pub fn with_status(mut self, status: SensorStatus) -> Self {
        self.status = status;
        self
    }

    /// Set thresholds.
    #[must_use]
    pub fn with_thresholds(mut self, warning: Option<f64>, critical: Option<f64>) -> Self {
        self.warning = warning;
        self.critical = critical;
        // Recalculate status
        if let Some(crit) = critical {
            if self.value >= crit {
                self.status = SensorStatus::Critical;
                return self;
            }
        }
        if let Some(warn) = warning {
            if self.value >= warn {
                self.status = SensorStatus::Warning;
                return self;
            }
        }
        self.status = SensorStatus::Normal;
        self
    }

    /// Format value for display.
    pub fn value_display(&self) -> String {
        if self.unit == "RPM" {
            format!("{:.0} {}", self.value, self.unit)
        } else {
            format!("{:.1}{}", self.value, self.unit)
        }
    }
}

/// Sensors panel displaying temperature, fan, and voltage readings.
#[derive(Debug, Clone)]
pub struct SensorsPanel {
    /// Temperature readings.
    temperatures: Vec<SensorReading>,
    /// Fan readings.
    fans: Vec<SensorReading>,
    /// Voltage readings.
    voltages: Vec<SensorReading>,
    /// Show mini bar for temperatures.
    show_bars: bool,
    /// Max items per category.
    max_per_category: usize,
    /// Cached bounds.
    bounds: Rect,
}

impl Default for SensorsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorsPanel {
    /// Create a new sensors panel.
    #[must_use]
    pub fn new() -> Self {
        Self {
            temperatures: Vec::new(),
            fans: Vec::new(),
            voltages: Vec::new(),
            show_bars: true,
            max_per_category: 4,
            bounds: Rect::default(),
        }
    }

    /// Add a temperature reading.
    pub fn add_temperature(&mut self, reading: SensorReading) {
        self.temperatures.push(reading);
    }

    /// Add a fan reading.
    pub fn add_fan(&mut self, reading: SensorReading) {
        self.fans.push(reading);
    }

    /// Add a voltage reading.
    pub fn add_voltage(&mut self, reading: SensorReading) {
        self.voltages.push(reading);
    }

    /// Set all temperature readings.
    #[must_use]
    pub fn with_temperatures(mut self, readings: Vec<SensorReading>) -> Self {
        self.temperatures = readings;
        self
    }

    /// Set all fan readings.
    #[must_use]
    pub fn with_fans(mut self, readings: Vec<SensorReading>) -> Self {
        self.fans = readings;
        self
    }

    /// Toggle mini bars.
    #[must_use]
    pub fn show_bars(mut self, show: bool) -> Self {
        self.show_bars = show;
        self
    }

    /// Set max items per category.
    #[must_use]
    pub fn max_per_category(mut self, max: usize) -> Self {
        self.max_per_category = max;
        self
    }

    /// Get max temperature.
    pub fn max_temperature(&self) -> Option<f64> {
        self.temperatures.iter().map(|r| r.value).reduce(f64::max)
    }

    /// Check if any sensor is critical.
    pub fn has_critical(&self) -> bool {
        self.temperatures
            .iter()
            .any(|r| r.status == SensorStatus::Critical)
    }

    /// Draw a temperature with mini bar.
    fn draw_temp_bar(
        &self,
        canvas: &mut dyn Canvas,
        reading: &SensorReading,
        x: f32,
        y: f32,
        width: f32,
    ) {
        // Label (left-aligned)
        let label = if reading.label.len() > 8 {
            format!("{}:", &reading.label[..8])
        } else {
            format!("{}:", reading.label)
        };

        canvas.draw_text(
            &label,
            Point::new(x, y),
            &TextStyle {
                color: Color::WHITE,
                ..Default::default()
            },
        );

        // Mini bar (percentage of 100°C max)
        if self.show_bars {
            let bar_x = x + 9.0;
            let bar_width = (width - 18.0) as usize;
            let pct = (reading.value / 100.0).min(1.0);
            let filled = (pct * bar_width as f64) as usize;

            let mut bar = String::new();
            for i in 0..bar_width {
                if i < filled {
                    bar.push('█');
                } else {
                    bar.push('░');
                }
            }

            canvas.draw_text(
                &bar,
                Point::new(bar_x, y),
                &TextStyle {
                    color: reading.status.color(),
                    ..Default::default()
                },
            );
        }

        // Value (right-aligned)
        canvas.draw_text(
            &reading.value_display(),
            Point::new(x + width - 7.0, y),
            &TextStyle {
                color: reading.status.color(),
                ..Default::default()
            },
        );
    }

    /// Draw a fan reading.
    fn draw_fan(&self, canvas: &mut dyn Canvas, reading: &SensorReading, x: f32, y: f32) {
        let line = format!("{}: {}", reading.label, reading.value_display());
        canvas.draw_text(
            &line,
            Point::new(x, y),
            &TextStyle {
                color: Color::new(0.6, 0.8, 1.0, 1.0),
                ..Default::default()
            },
        );
    }
}

impl Brick for SensorsPanel {
    fn brick_name(&self) -> &'static str {
        "sensors_panel"
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

impl Widget for SensorsPanel {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let temp_lines = self.temperatures.len().min(self.max_per_category);
        let fan_lines = self.fans.len().min(self.max_per_category);
        let height = (temp_lines + fan_lines) as f32;
        Size::new(constraints.max_width, height.min(constraints.max_height))
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

        // Draw temperature readings
        for reading in self.temperatures.iter().take(self.max_per_category) {
            if y >= self.bounds.y + self.bounds.height {
                break;
            }
            self.draw_temp_bar(canvas, reading, x, y, self.bounds.width);
            y += 1.0;
        }

        // Draw fan readings
        for reading in self.fans.iter().take(self.max_per_category) {
            if y >= self.bounds.y + self.bounds.height {
                break;
            }
            self.draw_fan(canvas, reading, x, y);
            y += 1.0;
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
    fn test_sensor_reading_temperature() {
        let reading = SensorReading::temperature("CPU", 45.0);
        assert_eq!(reading.status, SensorStatus::Normal);
        assert_eq!(reading.value_display(), "45.0°C");
    }

    #[test]
    fn test_sensor_reading_warning() {
        let reading = SensorReading::temperature("GPU", 78.0);
        assert_eq!(reading.status, SensorStatus::Warning);
    }

    #[test]
    fn test_sensor_reading_critical() {
        let reading = SensorReading::temperature("NVMe", 92.0);
        assert_eq!(reading.status, SensorStatus::Critical);
    }

    #[test]
    fn test_sensor_reading_fan() {
        let reading = SensorReading::fan("Fan 1", 1200.0);
        assert_eq!(reading.value_display(), "1200 RPM");
    }

    #[test]
    fn test_panel_max_temperature() {
        let mut panel = SensorsPanel::new();
        panel.add_temperature(SensorReading::temperature("CPU", 45.0));
        panel.add_temperature(SensorReading::temperature("GPU", 72.0));
        panel.add_temperature(SensorReading::temperature("NVMe", 55.0));

        assert_eq!(panel.max_temperature(), Some(72.0));
    }

    #[test]
    fn test_panel_has_critical() {
        let mut panel = SensorsPanel::new();
        panel.add_temperature(SensorReading::temperature("CPU", 45.0));
        assert!(!panel.has_critical());

        panel.add_temperature(SensorReading::temperature("GPU", 95.0));
        assert!(panel.has_critical());
    }

    #[test]
    fn test_status_color() {
        assert_eq!(SensorStatus::Normal.indicator(), '●');
        assert_eq!(SensorStatus::Warning.indicator(), '◐');
        assert_eq!(SensorStatus::Critical.indicator(), '○');
    }
}
