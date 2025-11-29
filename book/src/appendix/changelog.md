# Changelog

Version history and release notes.

## Version Format

```
MAJOR.MINOR.PATCH

MAJOR - Breaking API changes
MINOR - New features (backward compatible)
PATCH - Bug fixes (backward compatible)
```

## v0.1.0 (Current)

### Added

| Feature | Description |
|---------|-------------|
| Core widgets | Button, Text, Row, Column, Stack |
| Layout engine | Flexbox-inspired constraint system |
| Test harness | Zero-dependency visual testing |
| YAML config | Declarative app definition |
| A11y checking | WCAG 2.1 AA validation |

### Architecture

- Unidirectional data flow
- Widget trait with measure-layout-paint
- RecordingCanvas for draw commands
- CSS-like selectors for testing

## Versioning Policy

```rust
// Check version at runtime
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn check_compatibility(required: &str) -> bool {
    let current: Vec<u32> = VERSION.split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let req: Vec<u32> = required.split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    // Major version must match
    current.get(0) == req.get(0)
}
```

## Migration Notes

| From | To | Action |
|------|-----|--------|
| 0.0.x | 0.1.x | Update Widget trait |

## Verified Test

```rust
#[test]
fn test_changelog_version_parsing() {
    let version = "0.1.0";
    let parts: Vec<u32> = version.split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], 0);  // Major
    assert_eq!(parts[1], 1);  // Minor
    assert_eq!(parts[2], 0);  // Patch
}
```
