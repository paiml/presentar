# Charts

Data visualization with Chart widget.

## Chart Types

| Type | Use Case |
|------|----------|
| Line | Trends over time |
| Bar | Category comparison |
| Pie | Part of whole |
| Scatter | Correlation |
| Area | Cumulative values |

## Basic Line Chart

```yaml
widgets:
  - type: Chart
    chart_type: line
    data:
      - [0, 10]
      - [1, 20]
      - [2, 15]
      - [3, 30]
    x_label: "Time"
    y_label: "Value"
```

## Bar Chart

```yaml
widgets:
  - type: Chart
    chart_type: bar
    data:
      - { label: "A", value: 30 }
      - { label: "B", value: 50 }
      - { label: "C", value: 20 }
    colors:
      - "#4285f4"
      - "#ea4335"
      - "#fbbc05"
```

## Data Binding

```yaml
data:
  sales:
    source: "sales.ald"

widgets:
  - type: Chart
    chart_type: line
    data: "{{ sales | select('date', 'revenue') }}"
```

## Styling

| Property | Description |
|----------|-------------|
| `colors` | Series colors |
| `grid` | Show grid lines |
| `legend` | Legend position |
| `axis_*` | Axis configuration |

## Responsive

```yaml
widgets:
  - type: Chart
    responsive: true
    aspect_ratio: 16:9
```

## Verified Test

```rust
#[test]
fn test_charts_data_point() {
    // Chart data point structure
    #[derive(Debug, PartialEq)]
    struct DataPoint {
        x: f32,
        y: f32,
    }

    let points = vec![
        DataPoint { x: 0.0, y: 10.0 },
        DataPoint { x: 1.0, y: 20.0 },
        DataPoint { x: 2.0, y: 15.0 },
    ];

    assert_eq!(points.len(), 3);
    assert_eq!(points[1].y, 20.0);

    // Find min/max for axis scaling
    let max_y = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert_eq!(max_y, 20.0);
}
```
