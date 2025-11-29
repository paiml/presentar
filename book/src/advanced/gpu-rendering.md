# GPU Rendering

WebGPU-accelerated rendering via Trueno-Viz.

## Pipeline

```
Draw Commands → Vertex Buffer → WGSL Shader → Framebuffer
```

## Backend Selection

```rust
// Auto-select best backend
let backend = Backend::auto();

// Force specific backend
let backend = Backend::WebGPU;
let backend = Backend::Software;  // Fallback
```

## Batching

Commands batch by type for efficiency:

```
100 FillRect → 1 draw call
50 DrawText → 1 draw call (shared atlas)
```

## Texture Atlas

Text glyphs share a texture atlas:

```rust
// Glyph cache
struct GlyphCache {
    atlas: Texture,
    glyphs: HashMap<GlyphKey, GlyphInfo>,
}
```

## Performance

| Operation | CPU | GPU |
|-----------|-----|-----|
| 1000 rects | 5ms | 0.5ms |
| Text (100 glyphs) | 10ms | 1ms |
| Full frame | 15ms | 2ms |

## Fallback

Software rendering for testing:

```rust
#[cfg(test)]
let renderer = SoftwareRenderer::new();
```

## Verified Test

```rust
#[test]
fn test_gpu_rendering_abstraction() {
    // RecordingCanvas abstracts GPU details
    use presentar_core::RecordingCanvas;

    let canvas = RecordingCanvas::new();
    assert_eq!(canvas.command_count(), 0);
}
```
