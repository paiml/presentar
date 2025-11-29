# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Presentar is a **WASM-first visualization and rapid application framework** built on the Sovereign AI Stack (Trueno, Aprender, Realizar, Pacha). It eliminates Python/CUDA/cloud dependencies for fully self-hosted AI workloads.

**Key characteristics:**
- Pure Rust targeting `wasm32-unknown-unknown`
- 80% Sovereign Stack (trueno-viz GPU primitives), 20% minimal external (winit, fontdue)
- YAML-driven declarative app configuration
- 60fps GPU-accelerated rendering via WebGPU/WGSL shaders
- Unidirectional data flow (Event → State → Widget → Draw)

## Architecture (Layer Hierarchy)

```
Layer 9: App Runtime        - YAML parser, .apr/.ald loaders, Pacha integration
Layer 8: Presentar          - Widget tree, layout engine, event dispatch, state management
Layer 7: Trueno-Viz         - Paths, fills, strokes, text, charts, WGSL shaders
Layer 6: Trueno             - SIMD/GPU tensor ops, backend dispatch, memory management
```

## Build Commands (when implemented)

```bash
# Development server with hot reload
make dev                    # cargo watch + wasm-bindgen + http.server :8080

# Production WASM build
make build                  # cargo build --target wasm32-unknown-unknown --release + wasm-opt

# Testing
make test                   # Runs unit, integration, and visual regression tests
make test-unit              # cargo nextest run --lib
make test-integration       # cargo nextest run --test '*'
make test-visual            # cargo test --features visual-regression

# Quality gates
make lint                   # clippy + yamllint + a11y check
make fmt                    # cargo fmt + prettier
make coverage               # cargo llvm-cov
make score                  # Quality scoring (0-100, F-A grades)

# Three-tier quality pipeline
make tier1                  # On-save (<1s): cargo check + yamllint
make tier2                  # Pre-commit (1-5min): fmt + lint + tests + score
make tier3                  # Nightly: tier2 + visual + coverage + mutation testing
```

## Core File Types

- `app.yaml` - Presentar application manifest (layout, data sources, interactions)
- `.apr` - Aprender model files
- `.ald` - Alimentar dataset files
- `.presentar-gates.toml` - Quality gate configuration

## Testing Framework (presentar-test)

**Zero external dependencies** - no playwright, selenium, puppeteer, npm, or C bindings. Pure Rust + WASM only.

Key testing patterns:
- `#[presentar_test]` attribute for test functions
- `Harness::new(include_bytes!("fixtures/app.tar"))` for fixture loading
- CSS-like selectors: `"Button"`, `"#submit-btn"`, `"[data-testid='login']"`
- Visual regression via `Snapshot::assert_match()`
- Built-in WCAG 2.1 AA accessibility checking

## Quality Standards

- **Minimum grade: B+** (80+ score) for production
- **60fps target** (<16ms frame time)
- **<500KB bundle size**
- **WCAG AA compliance** required
- **Model/data cards** required for ML assets

## Key Dependencies

- `trueno` - SIMD-accelerated tensor operations (always use latest from crates.io)
- `winit` - Window/event loop (only external for windowing)
- `fontdue` - Font rasterization (only external for fonts)
- `wgpu` - WebGPU abstraction

## Expression Language (in YAML)

```
{{ source | transform | transform }}

Transforms: filter(), select(), sort(), limit(), count(), sum(), mean(), rate(), percentage(), join()
```

All transforms execute client-side in WASM - no server round-trips.
