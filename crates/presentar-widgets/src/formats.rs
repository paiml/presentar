//! Conversion from `.apr`/`.ald` file formats to display widgets.
//!
//! Bridges `presentar-yaml` format loaders to `ModelCard` and `DataCard` widgets.
#![allow(clippy::doc_markdown)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::redundant_closure_for_method_calls)]

use crate::data_card::{DataCard, DataColumn};
use crate::model_card::{ModelCard, ModelMetric, ModelStatus};
use presentar_yaml::formats::{AldDataset, AprModel, DType};

/// Extension trait for converting `AprModel` to `ModelCard`.
pub trait AprModelExt {
    /// Convert to a display-ready `ModelCard` widget.
    fn to_model_card(&self) -> ModelCard;
}

impl AprModelExt for AprModel {
    fn to_model_card(&self) -> ModelCard {
        // Count total parameters
        let total_params: u64 = self
            .layers
            .iter()
            .flat_map(|l| l.parameters.iter())
            .map(|t| t.numel() as u64)
            .sum();

        // Build layer summary for description
        let layer_summary: String = self
            .layers
            .iter()
            .map(|l| format!("{}({})", l.layer_type, l.parameters.len()))
            .collect::<Vec<_>>()
            .join(" → ");

        let mut card = ModelCard::new(&self.model_type)
            .version(format!("v{}", self.version))
            .framework("Aprender")
            .parameters(total_params)
            .status(ModelStatus::Published);

        if !layer_summary.is_empty() {
            card = card.description(format!("Architecture: {}", layer_summary));
        }

        // Add metrics from metadata
        if let Some(acc) = self.metadata.get("accuracy") {
            if let Ok(v) = acc.parse::<f64>() {
                card = card.metric(ModelMetric::new("accuracy", v));
            }
        }
        if let Some(loss) = self.metadata.get("loss") {
            if let Ok(v) = loss.parse::<f64>() {
                card = card.metric(ModelMetric::new("loss", v).lower_is_better());
            }
        }
        if let Some(f1) = self.metadata.get("f1_score") {
            if let Ok(v) = f1.parse::<f64>() {
                card = card.metric(ModelMetric::new("F1", v));
            }
        }

        // Add task if in metadata
        if let Some(task) = self.metadata.get("task") {
            card = card.task(task);
        }

        // Add dataset if in metadata
        if let Some(dataset) = self.metadata.get("dataset") {
            card = card.dataset(dataset);
        }

        // Add author if in metadata
        if let Some(author) = self.metadata.get("author") {
            card = card.author(author);
        }

        // Add remaining metadata
        for (k, v) in &self.metadata {
            if !["accuracy", "loss", "f1_score", "task", "dataset", "author"].contains(&k.as_str())
            {
                card = card.metadata_entry(k, v);
            }
        }

        card.tag("apr").tag("sovereign-ai")
    }
}

/// Extension trait for converting `AldDataset` to `DataCard`.
pub trait AldDatasetExt {
    /// Convert to a display-ready `DataCard` widget.
    fn to_data_card(&self, name: &str) -> DataCard;
}

impl AldDatasetExt for AldDataset {
    fn to_data_card(&self, name: &str) -> DataCard {
        // Calculate total size
        let total_bytes: usize = self.tensors.iter().map(|t| t.data.len()).sum();

        // Count total elements
        let total_elements: usize = self.tensors.iter().map(|t| t.numel()).sum();

        // Build columns from tensors
        let columns: Vec<DataColumn> = self
            .tensors
            .iter()
            .map(|t| {
                let dtype_str = match t.dtype {
                    DType::F32 => "float32",
                    DType::F64 => "float64",
                    DType::I32 => "int32",
                    DType::I64 => "int64",
                    DType::U8 => "uint8",
                };
                let shape_str = t
                    .shape
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join("×");

                DataColumn::new(&t.name, dtype_str).description(format!("Shape: [{}]", shape_str))
            })
            .collect();

        let mut card = DataCard::new(name)
            .description(format!(
                "{} elements, {} tensors, {}",
                total_elements,
                self.tensors.len(),
                format_bytes(total_bytes)
            ))
            .source("Alimentar (.ald)")
            .tag("ald")
            .tag("sovereign-ai");

        for col in columns {
            card = card.column(col);
        }

        card
    }
}

/// Format bytes as human-readable string.
fn format_bytes(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;
    const GB: usize = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Load an .apr file and convert to ModelCard.
///
/// # Errors
///
/// Returns error if file cannot be loaded.
pub fn load_apr_as_card(data: &[u8]) -> Result<ModelCard, presentar_yaml::FormatError> {
    let model = AprModel::load(data)?;
    Ok(model.to_model_card())
}

/// Load an .ald file and convert to DataCard.
///
/// # Errors
///
/// Returns error if file cannot be loaded.
pub fn load_ald_as_card(data: &[u8], name: &str) -> Result<DataCard, presentar_yaml::FormatError> {
    let dataset = AldDataset::load(data)?;
    Ok(dataset.to_data_card(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use presentar_yaml::formats::{ModelLayer, Tensor};

    #[test]
    fn test_apr_to_model_card() {
        let mut model = AprModel::new("LinearRegression");
        model.layers.push(ModelLayer {
            layer_type: "dense".to_string(),
            parameters: vec![Tensor::from_f32("weights", vec![10, 5], &[0.0; 50])],
        });
        model
            .metadata
            .insert("accuracy".to_string(), "0.95".to_string());
        model
            .metadata
            .insert("task".to_string(), "classification".to_string());

        let card = model.to_model_card();

        assert_eq!(card.get_name(), "LinearRegression");
        assert_eq!(card.get_framework(), Some("Aprender"));
        assert_eq!(card.get_parameters(), Some(50));
        assert!(card.get_tags().contains(&"apr".to_string()));
    }

    #[test]
    fn test_ald_to_data_card() {
        let mut dataset = AldDataset::new();
        dataset.add_tensor(Tensor::from_f32("features", vec![100, 10], &[0.0; 1000]));
        dataset.add_tensor(Tensor::from_f32("labels", vec![100], &[0.0; 100]));

        let card = dataset.to_data_card("mnist_sample");

        assert_eq!(card.get_name(), "mnist_sample");
        assert_eq!(card.column_count(), 2);
        assert!(card.get_tags().contains(&"ald".to_string()));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_load_apr_roundtrip() {
        let mut model = AprModel::new("MLP");
        model.layers.push(ModelLayer {
            layer_type: "dense".to_string(),
            parameters: vec![Tensor::from_f32("w", vec![4, 4], &[1.0; 16])],
        });

        let bytes = model.save();
        let card = load_apr_as_card(&bytes).unwrap();

        assert_eq!(card.get_name(), "MLP");
    }

    #[test]
    fn test_load_ald_roundtrip() {
        let mut dataset = AldDataset::new();
        dataset.add_tensor(Tensor::from_f32("data", vec![10], &[1.0; 10]));

        let bytes = dataset.save();
        let card = load_ald_as_card(&bytes, "test_data").unwrap();

        assert_eq!(card.get_name(), "test_data");
    }
}
