//! YAML manifest types for Presentar applications.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Application manifest loaded from app.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Presentar version
    pub presentar: String,
    /// Application name
    pub name: String,
    /// Application version
    pub version: String,
    /// Application description
    #[serde(default)]
    pub description: String,
    /// Quality score (auto-computed)
    #[serde(default)]
    pub score: Option<Score>,
    /// Data sources
    #[serde(default)]
    pub data: HashMap<String, DataSource>,
    /// Model references
    #[serde(default)]
    pub models: HashMap<String, ModelRef>,
    /// Layout configuration
    pub layout: LayoutConfig,
    /// Interactions
    #[serde(default)]
    pub interactions: Vec<Interaction>,
    /// Theme configuration
    #[serde(default)]
    pub theme: Option<ThemeConfig>,
}

/// Quality score metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    /// Letter grade (F-A+)
    pub grade: String,
    /// Numeric score (0-100)
    pub value: f64,
    /// Test coverage percentage
    #[serde(default)]
    pub coverage: Option<f64>,
}

/// Data source configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSource {
    /// Source URI (pacha://, file://, https://)
    pub source: String,
    /// Data format (ald, csv, json)
    #[serde(default = "default_format")]
    pub format: String,
    /// Refresh interval
    #[serde(default)]
    pub refresh: Option<String>,
}

fn default_format() -> String {
    "ald".to_string()
}

/// Model reference configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRef {
    /// Source URI (pacha://, file://)
    pub source: String,
    /// Model format (apr)
    #[serde(default = "default_model_format")]
    pub format: String,
}

fn default_model_format() -> String {
    "apr".to_string()
}

/// Layout configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Layout type (dashboard, app, custom)
    #[serde(rename = "type")]
    pub layout_type: String,
    /// Number of columns for grid layout
    #[serde(default = "default_columns")]
    pub columns: u32,
    /// Row configuration
    #[serde(default)]
    pub rows: String,
    /// Gap between sections
    #[serde(default = "default_gap")]
    pub gap: u32,
    /// Layout sections
    #[serde(default)]
    pub sections: Vec<Section>,
}

fn default_columns() -> u32 {
    12
}

fn default_gap() -> u32 {
    16
}

/// Layout section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Section ID
    pub id: String,
    /// Grid span [start, end]
    #[serde(default)]
    pub span: Option<[u32; 2]>,
    /// Widgets in this section
    #[serde(default)]
    pub widgets: Vec<WidgetConfig>,
}

/// Widget configuration from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetConfig {
    /// Widget type (text, button, chart, data-table, etc.)
    #[serde(rename = "type")]
    pub widget_type: String,
    /// Widget ID
    #[serde(default)]
    pub id: Option<String>,
    /// Content (for text widgets)
    #[serde(default)]
    pub content: Option<String>,
    /// Style name
    #[serde(default)]
    pub style: Option<String>,
    /// Data binding expression
    #[serde(default)]
    pub data: Option<String>,
    /// Chart type (for chart widgets)
    #[serde(default)]
    pub chart_type: Option<String>,
    /// X axis field
    #[serde(default)]
    pub x: Option<String>,
    /// Y axis field
    #[serde(default)]
    pub y: Option<String>,
    /// Color field
    #[serde(default)]
    pub color: Option<String>,
    /// Additional properties
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

/// Interaction configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interaction {
    /// Event trigger
    pub trigger: String,
    /// Action type
    pub action: String,
    /// Target (for navigation)
    #[serde(default)]
    pub target: Option<String>,
    /// Content (for tooltips)
    #[serde(default)]
    pub content: Option<String>,
    /// Script (for custom actions)
    #[serde(default)]
    pub script: Option<String>,
}

/// Theme configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Theme preset (light, dark)
    #[serde(default)]
    pub preset: Option<String>,
    /// Custom colors
    #[serde(default)]
    pub colors: HashMap<String, String>,
}

impl Manifest {
    /// Parse a manifest from YAML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the YAML is invalid.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Serialize manifest to YAML string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_YAML: &str = r#"
presentar: "0.1"
name: "test-app"
version: "1.0.0"
description: "Test application"

layout:
  type: "dashboard"
  columns: 12
  gap: 16
  sections:
    - id: "header"
      span: [1, 12]
      widgets:
        - type: "text"
          content: "Hello World"
          style: "heading-1"
"#;

    #[test]
    fn test_parse_manifest() {
        let manifest = Manifest::from_yaml(EXAMPLE_YAML).unwrap();
        assert_eq!(manifest.name, "test-app");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.layout.columns, 12);
        assert_eq!(manifest.layout.sections.len(), 1);
    }

    #[test]
    fn test_parse_section() {
        let manifest = Manifest::from_yaml(EXAMPLE_YAML).unwrap();
        let section = &manifest.layout.sections[0];
        assert_eq!(section.id, "header");
        assert_eq!(section.span, Some([1, 12]));
        assert_eq!(section.widgets.len(), 1);
    }

    #[test]
    fn test_parse_widget() {
        let manifest = Manifest::from_yaml(EXAMPLE_YAML).unwrap();
        let widget = &manifest.layout.sections[0].widgets[0];
        assert_eq!(widget.widget_type, "text");
        assert_eq!(widget.content, Some("Hello World".to_string()));
        assert_eq!(widget.style, Some("heading-1".to_string()));
    }

    #[test]
    fn test_roundtrip() {
        let manifest = Manifest::from_yaml(EXAMPLE_YAML).unwrap();
        let yaml = manifest.to_yaml().unwrap();
        let manifest2 = Manifest::from_yaml(&yaml).unwrap();
        assert_eq!(manifest.name, manifest2.name);
        assert_eq!(manifest.version, manifest2.version);
    }

    #[test]
    fn test_data_source() {
        let yaml = r#"
presentar: "0.1"
name: "test"
version: "1.0.0"
data:
  transactions:
    source: "pacha://datasets/transactions:latest"
    format: "ald"
    refresh: "5m"
layout:
  type: "app"
"#;

        let manifest = Manifest::from_yaml(yaml).unwrap();
        assert!(manifest.data.contains_key("transactions"));
        let ds = &manifest.data["transactions"];
        assert_eq!(ds.format, "ald");
        assert_eq!(ds.refresh, Some("5m".to_string()));
    }

    #[test]
    fn test_model_ref() {
        let yaml = r#"
presentar: "0.1"
name: "test"
version: "1.0.0"
models:
  classifier:
    source: "pacha://models/classifier:1.0.0"
    format: "apr"
layout:
  type: "app"
"#;

        let manifest = Manifest::from_yaml(yaml).unwrap();
        assert!(manifest.models.contains_key("classifier"));
        let model = &manifest.models["classifier"];
        assert_eq!(model.format, "apr");
    }
}
