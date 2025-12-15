# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.3] - 2025-12-15

### Changed
- Updated trueno from v0.7.4 to v0.8.5 (simulation testing framework)

## [0.1.2] - 2025-12-01

### Added
- Integration tests directory with end-to-end tests
- Criterion benchmarks for layout operations
- Enhanced rustdoc with examples
- `.clippy.toml` with unwrap() ban configuration
- Comprehensive README with Installation, Usage, Examples sections

### Changed
- Replaced `unwrap()` calls with `expect()` in production code

## [0.1.0] - 2024-11-29

### Added
- Core types: `Size`, `Point`, `Rect`, `Constraints`, `Color`
- Widget trait with measure-layout-paint cycle
- 20+ widget implementations:
  - Button, Text, Column, Row, Container, Stack
  - Chart, Checkbox, DataCard, DataTable, Image
  - ModelCard, ProgressBar, RadioGroup, Select
  - Slider, Tabs, TextInput, Toggle, Tooltip
- RecordingCanvas for testing
- YAML manifest parsing
- Expression language for data binding
- A11yChecker for WCAG 2.1 AA compliance
- 112 documentation chapters with verified tests
- 1188 unit tests

### Architecture
- Unidirectional data flow (Event → State → Widget → Paint)
- WASM-first design targeting `wasm32-unknown-unknown`
- Flexbox-inspired constraint-based layout
- GPU-ready draw command abstraction

[Unreleased]: https://github.com/paiml/presentar/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/paiml/presentar/releases/tag/v0.1.0
