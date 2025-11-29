# Model Card Display

Visualize ML model metadata and metrics.

## Model Card Standard

| Field | Required | Description |
|-------|----------|-------------|
| Name | Yes | Model identifier |
| Version | Yes | Semantic version |
| Task | Yes | Classification, regression, etc. |
| Metrics | Yes | Performance numbers |
| Limitations | Yes | Known constraints |
| Training Data | No | Dataset description |
| Intended Use | No | Deployment guidance |

## YAML Configuration

```yaml
app:
  name: "Model Card Viewer"

data:
  model:
    source: "classifier.apr"

widgets:
  root:
    type: ModelCard
    model: "{{ model }}"
    sections:
      - overview
      - metrics
      - limitations
      - training
```

## Widget Structure

```yaml
widgets:
  - type: Column
    children:
      - type: Text
        value: "{{ model.name }}"
        font_size: 24
        font_weight: bold

      - type: Row
        children:
          - type: DataCard
            title: "Accuracy"
            value: "{{ model.metrics.accuracy | percentage }}"
          - type: DataCard
            title: "F1 Score"
            value: "{{ model.metrics.f1 | percentage }}"

      - type: Text
        value: "Limitations"
        font_weight: bold

      - type: Column
        children: "{{ model.limitations | map(limitation_item) }}"
```

## Metrics Visualization

```yaml
widgets:
  - type: Chart
    chart_type: bar
    data:
      - { label: "Precision", value: "{{ model.metrics.precision }}" }
      - { label: "Recall", value: "{{ model.metrics.recall }}" }
      - { label: "F1", value: "{{ model.metrics.f1 }}" }
```

## Fairness Metrics

| Metric | Description |
|--------|-------------|
| Demographic Parity | Equal positive rates |
| Equal Opportunity | Equal true positive rates |
| Calibration | Predicted = actual probability |

## Verified Test

```rust
#[test]
fn test_model_card_validation() {
    // Model card required fields
    struct ModelCard {
        name: String,
        version: String,
        task: String,
        accuracy: f32,
        limitations: Vec<String>,
    }

    impl ModelCard {
        fn is_valid(&self) -> bool {
            !self.name.is_empty()
                && !self.version.is_empty()
                && !self.task.is_empty()
                && self.accuracy >= 0.0
                && self.accuracy <= 1.0
                && !self.limitations.is_empty()
        }
    }

    let card = ModelCard {
        name: "Classifier".to_string(),
        version: "1.0.0".to_string(),
        task: "classification".to_string(),
        accuracy: 0.95,
        limitations: vec!["English only".to_string()],
    };

    assert!(card.is_valid());

    // Empty name is invalid
    let invalid = ModelCard {
        name: "".to_string(),
        ..card
    };
    assert!(!invalid.is_valid());
}
```
