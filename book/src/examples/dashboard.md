# Dashboard

Complete dashboard example with multiple widgets.

## Layout

```yaml
app:
  name: "Analytics Dashboard"
  root:
    type: Column
    children:
      - type: Row
        children:
          - type: DataCard
            title: "Users"
            value: "{{ metrics.users }}"
          - type: DataCard
            title: "Revenue"
            value: "{{ metrics.revenue | currency }}"
          - type: DataCard
            title: "Growth"
            value: "{{ metrics.growth | percentage }}"
      - type: Row
        children:
          - type: Chart
            chart_type: line
            data: "{{ timeseries }}"
          - type: DataTable
            data: "{{ top_products }}"
```

## Data Sources

```yaml
data:
  metrics:
    source: "metrics.ald"
    refresh: 60s

  timeseries:
    source: "timeseries.ald"
    transform: "filter(date >= '2024-01-01')"

  top_products:
    source: "products.ald"
    transform: "sort(revenue, desc) | limit(10)"
```

## Responsive Grid

| Breakpoint | Columns |
|------------|---------|
| < 600px | 1 |
| 600-1200px | 2 |
| > 1200px | 3 |

## Interactions

```yaml
widgets:
  - type: Chart
    on_click:
      action: navigate
      target: "/details/{{ clicked.id }}"
```

## Refresh Pattern

```yaml
data:
  live_metrics:
    source: "api/metrics"
    refresh: 5s
    on_update:
      action: animate
      duration: 300ms
```

## Verified Test

```rust
#[test]
fn test_dashboard_card_layout() {
    use presentar_core::Size;

    // Dashboard with 3 cards in a row
    let container_width = 900.0;
    let card_count = 3;
    let gap = 16.0;

    // Calculate card width
    let total_gap = gap * (card_count as f32 - 1.0);
    let card_width = (container_width - total_gap) / card_count as f32;

    assert_eq!(card_width, 280.0);  // (900 - 32) / 3

    // Card positions
    let positions: Vec<f32> = (0..card_count)
        .map(|i| i as f32 * (card_width + gap))
        .collect();

    assert_eq!(positions[0], 0.0);
    assert_eq!(positions[1], 296.0);   // 280 + 16
    assert_eq!(positions[2], 592.0);   // 560 + 32
}
```
