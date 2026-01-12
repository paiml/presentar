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

/// Unix signals for process control (SPEC-024 Appendix G.6 P0)
/// Matches ttop's SignalType for feature parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalType {
    /// SIGTERM (15) - Graceful termination
    Term,
    /// SIGKILL (9) - Force kill
    Kill,
    /// SIGHUP (1) - Hangup / reload config
    Hup,
    /// SIGINT (2) - Interrupt
    Int,
    /// SIGUSR1 (10) - User-defined signal 1
    Usr1,
    /// SIGUSR2 (12) - User-defined signal 2
    Usr2,
    /// SIGSTOP (19) - Pause process
    Stop,
    /// SIGCONT (18) - Continue paused process
    Cont,
}

impl SignalType {
    /// Get the Unix signal number
    #[cfg(unix)]
    pub fn number(&self) -> i32 {
        match self {
            SignalType::Term => 15,
            SignalType::Kill => 9,
            SignalType::Hup => 1,
            SignalType::Int => 2,
            SignalType::Usr1 => 10,
            SignalType::Usr2 => 12,
            SignalType::Stop => 19,
            SignalType::Cont => 18,
        }
    }

    #[cfg(not(unix))]
    pub fn number(&self) -> i32 {
        0
    }

    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            SignalType::Term => "TERM",
            SignalType::Kill => "KILL",
            SignalType::Hup => "HUP",
            SignalType::Int => "INT",
            SignalType::Usr1 => "USR1",
            SignalType::Usr2 => "USR2",
            SignalType::Stop => "STOP",
            SignalType::Cont => "CONT",
        }
    }

    /// Get key binding for this signal
    pub fn key(&self) -> char {
        match self {
            SignalType::Term => 'x',
            SignalType::Kill => 'K',
            SignalType::Hup => 'H',
            SignalType::Int => 'i',
            SignalType::Usr1 => '1',
            SignalType::Usr2 => '2',
            SignalType::Stop => 'p',
            SignalType::Cont => 'c',
        }
    }

    /// Get description for help display
    pub fn description(&self) -> &'static str {
        match self {
            SignalType::Term => "Graceful shutdown",
            SignalType::Kill => "Force kill (cannot be caught)",
            SignalType::Hup => "Reload config / hangup",
            SignalType::Int => "Interrupt (like Ctrl+C)",
            SignalType::Usr1 => "User signal 1",
            SignalType::Usr2 => "User signal 2",
            SignalType::Stop => "Pause process",
            SignalType::Cont => "Resume paused process",
        }
    }

    /// All available signals
    pub fn all() -> &'static [SignalType] {
        &[
            SignalType::Term,
            SignalType::Kill,
            SignalType::Hup,
            SignalType::Int,
            SignalType::Usr1,
            SignalType::Usr2,
            SignalType::Stop,
            SignalType::Cont,
        ]
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
    /// Reference: SPEC-024 v5.2.0 - Exploded mode for height >= 40
    pub fn for_height(height: u16) -> Self {
        match height {
            0..=5 => Self::Minimal,
            6..=8 => Self::Minimal,
            9..=14 => Self::Compact,
            15..=19 => Self::Normal,
            20..=39 => Self::Expanded,
            _ => Self::Exploded, // height >= 40: fullscreen with history graphs
        }
    }
}

/// Layout type for panel arrangement
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    /// Last modification time for hot reload (SPEC-024 v5.2.0)
    pub last_modified: std::time::SystemTime,
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
            last_modified: std::time::SystemTime::UNIX_EPOCH,
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

    /// Load configuration from a specific file path
    pub fn load_from_file(path: &std::path::Path) -> Option<Self> {
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(path) {
                return Self::parse_yaml(&contents);
            }
        }
        None
    }

    /// Generate default configuration as YAML string
    pub fn default_yaml() -> String {
        r#"# ptop configuration file
# Location: ~/.config/ptop/config.yaml
# Documentation: https://github.com/anthropics/presentar/blob/main/docs/ptop-config.md

# Refresh interval in milliseconds
refresh_ms: 1000

# Layout configuration
layout:
  snap_to_grid: true
  grid_size: 4
  min_panel_width: 30
  min_panel_height: 6

# Panel configuration
panels:
  cpu:
    enabled: true
    histogram: braille    # braille | block | ascii
    show_temperature: true
    show_frequency: true
    sparkline_history: 60  # seconds

  memory:
    enabled: true
    histogram: braille

  disk:
    enabled: true

  network:
    enabled: true
    sparkline_history: 60

  process:
    enabled: true
    max_processes: 20
    columns:
      - pid
      - user
      - cpu
      - mem
      - cmd

  gpu:
    enabled: auto         # auto-detect availability
    show_temperature: true
    show_frequency: true

  sensors:
    enabled: auto

  battery:
    enabled: auto

  connections:
    enabled: true

  files:
    enabled: true

  psi:
    enabled: auto         # Pressure Stall Information

  containers:
    enabled: auto         # Docker/Podman

# Keybindings (default values shown)
keybindings:
  quit: q
  help: "?"
  toggle_fps: f
  filter: "/"
  sort_cpu: c
  sort_mem: m
  sort_pid: p
  kill_process: k
  explode: Enter
  collapse: Escape

# Theme (future - not yet implemented)
# theme:
#   cpu_color: "64C8FF"
#   memory_color: "B478FF"
"#
        .to_string()
    }

    /// Parse YAML config string (simplified parser without full serde)
    /// SPEC-024 v5.2.0: Complete parser for all `LayoutConfig` fields
    /// For full YAML support, add `serde_yaml` dependency
    fn parse_yaml(contents: &str) -> Option<Self> {
        let mut config = Self::default();
        let mut warnings: Vec<String> = Vec::new();

        // Simple line-by-line parser for key config options
        for line in contents.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key: value pairs
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    // Global settings
                    "refresh_ms" => {
                        if let Ok(ms) = value.parse::<u64>() {
                            config.refresh_ms = ms;
                        } else {
                            warnings.push(format!("Invalid refresh_ms: {value}"));
                        }
                    }

                    // Layout settings (SPEC-024 v5.2.0: all fields now parsed)
                    "snap_to_grid" => {
                        config.layout.snap_to_grid = value == "true";
                    }
                    "grid_size" => {
                        if let Ok(size) = value.parse::<u16>() {
                            config.layout.grid_size = size;
                        } else {
                            warnings.push(format!("Invalid grid_size: {value}"));
                        }
                    }
                    "min_panel_width" => {
                        if let Ok(width) = value.parse::<u16>() {
                            config.layout.min_panel_width = width;
                        } else {
                            warnings.push(format!("Invalid min_panel_width: {value}"));
                        }
                    }
                    "min_panel_height" => {
                        if let Ok(height) = value.parse::<u16>() {
                            config.layout.min_panel_height = height;
                        } else {
                            warnings.push(format!("Invalid min_panel_height: {value}"));
                        }
                    }

                    // Nested sections (skip silently, not yet implemented)
                    "layout" | "panels" | "keybindings" | "theme" | "version" => {}

                    // Unknown field warning
                    _ => {
                        if !value.is_empty() {
                            warnings.push(format!("Unknown config field: {key}"));
                        }
                    }
                }
            }
        }

        // Log warnings to stderr (SPEC-024 F1007: warn on invalid fields)
        for warning in warnings {
            eprintln!("[ptop config] warning: {warning}");
        }

        Some(config)
    }

    /// Check if config file has been modified since last load
    /// Returns (modified, `new_config`) if changed
    pub fn check_reload(&self) -> Option<Self> {
        for path in Self::config_paths() {
            if path.exists() {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        // Compare with stored modification time
                        if modified > self.last_modified {
                            return Self::load_from_path(&path);
                        }
                    }
                }
            }
        }
        None
    }

    /// Load config from specific path
    fn load_from_path(path: &std::path::Path) -> Option<Self> {
        if let Ok(contents) = fs::read_to_string(path) {
            let mut config = Self::parse_yaml(&contents)?;
            if let Ok(metadata) = fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    config.last_modified = modified;
                }
            }
            Some(config)
        } else {
            None
        }
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
        assert_eq!(DetailLevel::for_height(39), DetailLevel::Expanded);
        // SPEC-024 v5.2.0: Exploded mode for height >= 40
        assert_eq!(DetailLevel::for_height(40), DetailLevel::Exploded);
        assert_eq!(DetailLevel::for_height(80), DetailLevel::Exploded);
    }

    #[test]
    fn test_default_config() {
        let config = PtopConfig::default();
        assert_eq!(config.refresh_ms, 1000);
        assert!(config.layout.snap_to_grid);
        assert!(config.panels.get(&PanelType::Cpu).unwrap().enabled);
    }

    #[test]
    fn test_parse_yaml_all_layout_fields() {
        // SPEC-024 v5.2.0: Test all LayoutConfig fields are parsed
        let yaml = r#"
refresh_ms: 2000
snap_to_grid: false
grid_size: 4
min_panel_width: 100
min_panel_height: 12
"#;
        let config = PtopConfig::parse_yaml(yaml).unwrap();
        assert_eq!(config.refresh_ms, 2000);
        assert!(!config.layout.snap_to_grid);
        assert_eq!(config.layout.grid_size, 4);
        assert_eq!(config.layout.min_panel_width, 100);
        assert_eq!(config.layout.min_panel_height, 12);
    }

    #[test]
    fn test_parse_yaml_partial_config() {
        // SPEC-024 F1003: Partial config merges with defaults
        let yaml = "min_panel_width: 50";
        let config = PtopConfig::parse_yaml(yaml).unwrap();
        // Custom value applied
        assert_eq!(config.layout.min_panel_width, 50);
        // Default values preserved
        assert_eq!(config.refresh_ms, 1000);
        assert!(config.layout.snap_to_grid);
    }

    #[test]
    fn test_parse_yaml_invalid_values() {
        // SPEC-024 F1007: Invalid values should warn but not crash
        let yaml = r#"
refresh_ms: not_a_number
min_panel_width: 100
"#;
        let config = PtopConfig::parse_yaml(yaml).unwrap();
        // Invalid value uses default
        assert_eq!(config.refresh_ms, 1000);
        // Valid value still applied
        assert_eq!(config.layout.min_panel_width, 100);
    }

    #[test]
    fn test_parse_yaml_comments_ignored() {
        let yaml = r#"
# This is a comment
refresh_ms: 500
# Another comment
min_panel_width: 30
"#;
        let config = PtopConfig::parse_yaml(yaml).unwrap();
        assert_eq!(config.refresh_ms, 500);
        assert_eq!(config.layout.min_panel_width, 30);
    }

    #[test]
    fn test_config_check_reload_returns_none_when_unchanged() {
        let config = PtopConfig::default();
        // No config files exist, should return None
        assert!(config.check_reload().is_none());
    }

    // SignalType tests
    #[test]
    fn test_signal_type_term() {
        assert_eq!(SignalType::Term.name(), "TERM");
        assert_eq!(SignalType::Term.key(), 'x');
        assert!(SignalType::Term.description().contains("Graceful"));
        #[cfg(unix)]
        assert_eq!(SignalType::Term.number(), 15);
    }

    #[test]
    fn test_signal_type_kill() {
        assert_eq!(SignalType::Kill.name(), "KILL");
        assert_eq!(SignalType::Kill.key(), 'K');
        assert!(SignalType::Kill.description().contains("Force"));
        #[cfg(unix)]
        assert_eq!(SignalType::Kill.number(), 9);
    }

    #[test]
    fn test_signal_type_hup() {
        assert_eq!(SignalType::Hup.name(), "HUP");
        assert_eq!(SignalType::Hup.key(), 'H');
        #[cfg(unix)]
        assert_eq!(SignalType::Hup.number(), 1);
    }

    #[test]
    fn test_signal_type_int() {
        assert_eq!(SignalType::Int.name(), "INT");
        assert_eq!(SignalType::Int.key(), 'i');
        #[cfg(unix)]
        assert_eq!(SignalType::Int.number(), 2);
    }

    #[test]
    fn test_signal_type_usr1() {
        assert_eq!(SignalType::Usr1.name(), "USR1");
        assert_eq!(SignalType::Usr1.key(), '1');
        #[cfg(unix)]
        assert_eq!(SignalType::Usr1.number(), 10);
    }

    #[test]
    fn test_signal_type_usr2() {
        assert_eq!(SignalType::Usr2.name(), "USR2");
        assert_eq!(SignalType::Usr2.key(), '2');
        #[cfg(unix)]
        assert_eq!(SignalType::Usr2.number(), 12);
    }

    #[test]
    fn test_signal_type_stop() {
        assert_eq!(SignalType::Stop.name(), "STOP");
        assert_eq!(SignalType::Stop.key(), 'p');
        #[cfg(unix)]
        assert_eq!(SignalType::Stop.number(), 19);
    }

    #[test]
    fn test_signal_type_cont() {
        assert_eq!(SignalType::Cont.name(), "CONT");
        assert_eq!(SignalType::Cont.key(), 'c');
        #[cfg(unix)]
        assert_eq!(SignalType::Cont.number(), 18);
    }

    #[test]
    fn test_signal_type_all() {
        let all = SignalType::all();
        assert_eq!(all.len(), 8);
        assert_eq!(all[0], SignalType::Term);
        assert_eq!(all[7], SignalType::Cont);
    }

    #[test]
    fn test_signal_type_debug() {
        let sig = SignalType::Kill;
        let debug = format!("{:?}", sig);
        assert!(debug.contains("Kill"));
    }

    #[test]
    fn test_signal_type_clone() {
        let sig = SignalType::Stop;
        let cloned = sig.clone();
        assert_eq!(sig, cloned);
    }

    // PanelType tests
    #[test]
    fn test_panel_type_all() {
        let all = PanelType::all();
        assert_eq!(all.len(), 12);
        assert_eq!(all[0], PanelType::Cpu);
        assert_eq!(all[11], PanelType::Containers);
    }

    #[test]
    fn test_panel_type_debug() {
        let panel = PanelType::Memory;
        let debug = format!("{:?}", panel);
        assert!(debug.contains("Memory"));
    }

    #[test]
    fn test_panel_type_clone() {
        let panel = PanelType::Disk;
        let cloned = panel.clone();
        assert_eq!(panel, cloned);
    }

    #[test]
    fn test_panel_type_hash() {
        let mut map = HashMap::new();
        map.insert(PanelType::Cpu, "CPU");
        map.insert(PanelType::Memory, "MEM");
        assert_eq!(map.get(&PanelType::Cpu), Some(&"CPU"));
    }

    // DetailLevel tests
    #[test]
    fn test_detail_level_ordering() {
        assert!(DetailLevel::Minimal < DetailLevel::Compact);
        assert!(DetailLevel::Compact < DetailLevel::Normal);
        assert!(DetailLevel::Normal < DetailLevel::Expanded);
        assert!(DetailLevel::Expanded < DetailLevel::Exploded);
    }

    #[test]
    fn test_detail_level_debug() {
        let level = DetailLevel::Normal;
        let debug = format!("{:?}", level);
        assert!(debug.contains("Normal"));
    }

    #[test]
    fn test_detail_level_clone() {
        let level = DetailLevel::Expanded;
        let cloned = level.clone();
        assert_eq!(level, cloned);
    }

    // LayoutType tests
    #[test]
    fn test_layout_type_default() {
        let layout = LayoutType::default();
        assert_eq!(layout, LayoutType::AdaptiveGrid);
    }

    #[test]
    fn test_layout_type_debug() {
        let layout = LayoutType::Flexbox;
        let debug = format!("{:?}", layout);
        assert!(debug.contains("Flexbox"));
    }

    // FocusStyle tests
    #[test]
    fn test_focus_style_default() {
        let style = FocusStyle::default();
        assert_eq!(style, FocusStyle::DoubleBorder);
    }

    #[test]
    fn test_focus_style_debug() {
        let style = FocusStyle::Pulse;
        let debug = format!("{:?}", style);
        assert!(debug.contains("Pulse"));
    }

    // HistogramStyle tests
    #[test]
    fn test_histogram_style_default() {
        let style = HistogramStyle::default();
        assert_eq!(style, HistogramStyle::Braille);
    }

    #[test]
    fn test_histogram_style_debug() {
        let style = HistogramStyle::Block;
        let debug = format!("{:?}", style);
        assert!(debug.contains("Block"));
    }

    // PanelConfig tests
    #[test]
    fn test_panel_config_default() {
        let config = PanelConfig::default();
        assert!(config.enabled);
        assert!(!config.auto_detect);
        assert_eq!(config.span, 1);
        assert!(config.auto_expand);
        assert!(config.show_temperature);
        assert!(config.show_frequency);
        assert_eq!(config.max_processes, 5);
        assert_eq!(config.sparkline_history, 60);
    }

    #[test]
    fn test_panel_config_debug() {
        let config = PanelConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("PanelConfig"));
    }

    #[test]
    fn test_panel_config_clone() {
        let config = PanelConfig {
            enabled: false,
            max_processes: 10,
            ..Default::default()
        };
        let cloned = config.clone();
        assert!(!cloned.enabled);
        assert_eq!(cloned.max_processes, 10);
    }

    // LayoutConfig tests
    #[test]
    fn test_layout_config_default() {
        let config = LayoutConfig::default();
        assert!(config.snap_to_grid);
        assert_eq!(config.grid_size, 1);
        assert_eq!(config.min_panel_width, 20);
        assert_eq!(config.min_panel_height, 6);
    }

    #[test]
    fn test_layout_config_debug() {
        let config = LayoutConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("LayoutConfig"));
    }

    // ThemeConfig tests
    #[test]
    fn test_theme_config_default() {
        let config = ThemeConfig::default();
        assert!(config.borders.contains_key("cpu"));
        assert!(config.borders.contains_key("memory"));
        assert_eq!(config.background, "default");
        assert_eq!(config.focus_indicator, FocusStyle::DoubleBorder);
    }

    #[test]
    fn test_theme_config_debug() {
        let config = ThemeConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("ThemeConfig"));
    }

    // KeybindingConfig tests
    #[test]
    fn test_keybinding_config_default() {
        let config = KeybindingConfig::default();
        assert_eq!(config.toggle_panel, "1-9");
        assert!(config.quit.contains(&"q".to_string()));
    }

    #[test]
    fn test_keybinding_config_debug() {
        let config = KeybindingConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("KeybindingConfig"));
    }

    // PtopConfig tests
    #[test]
    fn test_ptop_config_panel() {
        let config = PtopConfig::default();
        let cpu = config.panel(PanelType::Cpu);
        assert!(cpu.enabled);
    }

    #[test]
    fn test_ptop_config_panel_unknown() {
        // Test fallback when panel type not in map
        let mut config = PtopConfig::default();
        config.panels.clear();
        let panel = config.panel(PanelType::Cpu);
        assert!(panel.enabled); // Default fallback
    }

    #[test]
    fn test_ptop_config_debug() {
        let config = PtopConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("PtopConfig"));
    }

    #[test]
    fn test_ptop_config_default_yaml() {
        let yaml = PtopConfig::default_yaml();
        assert!(yaml.contains("refresh_ms"));
        assert!(yaml.contains("layout"));
        assert!(yaml.contains("panels"));
        assert!(yaml.contains("keybindings"));
    }

    #[test]
    fn test_ptop_config_paths() {
        let paths = PtopConfig::config_paths();
        // May be empty if HOME not set, or may have entries
        for path in &paths {
            assert!(path.to_string_lossy().contains("ptop"));
        }
    }

    #[test]
    fn test_ptop_config_load_from_file_nonexistent() {
        let result = PtopConfig::load_from_file(std::path::Path::new("/nonexistent/path.yaml"));
        assert!(result.is_none());
    }

    #[test]
    fn test_ptop_config_load_defaults_on_missing_file() {
        // Should return default config
        let config = PtopConfig::load();
        assert_eq!(config.refresh_ms, 1000);
    }

    // PanelRect tests
    #[test]
    fn test_panel_rect_debug() {
        let rect = PanelRect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        let debug = format!("{:?}", rect);
        assert!(debug.contains("PanelRect"));
    }

    #[test]
    fn test_panel_rect_clone() {
        let rect = PanelRect {
            x: 10,
            y: 20,
            width: 30,
            height: 40,
        };
        let cloned = rect.clone();
        assert_eq!(cloned.x, 10);
        assert_eq!(cloned.y, 20);
    }

    // Grid layout edge cases
    #[test]
    fn test_calculate_grid_layout_zero_panels() {
        let config = LayoutConfig::default();
        let rects = calculate_grid_layout(0, 120, 40, &config);
        assert!(rects.is_empty());
    }

    #[test]
    fn test_calculate_grid_layout_five_panels() {
        // 5 panels: row 1 gets 3, row 2 gets 2
        let config = LayoutConfig::default();
        let rects = calculate_grid_layout(5, 120, 40, &config);
        assert_eq!(rects.len(), 5);
    }

    #[test]
    fn test_snap_to_grid_zero() {
        assert_eq!(snap_to_grid(0, 0), 0);
        assert_eq!(snap_to_grid(10, 0), 10);
    }

    // Parse yaml edge cases
    #[test]
    fn test_parse_yaml_empty() {
        let config = PtopConfig::parse_yaml("").unwrap();
        assert_eq!(config.refresh_ms, 1000); // Default
    }

    #[test]
    fn test_parse_yaml_only_comments() {
        let yaml = "# comment\n# another comment\n";
        let config = PtopConfig::parse_yaml(yaml).unwrap();
        assert_eq!(config.refresh_ms, 1000); // Default
    }
}
