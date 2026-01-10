//! YAML configuration for ptop.
//!
//! Feature A from SPEC-024 v5.0.0: XDG-compliant configuration loading.
//!
//! Reference: Dourish & Bellotti (1992) "Awareness and coordination"

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Panel types that can be configured
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelType {
    Cpu,
    Memory,
    Disk,
    Network,
    Process,
    Gpu,
    Battery,
    Sensors,
    Files,
    Connections,
    Psi,
    Containers,
}

impl PanelType {
    /// Get all panel types in order
    pub fn all() -> &'static [Self] {
        &[
            Self::Cpu,
            Self::Memory,
            Self::Disk,
            Self::Network,
            Self::Process,
            Self::Gpu,
            Self::Battery,
            Self::Sensors,
            Self::Files,
            Self::Connections,
            Self::Psi,
            Self::Containers,
        ]
    }

    /// Get next panel in cycle
    pub fn next(self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|&p| p == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }

    /// Get previous panel in cycle
    pub fn prev(self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|&p| p == self).unwrap_or(0);
        if idx == 0 {
            all[all.len() - 1]
        } else {
            all[idx - 1]
        }
    }
}

/// Detail level for adaptive panel rendering
/// Reference: SPEC-024 Section 17.3
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DetailLevel {
    /// Just title + single bar (min height: 6)
    Minimal,
    /// + secondary bar, basic stats (min height: 9)
    Compact,
    /// + thermal, power, clock speed (min height: 15)
    Normal,
    /// + processes with G/C types (min height: 20)
    Expanded,
    /// Full screen with history graphs
    Exploded,
}

impl DetailLevel {
    /// Get detail level appropriate for given height
    pub fn for_height(height: u16) -> Self {
        match height {
            0..=5 => Self::Minimal,
            6..=8 => Self::Minimal,
            9..=14 => Self::Compact,
            15..=19 => Self::Normal,
            _ => Self::Expanded,
        }
    }
}

/// Layout type for panel arrangement
#[derive(Debug, Clone, Copy, Default)]
pub enum LayoutType {
    /// Adaptive grid with automatic column distribution
    #[default]
    AdaptiveGrid,
    /// Fixed grid with explicit row/col positions
    FixedGrid,
    /// Flexbox-style layout
    Flexbox,
    /// Constraint-based layout
    Constraint,
}

/// Focus indicator style
#[derive(Debug, Clone, Copy, Default)]
pub enum FocusStyle {
    /// Double-line border (ttop default)
    #[default]
    DoubleBorder,
    /// Highlighted border color
    HighlightBorder,
    /// Animated pulse
    Pulse,
    /// Bold title
    BoldTitle,
}

/// Histogram style for CPU/Memory bars
#[derive(Debug, Clone, Copy, Default)]
pub enum HistogramStyle {
    /// Braille characters (highest resolution)
    #[default]
    Braille,
    /// Block characters
    Block,
    /// ASCII only
    Ascii,
}

/// Panel-specific configuration
#[derive(Debug, Clone)]
pub struct PanelConfig {
    /// Whether panel is enabled
    pub enabled: bool,
    /// Auto-detect availability (for GPU, etc.)
    pub auto_detect: bool,
    /// Grid position (row, col)
    pub position: Option<(u16, u16)>,
    /// Column span
    pub span: u16,
    /// Auto-expand when space available
    pub auto_expand: bool,
    /// Minimum detail level to show
    pub min_detail: DetailLevel,
    /// Expansion priority (higher = expands first)
    pub expansion_priority: u8,
    /// Histogram style
    pub histogram: HistogramStyle,
    /// Show temperature (CPU/GPU)
    pub show_temperature: bool,
    /// Show frequency (CPU/GPU)
    pub show_frequency: bool,
    /// Max processes to show
    pub max_processes: usize,
    /// Columns to show in process list
    pub process_columns: Vec<String>,
    /// Sparkline history in seconds
    pub sparkline_history: u32,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_detect: false,
            position: None,
            span: 1,
            auto_expand: true,
            min_detail: DetailLevel::Compact,
            expansion_priority: 5,
            histogram: HistogramStyle::default(),
            show_temperature: true,
            show_frequency: true,
            max_processes: 5,
            process_columns: vec![
                "pid".into(),
                "user".into(),
                "cpu".into(),
                "mem".into(),
                "cmd".into(),
            ],
            sparkline_history: 60,
        }
    }
}

/// Layout configuration
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Layout algorithm type
    pub layout_type: LayoutType,
    /// Snap panel boundaries to grid
    pub snap_to_grid: bool,
    /// Grid snap size in characters
    pub grid_size: u16,
    /// Minimum panel width in characters
    pub min_panel_width: u16,
    /// Minimum panel height in characters
    pub min_panel_height: u16,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            layout_type: LayoutType::AdaptiveGrid,
            snap_to_grid: true,
            grid_size: 1, // Character-level snap (ttop behavior)
            min_panel_width: 20,
            min_panel_height: 6,
        }
    }
}

/// Theme configuration
#[derive(Debug, Clone)]
pub struct ThemeConfig {
    /// Panel border colors (hex strings)
    pub borders: HashMap<String, String>,
    /// Background setting
    pub background: String,
    /// Focus indicator style
    pub focus_indicator: FocusStyle,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        let mut borders = HashMap::new();
        borders.insert("cpu".into(), "#64C8FF".into());
        borders.insert("memory".into(), "#B478FF".into());
        borders.insert("disk".into(), "#64B4FF".into());
        borders.insert("network".into(), "#FF9664".into());
        borders.insert("process".into(), "#DCC464".into());
        borders.insert("gpu".into(), "#64FF96".into());
        borders.insert("battery".into(), "#FFDC64".into());
        borders.insert("sensors".into(), "#FF6496".into());

        Self {
            borders,
            background: "default".into(),
            focus_indicator: FocusStyle::DoubleBorder,
        }
    }
}

/// Keybinding configuration
#[derive(Debug, Clone)]
pub struct KeybindingConfig {
    /// Keys to toggle panels (default: 1-9)
    pub toggle_panel: String,
    /// Keys to explode/collapse panel
    pub explode_panel: Vec<String>,
    /// Keys for navigation
    pub navigate: Vec<String>,
    /// Keys to quit
    pub quit: Vec<String>,
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self {
            toggle_panel: "1-9".into(),
            explode_panel: vec!["Enter".into(), "z".into()],
            navigate: vec!["Tab".into(), "Shift+Tab".into(), "hjkl".into()],
            quit: vec!["q".into(), "Ctrl+c".into()],
        }
    }
}

/// Main ptop configuration
#[derive(Debug, Clone)]
pub struct PtopConfig {
    /// Configuration version
    pub version: String,
    /// Refresh interval in milliseconds
    pub refresh_ms: u64,
    /// Layout settings
    pub layout: LayoutConfig,
    /// Per-panel settings
    pub panels: HashMap<PanelType, PanelConfig>,
    /// Theme settings
    pub theme: ThemeConfig,
    /// Keybinding settings
    pub keybindings: KeybindingConfig,
}

impl Default for PtopConfig {
    fn default() -> Self {
        let mut panels = HashMap::new();

        // Core panels - enabled by default
        panels.insert(PanelType::Cpu, PanelConfig::default());
        panels.insert(PanelType::Memory, PanelConfig::default());
        panels.insert(PanelType::Disk, PanelConfig::default());
        panels.insert(PanelType::Network, PanelConfig::default());
        panels.insert(PanelType::Process, PanelConfig::default());

        // Hardware panels - auto-detect
        panels.insert(
            PanelType::Gpu,
            PanelConfig {
                auto_detect: true,
                enabled: false, // Will be enabled if GPU detected
                process_columns: vec![
                    "type".into(), // G or C
                    "pid".into(),
                    "sm".into(),
                    "mem".into(),
                    "enc".into(),
                    "dec".into(),
                    "cmd".into(),
                ],
                ..Default::default()
            },
        );
        panels.insert(
            PanelType::Sensors,
            PanelConfig {
                auto_detect: true,
                enabled: false,
                ..Default::default()
            },
        );
        panels.insert(
            PanelType::Connections,
            PanelConfig {
                auto_detect: true,
                enabled: false,
                ..Default::default()
            },
        );
        panels.insert(
            PanelType::Psi,
            PanelConfig {
                auto_detect: true,
                enabled: false,
                ..Default::default()
            },
        );

        // Optional panels - disabled by default
        panels.insert(
            PanelType::Battery,
            PanelConfig {
                auto_detect: true,
                enabled: false,
                ..Default::default()
            },
        );
        panels.insert(
            PanelType::Files,
            PanelConfig {
                auto_detect: true,
                enabled: false,
                ..Default::default()
            },
        );
        panels.insert(
            PanelType::Containers,
            PanelConfig {
                auto_detect: true,
                enabled: false,
                ..Default::default()
            },
        );

        Self {
            version: "1.0".into(),
            refresh_ms: 1000,
            layout: LayoutConfig::default(),
            panels,
            theme: ThemeConfig::default(),
            keybindings: KeybindingConfig::default(),
        }
    }
}

impl PtopConfig {
    /// Get XDG-compliant config paths to search
    /// Order: $`XDG_CONFIG_HOME/ptop/config.yaml`, ~/.config/ptop/config.yaml
    pub fn config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // XDG_CONFIG_HOME
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            paths.push(PathBuf::from(xdg_config).join("ptop/config.yaml"));
        }

        // Fallback to ~/.config using HOME environment variable
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(home).join(".config/ptop/config.yaml"));
        }

        paths
    }

    /// Load configuration from file, falling back to defaults
    pub fn load() -> Self {
        for path in Self::config_paths() {
            if path.exists() {
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Some(config) = Self::parse_yaml(&contents) {
                        return config;
                    }
                }
            }
        }

        // No config found, use defaults
        Self::default()
    }

    /// Parse YAML config string (simplified parser without full serde)
    /// For full YAML support, add `serde_yaml` dependency
    fn parse_yaml(contents: &str) -> Option<Self> {
        let mut config = Self::default();

        // Simple line-by-line parser for key config options
        for line in contents.lines() {
            let line = line.trim();

            // Parse refresh_ms
            if line.starts_with("refresh_ms:") {
                if let Some(value) = line.strip_prefix("refresh_ms:") {
                    if let Ok(ms) = value.trim().parse::<u64>() {
                        config.refresh_ms = ms;
                    }
                }
            }

            // Parse snap_to_grid
            if line.starts_with("snap_to_grid:") {
                if let Some(value) = line.strip_prefix("snap_to_grid:") {
                    config.layout.snap_to_grid = value.trim() == "true";
                }
            }

            // Parse grid_size
            if line.starts_with("grid_size:") {
                if let Some(value) = line.strip_prefix("grid_size:") {
                    if let Ok(size) = value.trim().parse::<u16>() {
                        config.layout.grid_size = size;
                    }
                }
            }
        }

        Some(config)
    }

    /// Get panel config, returning default if not configured
    pub fn panel(&self, panel_type: PanelType) -> &PanelConfig {
        self.panels.get(&panel_type).unwrap_or_else(|| {
            static DEFAULT: PanelConfig = PanelConfig {
                enabled: true,
                auto_detect: false,
                position: None,
                span: 1,
                auto_expand: true,
                min_detail: DetailLevel::Compact,
                expansion_priority: 5,
                histogram: HistogramStyle::Braille,
                show_temperature: true,
                show_frequency: true,
                max_processes: 5,
                process_columns: Vec::new(),
                sparkline_history: 60,
            };
            &DEFAULT
        })
    }
}

/// Snap a value to the nearest grid boundary
/// Reference: SPEC-024 Section 14.3
pub fn snap_to_grid(value: u16, grid_size: u16) -> u16 {
    if grid_size == 0 || grid_size == 1 {
        return value;
    }
    ((value + grid_size / 2) / grid_size) * grid_size
}

/// Calculate panel rectangles with adaptive grid layout
/// Reference: SPEC-024 Section 14.2, ttop/src/ui.rs lines 162-239
pub fn calculate_grid_layout(
    panel_count: u32,
    width: u16,
    height: u16,
    config: &LayoutConfig,
) -> Vec<PanelRect> {
    if panel_count == 0 {
        return Vec::new();
    }

    // For small counts (1-4), put all in one row
    // For larger counts, use 2 rows with ceiling division
    let (rows, first_row_count, second_row_count) = if panel_count <= 4 {
        (1u32, panel_count as usize, 0usize)
    } else {
        // Ceiling division to distribute panels: e.g., 7 → 4 + 3
        let first = (panel_count as usize).div_ceil(2);
        let second = panel_count as usize - first;
        (2u32, first, second)
    };

    let row_height = height / rows as u16;
    let mut rects = Vec::with_capacity(panel_count as usize);

    // First row
    let first_col_width = width / first_row_count as u16;
    for i in 0..first_row_count {
        let x = snap_to_grid(i as u16 * first_col_width, config.grid_size);
        let w = if i == first_row_count - 1 {
            width - x // Last panel takes remaining space
        } else {
            snap_to_grid(first_col_width, config.grid_size)
        };

        rects.push(PanelRect {
            x,
            y: 0,
            width: w.max(config.min_panel_width),
            height: if rows == 1 {
                height
            } else {
                row_height.max(config.min_panel_height)
            },
        });
    }

    // Second row (if needed)
    if second_row_count > 0 {
        let second_col_width = width / second_row_count as u16;
        for i in 0..second_row_count {
            let x = snap_to_grid(i as u16 * second_col_width, config.grid_size);
            let w = if i == second_row_count - 1 {
                width - x
            } else {
                snap_to_grid(second_col_width, config.grid_size)
            };

            rects.push(PanelRect {
                x,
                y: row_height,
                width: w.max(config.min_panel_width),
                height: (height - row_height).max(config.min_panel_height),
            });
        }
    }

    rects
}

/// Rectangle for a panel
#[derive(Debug, Clone, Copy)]
pub struct PanelRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_to_grid() {
        assert_eq!(snap_to_grid(10, 1), 10);
        assert_eq!(snap_to_grid(10, 8), 8);
        assert_eq!(snap_to_grid(12, 8), 16);
        assert_eq!(snap_to_grid(15, 8), 16);
        assert_eq!(snap_to_grid(0, 8), 0);
    }

    #[test]
    fn test_calculate_grid_layout_single() {
        let config = LayoutConfig::default();
        let rects = calculate_grid_layout(1, 120, 40, &config);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].width, 120);
        assert_eq!(rects[0].height, 40);
    }

    #[test]
    fn test_calculate_grid_layout_two() {
        let config = LayoutConfig::default();
        let rects = calculate_grid_layout(2, 120, 40, &config);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].width, 60);
        assert_eq!(rects[1].width, 60);
    }

    #[test]
    fn test_calculate_grid_layout_seven() {
        // 7 panels: row 1 gets 4, row 2 gets 3
        let config = LayoutConfig::default();
        let rects = calculate_grid_layout(7, 120, 40, &config);
        assert_eq!(rects.len(), 7);
        // First row: 4 panels of 30 width each
        assert_eq!(rects[0].y, 0);
        assert_eq!(rects[3].y, 0);
        // Second row: 3 panels
        assert_eq!(rects[4].y, 20);
        assert_eq!(rects[6].y, 20);
    }

    #[test]
    fn test_panel_type_cycle() {
        let cpu = PanelType::Cpu;
        assert_eq!(cpu.next(), PanelType::Memory);
        assert_eq!(PanelType::Containers.next(), PanelType::Cpu);
        assert_eq!(PanelType::Cpu.prev(), PanelType::Containers);
    }

    #[test]
    fn test_detail_level_for_height() {
        assert_eq!(DetailLevel::for_height(5), DetailLevel::Minimal);
        assert_eq!(DetailLevel::for_height(6), DetailLevel::Minimal);
        assert_eq!(DetailLevel::for_height(10), DetailLevel::Compact);
        assert_eq!(DetailLevel::for_height(16), DetailLevel::Normal);
        assert_eq!(DetailLevel::for_height(25), DetailLevel::Expanded);
    }

    #[test]
    fn test_default_config() {
        let config = PtopConfig::default();
        assert_eq!(config.refresh_ms, 1000);
        assert!(config.layout.snap_to_grid);
        assert!(config.panels.get(&PanelType::Cpu).unwrap().enabled);
    }
}
