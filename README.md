![Presentar](.github/presentar-hero.svg)

# Presentar

<p align="center">
  <b>WASM-first visualization and rapid application framework for the Sovereign AI Stack.</b>
</p>

<p align="center">
  <a href="https://crates.io/crates/presentar"><img src="https://img.shields.io/crates/v/presentar.svg" alt="Crates.io"></a>
  <a href="https://docs.rs/presentar"><img src="https://docs.rs/presentar/badge.svg" alt="Documentation"></a>
  <a href="https://github.com/paiml/presentar/actions/workflows/ci.yml"><img src="https://github.com/paiml/presentar/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://img.shields.io/badge/coverage-91%25-brightgreen"><img src="https://img.shields.io/badge/coverage-91%25-brightgreen" alt="Coverage"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License"></a>
</p>

---

Presentar provides a WASM-first UI framework for building high-performance visualization and application components. Built on the Sovereign AI Stack, it enables 60fps GPU-accelerated rendering with zero Python dependencies.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [YAML Configuration](#yaml-configuration)
- [Widgets](#widgets)
- [Architecture](#architecture)
- [Testing](#testing)
- [Documentation](#documentation)
- [Related Crates](#related-crates)
- [License](#license)
- [Contributing](#contributing)

## Features

- **WASM-First**: Primary target is `wasm32-unknown-unknown`
- **Brick Architecture**: Tests define interface (PROBAR-SPEC-009) - all widgets implement `Brick` trait
- **Zero Dependencies**: Minimal external crates (winit, fontdue only)
- **60fps Rendering**: GPU-accelerated via WebGPU/WGSL shaders
- **JIDOKA Enforcement**: Rendering blocked if Brick verification fails
- **Accessibility**: Built-in WCAG 2.1 AA compliance checking
- **Declarative**: YAML-driven application configuration
- **Testable**: Zero-dependency test harness with visual regression

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
presentar = "0.1"
presentar-widgets = "0.1"
```

## Quick Start

```rust
use presentar::widgets::{Button, Column, Text};
use presentar::{Brick, Constraints, Size, Widget};

// Build UI tree - all widgets implement Brick trait
let ui = Column::new(vec![
    Box::new(Text::new("Hello, Presentar!")),
    Box::new(Button::new("Click me")),
]);

// Verify before rendering (PROBAR-SPEC-009: Brick Architecture)
assert!(ui.can_render(), "Widget must pass Brick verification");

// Measure and layout
let constraints = Constraints::new(0.0, 800.0, 0.0, 600.0);
let size = ui.measure(&constraints);
```

## Brick Architecture (PROBAR-SPEC-009)

All widgets implement the `Brick` trait, enforcing "tests define interface":

```rust
use presentar::{Brick, BrickAssertion, BrickBudget, Widget};

// Every widget has:
// - assertions(): What the widget promises (TextVisible, ContrastRatio, etc.)
// - budget(): Performance budget (default 16ms for 60fps)
// - verify(): Runtime verification of assertions
// - can_render(): Returns false if verification fails

let button = Button::new("Submit");

// Check assertions
println!("Brick: {}", button.brick_name());
println!("Assertions: {:?}", button.assertions());
println!("Can render: {}", button.can_render());

// JIDOKA: Rendering is blocked if can_render() returns false
```

**Key Benefits:**
- Widgets declare their contracts as falsifiable assertions
- Verification happens before rendering, not after bugs ship
- Performance budgets are enforced at the framework level

## YAML Configuration

```yaml
app:
  name: "My Dashboard"

widgets:
  root:
    type: Column
    children:
      - type: Text
        value: "Hello World"
      - type: Button
        label: "Click"
```

## Showcase Demo: Shell Autocomplete

Real-time shell command autocomplete powered by a trained N-gram model. **Zero infrastructure** - runs entirely in the browser via WASM.

```bash
make serve
# Open http://localhost:8080/shell-autocomplete.html
```

| Metric | Value |
|--------|-------|
| Bundle Size | 574 KB |
| Inference Latency | <1ms |
| Cold Start | <100ms |
| Server Required | None |

**10X faster than Streamlit/Gradio** with zero Python dependencies.

See [docs/specifications/showcase-demo-aprender-shell-apr.md](docs/specifications/showcase-demo-aprender-shell-apr.md) for full specification.

## Widgets

| Widget | Description |
|--------|-------------|
| `Button` | Interactive button with hover/press states |
| `Text` | Text rendering with font configuration |
| `Container` | Layout container with padding/margins |
| `Column` | Vertical flex layout |
| `Row` | Horizontal flex layout |
| `Checkbox` | Toggle checkbox with label |
| `TextInput` | Single-line text input field |
| `Tabs` | Tabbed navigation container |
| `Grid` | CSS Grid-compatible layout |
| `Chart` | Data visualization charts |

## Architecture

```
Layer 9: App Runtime        - YAML parser, Pacha integration
Layer 8: Presentar          - Widget tree, layout engine
Layer 7: Trueno-Viz         - GPU primitives, WGSL shaders
Layer 6: Trueno             - SIMD/GPU tensor operations
```

## Testing

```bash
# Run all tests
cargo test

# Run with coverage
cargo llvm-cov

# Run benchmarks
cargo bench -p presentar-core
```

## Documentation

- [Book](book/) - Comprehensive documentation
- [API Docs](https://docs.rs/presentar) - Rustdoc API reference

## Related Crates

| Crate | Description |
|-------|-------------|
| [`trueno`](https://crates.io/crates/trueno) | SIMD-accelerated tensor operations |
| [`trueno-viz`](https://crates.io/crates/trueno-viz) | GPU rendering primitives |
| [`aprender`](https://crates.io/crates/aprender) | Machine learning algorithms |
| [`presentar-core`](https://crates.io/crates/presentar-core) | Core types and traits |
| [`presentar-widgets`](https://crates.io/crates/presentar-widgets) | Widget library |
| [`presentar-layout`](https://crates.io/crates/presentar-layout) | Layout engine |

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
