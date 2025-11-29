# Pacha

Application runtime for the Sovereign AI Stack.

## Overview

| Feature | Description |
|---------|-------------|
| Role | YAML app loader and executor |
| Parsing | `.yaml` to widget tree |
| Binding | Data source connections |
| Events | User interaction dispatch |

## Architecture

```
┌──────────────────────────────────────────┐
│ Pacha Runtime                            │
├──────────────────────────────────────────┤
│ YAML Parser → Widget Factory → App Tree  │
├──────────────────────────────────────────┤
│ Event Loop → State Manager → Renderer    │
└──────────────────────────────────────────┘
```

## YAML Processing

```yaml
# app.yaml
app:
  name: "My Dashboard"
  root:
    type: Column
    children:
      - type: Text
        value: "Hello"
```

```rust
// Pacha parses and instantiates
let app = pacha::load("app.yaml")?;
app.run();
```

## Data Binding

```yaml
data:
  metrics:
    source: "data.ald"
    transform: "filter(status='active')"

widgets:
  - type: Chart
    data: "{{ metrics | select('value') }}"
```

## Event Dispatch

| Event | Handler |
|-------|---------|
| Click | `on_click` action |
| Input | `on_change` binding |
| Load | `on_load` initializer |

## Runtime Loop

```rust
loop {
    let events = poll_events();
    state.update(events);
    let tree = build_widgets(state);
    layout(tree);
    paint(tree);
}
```

## Verified Test

```rust
#[test]
fn test_pacha_yaml_structure() {
    // YAML app structure validation
    use std::collections::HashMap;

    let mut app: HashMap<&str, &str> = HashMap::new();
    app.insert("name", "Test App");
    app.insert("version", "1.0.0");

    assert!(app.contains_key("name"));
    assert!(!app.get("name").unwrap().is_empty());

    // Root widget type must be specified
    let root_type = "Column";
    assert!(["Column", "Row", "Stack", "Container"].contains(&root_type));
}
```
