//! Presentar Scene Format (.prs) parser.
//!
//! This module implements the `.prs` format specification for shareable
//! visualization manifests. The format is YAML-based and declarative,
//! enabling WASM-native parsing without runtime interpreters.
//!
//! # Design Philosophy
//!
//! A `.prs` file is a *bill of materials* for a visualization—it declares
//! **what** to display and **where** data lives, not **how** to fetch or render it.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Presentar Scene - top-level structure for `.prs` files.
///
/// A Scene is a declarative manifest that references external resources
/// (models, datasets) and defines widget layout and interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// Format version (semver, e.g., "1.0")
    pub prs_version: String,

    /// Scene metadata
    pub metadata: SceneMetadata,

    /// External resources (models, datasets)
    #[serde(default)]
    pub resources: Resources,

    /// Widget layout configuration
    pub layout: SceneLayout,

    /// Widget definitions
    pub widgets: Vec<SceneWidget>,

    /// Event → action bindings
    #[serde(default)]
    pub bindings: Vec<Binding>,

    /// Theme configuration
    #[serde(default)]
    pub theme: Option<SceneTheme>,

    /// Security permissions
    #[serde(default)]
    pub permissions: Permissions,
}

/// Scene metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMetadata {
    /// Unique scene identifier (kebab-case, e.g., "sentiment-analysis-demo")
    pub name: String,

    /// Human-readable title
    #[serde(default)]
    pub title: Option<String>,

    /// Description
    #[serde(default)]
    pub description: Option<String>,

    /// Author email or identifier
    #[serde(default)]
    pub author: Option<String>,

    /// Creation timestamp (ISO 8601)
    #[serde(default)]
    pub created: Option<String>,

    /// License identifier (e.g., "MIT", "Apache-2.0")
    #[serde(default)]
    pub license: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// External resources container.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Resources {
    /// Model resources
    #[serde(default)]
    pub models: HashMap<String, ModelResource>,

    /// Dataset resources
    #[serde(default)]
    pub datasets: HashMap<String, DatasetResource>,
}

/// Model resource reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResource {
    /// Model format (apr, gguf, safetensors)
    #[serde(rename = "type")]
    pub resource_type: ModelType,

    /// Source URL or path (can be array for fallback)
    pub source: ResourceSource,

    /// Content hash for verification (blake3:<hex>)
    #[serde(default)]
    pub hash: Option<String>,

    /// File size in bytes (for progress indication)
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// Dataset resource reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetResource {
    /// Dataset format (ald, parquet, csv)
    #[serde(rename = "type")]
    pub resource_type: DatasetType,

    /// Source URL or path (can be array for fallback)
    pub source: ResourceSource,

    /// Content hash for verification (blake3:<hex>)
    #[serde(default)]
    pub hash: Option<String>,

    /// File size in bytes (for progress indication)
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// Model format types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelType {
    /// Aprender model format
    Apr,
    /// GGUF quantized format
    Gguf,
    /// SafeTensors format
    Safetensors,
}

/// Dataset format types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasetType {
    /// Alimentar dataset format
    Ald,
    /// Apache Parquet
    Parquet,
    /// Comma-separated values
    Csv,
}

/// Resource source - single URL/path or array of fallbacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResourceSource {
    /// Single source
    Single(String),
    /// Multiple sources (tried in order)
    Multiple(Vec<String>),
}

impl ResourceSource {
    /// Get all sources as a slice.
    #[must_use]
    pub fn sources(&self) -> Vec<&str> {
        match self {
            Self::Single(s) => vec![s.as_str()],
            Self::Multiple(v) => v.iter().map(String::as_str).collect(),
        }
    }

    /// Get primary source.
    #[must_use]
    pub fn primary(&self) -> &str {
        match self {
            Self::Single(s) => s.as_str(),
            Self::Multiple(v) => v.first().map_or("", String::as_str),
        }
    }
}

/// Scene layout configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneLayout {
    /// Layout type
    #[serde(rename = "type")]
    pub layout_type: LayoutType,

    /// Number of columns (for grid layout)
    #[serde(default)]
    pub columns: Option<u32>,

    /// Number of rows (for grid layout)
    #[serde(default)]
    pub rows: Option<u32>,

    /// Gap between widgets in pixels
    #[serde(default = "default_gap")]
    pub gap: u32,

    /// Flex direction (for flex layout)
    #[serde(default)]
    pub direction: Option<FlexDirection>,

    /// Flex wrap (for flex layout)
    #[serde(default)]
    pub wrap: Option<bool>,

    /// Canvas width (for absolute layout)
    #[serde(default)]
    pub width: Option<u32>,

    /// Canvas height (for absolute layout)
    #[serde(default)]
    pub height: Option<u32>,
}

const fn default_gap() -> u32 {
    16
}

/// Layout type enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutType {
    /// CSS Grid layout
    Grid,
    /// Flexbox layout
    Flex,
    /// Absolute positioning
    Absolute,
}

/// Flex direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlexDirection {
    /// Horizontal (left to right)
    Row,
    /// Vertical (top to bottom)
    Column,
}

/// Scene widget definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneWidget {
    /// Unique widget identifier
    pub id: String,

    /// Widget type
    #[serde(rename = "type")]
    pub widget_type: WidgetType,

    /// Grid position (for grid layout)
    #[serde(default)]
    pub position: Option<GridPosition>,

    /// Widget-specific configuration
    #[serde(default)]
    pub config: WidgetConfig,
}

/// Widget types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetType {
    /// Text input field
    Textbox,
    /// Numeric slider
    Slider,
    /// Selection dropdown
    Dropdown,
    /// Clickable button
    Button,
    /// Image display
    Image,
    /// Bar chart visualization
    BarChart,
    /// Line chart visualization
    LineChart,
    /// Single-value gauge
    Gauge,
    /// Data table
    Table,
    /// Markdown content
    Markdown,
    /// Model inference runner
    Inference,
}

/// Grid position for widgets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPosition {
    /// Row index (0-based)
    pub row: u32,
    /// Column index (0-based)
    pub col: u32,
    /// Column span (defaults to 1)
    #[serde(default = "default_span")]
    pub colspan: u32,
    /// Row span (defaults to 1)
    #[serde(default = "default_span")]
    pub rowspan: u32,
}

const fn default_span() -> u32 {
    1
}

/// Widget configuration - varies by widget type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WidgetConfig {
    // Common fields
    /// Label text
    #[serde(default)]
    pub label: Option<String>,
    /// Title text
    #[serde(default)]
    pub title: Option<String>,

    // Textbox fields
    /// Placeholder text
    #[serde(default)]
    pub placeholder: Option<String>,
    /// Maximum input length
    #[serde(default)]
    pub max_length: Option<u32>,

    // Slider fields
    /// Minimum value
    #[serde(default)]
    pub min: Option<f64>,
    /// Maximum value
    #[serde(default)]
    pub max: Option<f64>,
    /// Step increment
    #[serde(default)]
    pub step: Option<f64>,
    /// Default value
    #[serde(default)]
    pub default: Option<f64>,

    // Dropdown fields
    /// Selection options
    #[serde(default)]
    pub options: Option<String>,
    /// Allow multiple selection
    #[serde(default)]
    pub multi_select: Option<bool>,

    // Button fields
    /// Button action
    #[serde(default)]
    pub action: Option<String>,

    // Image fields
    /// Image source URL/path
    #[serde(default)]
    pub source: Option<String>,
    /// Alt text
    #[serde(default)]
    pub alt: Option<String>,
    /// Upload mode
    #[serde(default)]
    pub mode: Option<String>,
    /// Accepted MIME types
    #[serde(default)]
    pub accept: Option<Vec<String>>,

    // Chart fields
    /// Data source expression
    #[serde(default)]
    pub data: Option<String>,
    /// X-axis field/expression
    #[serde(default)]
    pub x_axis: Option<String>,
    /// Y-axis field/expression
    #[serde(default)]
    pub y_axis: Option<String>,

    // Gauge fields
    /// Gauge value expression
    #[serde(default)]
    pub value: Option<String>,
    /// Gauge thresholds
    #[serde(default)]
    pub thresholds: Option<Vec<Threshold>>,

    // Table fields
    /// Column definitions
    #[serde(default)]
    pub columns: Option<Vec<String>>,
    /// Sortable flag
    #[serde(default)]
    pub sortable: Option<bool>,

    // Markdown fields
    /// Markdown content
    #[serde(default)]
    pub content: Option<String>,

    // Inference fields
    /// Model reference
    #[serde(default)]
    pub model: Option<String>,
    /// Input expression
    #[serde(default)]
    pub input: Option<String>,
    /// Output field
    #[serde(default)]
    pub output: Option<String>,
}

/// Gauge threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threshold {
    /// Threshold value
    pub value: f64,
    /// Color at/below threshold
    pub color: String,
}

/// Event binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    /// Event trigger (e.g., "text_input.change")
    pub trigger: String,

    /// Debounce delay in milliseconds
    #[serde(default)]
    pub debounce_ms: Option<u32>,

    /// Actions to execute
    pub actions: Vec<BindingAction>,
}

/// Binding action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingAction {
    /// Target (widget ID or inference.model)
    pub target: String,

    /// Action type (refresh, set, etc.)
    #[serde(default)]
    pub action: Option<String>,

    /// Input expression
    #[serde(default)]
    pub input: Option<String>,
}

/// Theme configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneTheme {
    /// Theme preset (light, dark)
    #[serde(default)]
    pub preset: Option<String>,

    /// Custom theme values
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

/// Security permissions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Permissions {
    /// Allowed network URLs (glob patterns)
    #[serde(default)]
    pub network: Vec<String>,

    /// Allowed filesystem paths (glob patterns)
    #[serde(default)]
    pub filesystem: Vec<String>,

    /// Clipboard access
    #[serde(default)]
    pub clipboard: bool,

    /// Camera access
    #[serde(default)]
    pub camera: bool,
}

/// Error type for scene parsing and validation.
#[derive(Debug)]
pub enum SceneError {
    /// YAML parsing error
    Yaml(serde_yaml::Error),

    /// Invalid prs_version format
    InvalidVersion(String),

    /// Duplicate widget ID
    DuplicateWidgetId(String),

    /// Invalid binding target (references non-existent widget)
    InvalidBindingTarget {
        /// The binding trigger
        trigger: String,
        /// The invalid target
        target: String,
    },

    /// Invalid hash format
    InvalidHashFormat {
        /// Resource name
        resource: String,
        /// The invalid hash
        hash: String,
    },

    /// Missing required hash for remote resource
    MissingRemoteHash {
        /// Resource name
        resource: String,
    },

    /// Invalid expression syntax
    InvalidExpression {
        /// Widget ID or context
        context: String,
        /// The invalid expression
        expression: String,
        /// Error message
        message: String,
    },

    /// Invalid metadata name (must be kebab-case)
    InvalidMetadataName(String),

    /// Layout validation error
    LayoutError(String),
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml(e) => write!(f, "YAML error: {e}"),
            Self::InvalidVersion(v) => write!(f, "Invalid prs_version: {v}"),
            Self::DuplicateWidgetId(id) => write!(f, "Duplicate widget id: {id}"),
            Self::InvalidBindingTarget { trigger, target } => {
                write!(
                    f,
                    "Invalid binding target '{target}' in trigger '{trigger}'"
                )
            }
            Self::InvalidHashFormat { resource, hash } => {
                write!(f, "Invalid hash format for '{resource}': {hash}")
            }
            Self::MissingRemoteHash { resource } => {
                write!(f, "Missing hash for remote resource: {resource}")
            }
            Self::InvalidExpression {
                context,
                expression,
                message,
            } => {
                write!(
                    f,
                    "Invalid expression in {context}: '{expression}' - {message}"
                )
            }
            Self::InvalidMetadataName(name) => {
                write!(f, "Invalid metadata name '{name}': must be kebab-case")
            }
            Self::LayoutError(msg) => write!(f, "Layout error: {msg}"),
        }
    }
}

impl std::error::Error for SceneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Yaml(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_yaml::Error> for SceneError {
    fn from(e: serde_yaml::Error) -> Self {
        Self::Yaml(e)
    }
}

impl Scene {
    /// Parse a scene from YAML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the YAML is invalid or fails validation.
    pub fn from_yaml(yaml: &str) -> Result<Self, SceneError> {
        let scene: Self = serde_yaml::from_str(yaml)?;
        scene.validate()?;
        Ok(scene)
    }

    /// Serialize scene to YAML string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Validate the scene structure.
    ///
    /// Checks:
    /// 1. prs_version format (semver)
    /// 2. metadata.name is kebab-case
    /// 3. Widget IDs are unique
    /// 4. Binding targets reference valid widgets/resources
    /// 5. Remote resources have hashes
    /// 6. Hash formats are valid (blake3:<hex>)
    ///
    /// # Errors
    ///
    /// Returns the first validation error found.
    pub fn validate(&self) -> Result<(), SceneError> {
        self.validate_version()?;
        self.validate_metadata_name()?;
        self.validate_widget_ids()?;
        self.validate_bindings()?;
        self.validate_resource_hashes()?;
        self.validate_layout()?;
        Ok(())
    }

    fn validate_version(&self) -> Result<(), SceneError> {
        // Version should be "X.Y" format
        let parts: Vec<&str> = self.prs_version.split('.').collect();
        if parts.len() != 2 {
            return Err(SceneError::InvalidVersion(self.prs_version.clone()));
        }
        for part in parts {
            if part.parse::<u32>().is_err() {
                return Err(SceneError::InvalidVersion(self.prs_version.clone()));
            }
        }
        Ok(())
    }

    fn validate_metadata_name(&self) -> Result<(), SceneError> {
        let name = &self.metadata.name;
        // Must be kebab-case: lowercase letters, numbers, hyphens
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(SceneError::InvalidMetadataName(name.clone()));
        }
        // Cannot start or end with hyphen
        if name.starts_with('-') || name.ends_with('-') {
            return Err(SceneError::InvalidMetadataName(name.clone()));
        }
        // Cannot have consecutive hyphens
        if name.contains("--") {
            return Err(SceneError::InvalidMetadataName(name.clone()));
        }
        Ok(())
    }

    fn validate_widget_ids(&self) -> Result<(), SceneError> {
        let mut seen = std::collections::HashSet::new();
        for widget in &self.widgets {
            if !seen.insert(&widget.id) {
                return Err(SceneError::DuplicateWidgetId(widget.id.clone()));
            }
        }
        Ok(())
    }

    fn validate_bindings(&self) -> Result<(), SceneError> {
        let widget_ids: std::collections::HashSet<&str> =
            self.widgets.iter().map(|w| w.id.as_str()).collect();
        let model_ids: std::collections::HashSet<&str> =
            self.resources.models.keys().map(String::as_str).collect();

        for binding in &self.bindings {
            for action in &binding.actions {
                let target = &action.target;

                // Check if target is a widget ID
                if widget_ids.contains(target.as_str()) {
                    continue;
                }

                // Check if target is inference.<model_name>
                if let Some(model_name) = target.strip_prefix("inference.") {
                    if model_ids.contains(model_name) {
                        continue;
                    }
                }

                return Err(SceneError::InvalidBindingTarget {
                    trigger: binding.trigger.clone(),
                    target: target.clone(),
                });
            }
        }
        Ok(())
    }

    fn validate_resource_hashes(&self) -> Result<(), SceneError> {
        // Validate model hashes
        for (name, resource) in &self.resources.models {
            if is_remote_source(&resource.source) && resource.hash.is_none() {
                return Err(SceneError::MissingRemoteHash {
                    resource: name.clone(),
                });
            }
            if let Some(hash) = &resource.hash {
                validate_hash_format(name, hash)?;
            }
        }

        // Validate dataset hashes
        for (name, resource) in &self.resources.datasets {
            if is_remote_source(&resource.source) && resource.hash.is_none() {
                return Err(SceneError::MissingRemoteHash {
                    resource: name.clone(),
                });
            }
            if let Some(hash) = &resource.hash {
                validate_hash_format(name, hash)?;
            }
        }

        Ok(())
    }

    fn validate_layout(&self) -> Result<(), SceneError> {
        match self.layout.layout_type {
            LayoutType::Grid => {
                if self.layout.columns.is_none() {
                    return Err(SceneError::LayoutError(
                        "Grid layout requires 'columns' field".to_string(),
                    ));
                }
            }
            LayoutType::Absolute => {
                if self.layout.width.is_none() || self.layout.height.is_none() {
                    return Err(SceneError::LayoutError(
                        "Absolute layout requires 'width' and 'height' fields".to_string(),
                    ));
                }
            }
            LayoutType::Flex => {
                // Flex layout has optional fields
            }
        }
        Ok(())
    }

    /// Get all widget IDs.
    #[must_use]
    pub fn widget_ids(&self) -> Vec<&str> {
        self.widgets.iter().map(|w| w.id.as_str()).collect()
    }

    /// Get a widget by ID.
    #[must_use]
    pub fn get_widget(&self, id: &str) -> Option<&SceneWidget> {
        self.widgets.iter().find(|w| w.id == id)
    }

    /// Get a model resource by name.
    #[must_use]
    pub fn get_model(&self, name: &str) -> Option<&ModelResource> {
        self.resources.models.get(name)
    }

    /// Get a dataset resource by name.
    #[must_use]
    pub fn get_dataset(&self, name: &str) -> Option<&DatasetResource> {
        self.resources.datasets.get(name)
    }
}

/// Check if a resource source is remote (https://).
fn is_remote_source(source: &ResourceSource) -> bool {
    source.sources().iter().any(|s| s.starts_with("https://"))
}

/// Validate hash format (blake3:<64-hex-chars>).
fn validate_hash_format(resource: &str, hash: &str) -> Result<(), SceneError> {
    if let Some(hex) = hash.strip_prefix("blake3:") {
        // BLAKE3 produces 256-bit (32-byte) hashes = 64 hex characters
        if hex.len() >= 12 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(());
        }
    }
    Err(SceneError::InvalidHashFormat {
        resource: resource.to_string(),
        hash: hash.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    // =========================================================================
    // Basic Parsing Tests
    // =========================================================================

    const MINIMAL_SCENE: &str = r##"
prs_version: "1.0"

metadata:
  name: "hello-world"

layout:
  type: flex
  direction: column

widgets:
  - id: greeting
    type: markdown
    config:
      content: "# Hello, Presentar!"
"##;

    #[test]
    fn test_parse_minimal_scene() {
        let scene = Scene::from_yaml(MINIMAL_SCENE).unwrap();
        assert_eq!(scene.prs_version, "1.0");
        assert_eq!(scene.metadata.name, "hello-world");
        assert_eq!(scene.widgets.len(), 1);
        assert_eq!(scene.widgets[0].id, "greeting");
        assert_eq!(scene.widgets[0].widget_type, WidgetType::Markdown);
    }

    #[test]
    fn test_parse_layout_flex() {
        let scene = Scene::from_yaml(MINIMAL_SCENE).unwrap();
        assert_eq!(scene.layout.layout_type, LayoutType::Flex);
        assert_eq!(scene.layout.direction, Some(FlexDirection::Column));
    }

    #[test]
    fn test_parse_widget_config() {
        let scene = Scene::from_yaml(MINIMAL_SCENE).unwrap();
        let widget = &scene.widgets[0];
        assert_eq!(
            widget.config.content.as_deref(),
            Some("# Hello, Presentar!")
        );
    }

    // =========================================================================
    // Full Scene Parsing Tests
    // =========================================================================

    const FULL_SCENE: &str = r##"
prs_version: "1.0"

metadata:
  name: "sentiment-analysis-demo"
  title: "Real-time Sentiment Analysis"
  description: "Interactive sentiment classifier with confidence visualization"
  author: "alice@example.com"
  created: "2025-12-06T10:00:00Z"
  license: "MIT"
  tags: ["nlp", "sentiment", "demo"]

resources:
  models:
    sentiment_model:
      type: apr
      source: "https://registry.paiml.com/models/sentiment-bert-q4.apr"
      hash: "blake3:a1b2c3d4e5f6789012345678901234567890123456789012345678901234"
      size_bytes: 45000000

  datasets:
    examples:
      type: ald
      source: "./data/sentiment-examples.ald"

layout:
  type: grid
  columns: 2
  rows: 2
  gap: 16

widgets:
  - id: text_input
    type: textbox
    position: { row: 0, col: 0, colspan: 2 }
    config:
      label: "Enter text to analyze"
      placeholder: "Type a sentence..."
      max_length: 512

  - id: sentiment_chart
    type: bar_chart
    position: { row: 1, col: 0 }
    config:
      title: "Sentiment Scores"
      data: "{{ inference.sentiment_model | select('scores') }}"
      x_axis: "{{ ['Positive', 'Negative', 'Neutral'] }}"

  - id: confidence_gauge
    type: gauge
    position: { row: 1, col: 1 }
    config:
      value: "{{ inference.sentiment_model | select('confidence') | percentage }}"
      min: 0
      max: 100
      thresholds:
        - { value: 50, color: "red" }
        - { value: 75, color: "yellow" }
        - { value: 100, color: "green" }

bindings:
  - trigger: "text_input.change"
    debounce_ms: 300
    actions:
      - target: inference.sentiment_model
        input: "{{ text_input.value }}"
      - target: sentiment_chart
        action: refresh
      - target: confidence_gauge
        action: refresh

theme:
  preset: "dark"
  custom:
    primary_color: "#4A90D9"
    font_family: "Inter, sans-serif"

permissions:
  network:
    - "https://registry.paiml.com/*"
  filesystem: []
  clipboard: false
"##;

    #[test]
    fn test_parse_full_scene() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        assert_eq!(scene.prs_version, "1.0");
        assert_eq!(scene.metadata.name, "sentiment-analysis-demo");
        assert_eq!(
            scene.metadata.title,
            Some("Real-time Sentiment Analysis".to_string())
        );
        assert_eq!(scene.metadata.tags.len(), 3);
    }

    #[test]
    fn test_parse_resources() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        assert_eq!(scene.resources.models.len(), 1);
        assert_eq!(scene.resources.datasets.len(), 1);

        let model = scene.get_model("sentiment_model").unwrap();
        assert_eq!(model.resource_type, ModelType::Apr);
        assert!(model.hash.is_some());
        assert_eq!(model.size_bytes, Some(45_000_000));
    }

    #[test]
    fn test_parse_grid_layout() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        assert_eq!(scene.layout.layout_type, LayoutType::Grid);
        assert_eq!(scene.layout.columns, Some(2));
        assert_eq!(scene.layout.rows, Some(2));
        assert_eq!(scene.layout.gap, 16);
    }

    #[test]
    fn test_parse_widget_positions() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();

        let text_input = scene.get_widget("text_input").unwrap();
        let pos = text_input.position.as_ref().unwrap();
        assert_eq!(pos.row, 0);
        assert_eq!(pos.col, 0);
        assert_eq!(pos.colspan, 2);

        let chart = scene.get_widget("sentiment_chart").unwrap();
        let pos = chart.position.as_ref().unwrap();
        assert_eq!(pos.row, 1);
        assert_eq!(pos.col, 0);
    }

    #[test]
    fn test_parse_bindings() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        assert_eq!(scene.bindings.len(), 1);

        let binding = &scene.bindings[0];
        assert_eq!(binding.trigger, "text_input.change");
        assert_eq!(binding.debounce_ms, Some(300));
        assert_eq!(binding.actions.len(), 3);
    }

    #[test]
    fn test_parse_theme() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        let theme = scene.theme.as_ref().unwrap();
        assert_eq!(theme.preset, Some("dark".to_string()));
        assert_eq!(
            theme.custom.get("primary_color"),
            Some(&"#4A90D9".to_string())
        );
    }

    #[test]
    fn test_parse_permissions() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        assert_eq!(scene.permissions.network.len(), 1);
        assert!(scene.permissions.filesystem.is_empty());
        assert!(!scene.permissions.clipboard);
    }

    // =========================================================================
    // Widget Type Tests
    // =========================================================================

    #[test]
    fn test_widget_types() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "widget-test"
layout:
  type: flex
widgets:
  - id: w1
    type: textbox
  - id: w2
    type: slider
  - id: w3
    type: dropdown
  - id: w4
    type: button
  - id: w5
    type: image
  - id: w6
    type: bar_chart
  - id: w7
    type: line_chart
  - id: w8
    type: gauge
  - id: w9
    type: table
  - id: w10
    type: markdown
  - id: w11
    type: inference
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(scene.widgets.len(), 11);
        assert_eq!(scene.widgets[0].widget_type, WidgetType::Textbox);
        assert_eq!(scene.widgets[1].widget_type, WidgetType::Slider);
        assert_eq!(scene.widgets[2].widget_type, WidgetType::Dropdown);
        assert_eq!(scene.widgets[3].widget_type, WidgetType::Button);
        assert_eq!(scene.widgets[4].widget_type, WidgetType::Image);
        assert_eq!(scene.widgets[5].widget_type, WidgetType::BarChart);
        assert_eq!(scene.widgets[6].widget_type, WidgetType::LineChart);
        assert_eq!(scene.widgets[7].widget_type, WidgetType::Gauge);
        assert_eq!(scene.widgets[8].widget_type, WidgetType::Table);
        assert_eq!(scene.widgets[9].widget_type, WidgetType::Markdown);
        assert_eq!(scene.widgets[10].widget_type, WidgetType::Inference);
    }

    // =========================================================================
    // Resource Source Tests
    // =========================================================================

    #[test]
    fn test_resource_source_single() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
resources:
  models:
    model:
      type: apr
      source: "./local/model.apr"
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        let model = scene.get_model("model").unwrap();
        assert_eq!(model.source.primary(), "./local/model.apr");
        assert_eq!(model.source.sources().len(), 1);
    }

    #[test]
    fn test_resource_source_multiple() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
resources:
  models:
    model:
      type: apr
      source:
        - "./local-cache/model.apr"
        - "https://cdn.example.com/model.apr"
      hash: "blake3:a1b2c3d4e5f6789012345678901234567890123456789012345678901234"
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        let model = scene.get_model("model").unwrap();
        assert_eq!(model.source.primary(), "./local-cache/model.apr");
        assert_eq!(model.source.sources().len(), 2);
    }

    // =========================================================================
    // Gauge Threshold Tests
    // =========================================================================

    #[test]
    fn test_gauge_thresholds() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        let gauge = scene.get_widget("confidence_gauge").unwrap();
        let thresholds = gauge.config.thresholds.as_ref().unwrap();

        assert_eq!(thresholds.len(), 3);
        assert!((thresholds[0].value - 50.0).abs() < f64::EPSILON);
        assert_eq!(thresholds[0].color, "red");
        assert!((thresholds[1].value - 75.0).abs() < f64::EPSILON);
        assert_eq!(thresholds[1].color, "yellow");
    }

    // =========================================================================
    // Validation Tests
    // =========================================================================

    #[test]
    fn test_validation_invalid_version() {
        let yaml = r#"
prs_version: "invalid"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SceneError::InvalidVersion(_)));
    }

    #[test]
    fn test_validation_invalid_version_format() {
        let yaml = r#"
prs_version: "1.0.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SceneError::InvalidVersion(_)));
    }

    #[test]
    fn test_validation_invalid_metadata_name_uppercase() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "Invalid-Name"
layout:
  type: flex
widgets: []
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SceneError::InvalidMetadataName(_)
        ));
    }

    #[test]
    fn test_validation_invalid_metadata_name_leading_hyphen() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "-invalid"
layout:
  type: flex
widgets: []
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_duplicate_widget_ids() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets:
  - id: same_id
    type: textbox
  - id: same_id
    type: button
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SceneError::DuplicateWidgetId(_)
        ));
    }

    #[test]
    fn test_validation_invalid_binding_target() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets:
  - id: input
    type: textbox
bindings:
  - trigger: "input.change"
    actions:
      - target: nonexistent_widget
        action: refresh
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SceneError::InvalidBindingTarget { .. }
        ));
    }

    #[test]
    fn test_validation_valid_binding_to_widget() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets:
  - id: input
    type: textbox
  - id: output
    type: markdown
bindings:
  - trigger: "input.change"
    actions:
      - target: output
        action: refresh
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_valid_binding_to_inference() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets:
  - id: input
    type: textbox
resources:
  models:
    my_model:
      type: apr
      source: "./model.apr"
bindings:
  - trigger: "input.change"
    actions:
      - target: inference.my_model
        input: "{{ input.value }}"
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_missing_remote_hash() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
resources:
  models:
    model:
      type: apr
      source: "https://example.com/model.apr"
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SceneError::MissingRemoteHash { .. }
        ));
    }

    #[test]
    fn test_validation_local_resource_no_hash_ok() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
resources:
  models:
    model:
      type: apr
      source: "./local/model.apr"
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_invalid_hash_format() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
resources:
  models:
    model:
      type: apr
      source: "./model.apr"
      hash: "sha256:invalid"
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SceneError::InvalidHashFormat { .. }
        ));
    }

    #[test]
    fn test_validation_grid_layout_requires_columns() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: grid
widgets: []
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SceneError::LayoutError(_)));
    }

    #[test]
    fn test_validation_absolute_layout_requires_dimensions() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: absolute
widgets: []
"#;

        let result = Scene::from_yaml(yaml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SceneError::LayoutError(_)));
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_roundtrip() {
        let scene = Scene::from_yaml(MINIMAL_SCENE).unwrap();
        let yaml = scene.to_yaml().unwrap();
        let scene2 = Scene::from_yaml(&yaml).unwrap();
        assert_eq!(scene.prs_version, scene2.prs_version);
        assert_eq!(scene.metadata.name, scene2.metadata.name);
        assert_eq!(scene.widgets.len(), scene2.widgets.len());
    }

    #[test]
    fn test_roundtrip_full() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        let yaml = scene.to_yaml().unwrap();
        let scene2 = Scene::from_yaml(&yaml).unwrap();
        assert_eq!(scene.prs_version, scene2.prs_version);
        assert_eq!(scene.metadata.name, scene2.metadata.name);
        assert_eq!(scene.resources.models.len(), scene2.resources.models.len());
        assert_eq!(scene.widgets.len(), scene2.widgets.len());
        assert_eq!(scene.bindings.len(), scene2.bindings.len());
    }

    // =========================================================================
    // Helper Method Tests
    // =========================================================================

    #[test]
    fn test_widget_ids() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        let ids = scene.widget_ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"text_input"));
        assert!(ids.contains(&"sentiment_chart"));
        assert!(ids.contains(&"confidence_gauge"));
    }

    #[test]
    fn test_get_widget() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        let widget = scene.get_widget("text_input");
        assert!(widget.is_some());
        assert_eq!(widget.unwrap().widget_type, WidgetType::Textbox);

        let missing = scene.get_widget("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_model() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        let model = scene.get_model("sentiment_model");
        assert!(model.is_some());
        assert_eq!(model.unwrap().resource_type, ModelType::Apr);
    }

    #[test]
    fn test_get_dataset() {
        let scene = Scene::from_yaml(FULL_SCENE).unwrap();
        let dataset = scene.get_dataset("examples");
        assert!(dataset.is_some());
        assert_eq!(dataset.unwrap().resource_type, DatasetType::Ald);
    }

    // =========================================================================
    // Error Display Tests
    // =========================================================================

    #[test]
    fn test_error_display_yaml() {
        let yaml_err: serde_yaml::Error =
            serde_yaml::from_str::<serde_yaml::Value>("{{").unwrap_err();
        let err = SceneError::Yaml(yaml_err);
        assert!(err.to_string().contains("YAML error"));
    }

    #[test]
    fn test_error_display_invalid_version() {
        let err = SceneError::InvalidVersion("bad".to_string());
        assert_eq!(err.to_string(), "Invalid prs_version: bad");
    }

    #[test]
    fn test_error_display_duplicate_id() {
        let err = SceneError::DuplicateWidgetId("my_id".to_string());
        assert_eq!(err.to_string(), "Duplicate widget id: my_id");
    }

    #[test]
    fn test_error_display_invalid_binding() {
        let err = SceneError::InvalidBindingTarget {
            trigger: "input.change".to_string(),
            target: "bad_target".to_string(),
        };
        assert!(err.to_string().contains("Invalid binding target"));
        assert!(err.to_string().contains("bad_target"));
    }

    #[test]
    fn test_error_display_invalid_hash() {
        let err = SceneError::InvalidHashFormat {
            resource: "model".to_string(),
            hash: "bad".to_string(),
        };
        assert!(err.to_string().contains("Invalid hash format"));
    }

    #[test]
    fn test_error_display_missing_hash() {
        let err = SceneError::MissingRemoteHash {
            resource: "model".to_string(),
        };
        assert!(err.to_string().contains("Missing hash for remote resource"));
    }

    #[test]
    fn test_error_source() {
        let yaml_err: serde_yaml::Error =
            serde_yaml::from_str::<serde_yaml::Value>("{{").unwrap_err();
        let err = SceneError::Yaml(yaml_err);
        assert!(err.source().is_some());

        let err2 = SceneError::InvalidVersion("x".to_string());
        assert!(err2.source().is_none());
    }

    // =========================================================================
    // Model Type Tests
    // =========================================================================

    #[test]
    fn test_model_types() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
resources:
  models:
    apr_model:
      type: apr
      source: "./model.apr"
    gguf_model:
      type: gguf
      source: "./model.gguf"
    safetensors_model:
      type: safetensors
      source: "./model.safetensors"
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(
            scene.get_model("apr_model").unwrap().resource_type,
            ModelType::Apr
        );
        assert_eq!(
            scene.get_model("gguf_model").unwrap().resource_type,
            ModelType::Gguf
        );
        assert_eq!(
            scene.get_model("safetensors_model").unwrap().resource_type,
            ModelType::Safetensors
        );
    }

    #[test]
    fn test_dataset_types() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
resources:
  datasets:
    ald_data:
      type: ald
      source: "./data.ald"
    parquet_data:
      type: parquet
      source: "./data.parquet"
    csv_data:
      type: csv
      source: "./data.csv"
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(
            scene.get_dataset("ald_data").unwrap().resource_type,
            DatasetType::Ald
        );
        assert_eq!(
            scene.get_dataset("parquet_data").unwrap().resource_type,
            DatasetType::Parquet
        );
        assert_eq!(
            scene.get_dataset("csv_data").unwrap().resource_type,
            DatasetType::Csv
        );
    }

    // =========================================================================
    // Layout Type Tests
    // =========================================================================

    #[test]
    fn test_layout_type_grid() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: grid
  columns: 3
  rows: 2
  gap: 8
widgets: []
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(scene.layout.layout_type, LayoutType::Grid);
        assert_eq!(scene.layout.columns, Some(3));
        assert_eq!(scene.layout.rows, Some(2));
        assert_eq!(scene.layout.gap, 8);
    }

    #[test]
    fn test_layout_type_flex() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
  direction: row
  wrap: true
  gap: 4
widgets: []
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(scene.layout.layout_type, LayoutType::Flex);
        assert_eq!(scene.layout.direction, Some(FlexDirection::Row));
        assert_eq!(scene.layout.wrap, Some(true));
    }

    #[test]
    fn test_layout_type_absolute() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: absolute
  width: 1200
  height: 800
widgets: []
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(scene.layout.layout_type, LayoutType::Absolute);
        assert_eq!(scene.layout.width, Some(1200));
        assert_eq!(scene.layout.height, Some(800));
    }

    // =========================================================================
    // Default Value Tests
    // =========================================================================

    #[test]
    fn test_default_gap() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(scene.layout.gap, 16); // Default value
    }

    #[test]
    fn test_default_span() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: grid
  columns: 2
widgets:
  - id: widget
    type: textbox
    position: { row: 0, col: 0 }
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        let pos = scene.widgets[0].position.as_ref().unwrap();
        assert_eq!(pos.colspan, 1); // Default
        assert_eq!(pos.rowspan, 1); // Default
    }

    // =========================================================================
    // Image Classifier Example (from spec)
    // =========================================================================

    #[test]
    fn test_image_classifier_example() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "image-classifier"
  title: "CIFAR-10 Classifier"

resources:
  models:
    classifier:
      type: apr
      source: "https://registry.paiml.com/models/cifar10-resnet.apr"
      hash: "blake3:abc123def456789012345678901234567890123456789012345678901234"

layout:
  type: grid
  columns: 2
  rows: 1

widgets:
  - id: image_upload
    type: image
    position: { row: 0, col: 0 }
    config:
      mode: upload
      accept: ["image/png", "image/jpeg"]

  - id: predictions
    type: bar_chart
    position: { row: 0, col: 1 }
    config:
      title: "Predictions"
      data: "{{ inference.classifier | select('probabilities') }}"
      x_axis: "{{ ['airplane', 'automobile', 'bird', 'cat', 'deer', 'dog', 'frog', 'horse', 'ship', 'truck'] }}"

bindings:
  - trigger: image_upload.change
    actions:
      - target: inference.classifier
        input: "{{ image_upload.data }}"
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(scene.metadata.name, "image-classifier");
        assert_eq!(scene.widgets.len(), 2);

        let upload = scene.get_widget("image_upload").unwrap();
        assert_eq!(upload.widget_type, WidgetType::Image);
        assert_eq!(upload.config.mode, Some("upload".to_string()));
        assert_eq!(
            upload.config.accept,
            Some(vec!["image/png".to_string(), "image/jpeg".to_string()])
        );
    }

    // =========================================================================
    // Data Explorer Example (from spec)
    // =========================================================================

    #[test]
    fn test_data_explorer_example() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "data-explorer"

resources:
  datasets:
    sales:
      type: ald
      source: "./data/sales-2024.ald"
      hash: "blake3:789abc012345678901234567890123456789012345678901234567890123"

layout:
  type: flex
  direction: column

widgets:
  - id: filters
    type: dropdown
    config:
      label: "Region"
      options: "{{ dataset.sales | select('region') | unique }}"

  - id: chart
    type: line_chart
    config:
      title: "Sales Over Time"
      data: "{{ dataset.sales | filter('region == filters.value') }}"
      x_axis: date
      y_axis: revenue

  - id: table
    type: table
    config:
      data: "{{ dataset.sales | filter('region == filters.value') | limit(100) }}"
      columns: ["date", "region", "product", "revenue"]
      sortable: true
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(scene.metadata.name, "data-explorer");
        assert_eq!(scene.widgets.len(), 3);

        let table = scene.get_widget("table").unwrap();
        assert_eq!(table.widget_type, WidgetType::Table);
        assert_eq!(table.config.sortable, Some(true));
        assert_eq!(
            table.config.columns,
            Some(vec![
                "date".to_string(),
                "region".to_string(),
                "product".to_string(),
                "revenue".to_string()
            ])
        );
    }

    // =========================================================================
    // Slider Widget Tests
    // =========================================================================

    #[test]
    fn test_slider_widget() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets:
  - id: temperature
    type: slider
    config:
      label: "Temperature"
      min: 0.0
      max: 2.0
      step: 0.1
      default: 0.7
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        let slider = scene.get_widget("temperature").unwrap();
        assert_eq!(slider.widget_type, WidgetType::Slider);
        assert_eq!(slider.config.min, Some(0.0));
        assert_eq!(slider.config.max, Some(2.0));
        assert_eq!(slider.config.step, Some(0.1));
        assert_eq!(slider.config.default, Some(0.7));
    }

    // =========================================================================
    // Multiple Binding Actions Tests
    // =========================================================================

    #[test]
    fn test_multiple_binding_actions() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets:
  - id: input
    type: textbox
  - id: chart1
    type: bar_chart
  - id: chart2
    type: line_chart
  - id: label
    type: markdown
bindings:
  - trigger: input.submit
    actions:
      - target: chart1
        action: refresh
      - target: chart2
        action: refresh
      - target: label
        action: refresh
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert_eq!(scene.bindings[0].actions.len(), 3);
    }

    // =========================================================================
    // Empty Scene Tests
    // =========================================================================

    #[test]
    fn test_empty_widgets() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "empty"
layout:
  type: flex
widgets: []
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert!(scene.widgets.is_empty());
    }

    #[test]
    fn test_empty_resources() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert!(scene.resources.models.is_empty());
        assert!(scene.resources.datasets.is_empty());
    }

    #[test]
    fn test_empty_bindings() {
        let yaml = r#"
prs_version: "1.0"
metadata:
  name: "test"
layout:
  type: flex
widgets: []
"#;

        let scene = Scene::from_yaml(yaml).unwrap();
        assert!(scene.bindings.is_empty());
    }
}
