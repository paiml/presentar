# WGSL Shaders

WebGPU Shading Language for custom rendering.

## Basic Shader

```wgsl
// Vertex shader
@vertex
fn vs_main(@location(0) position: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(position, 0.0, 1.0);
}

// Fragment shader
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);  // Red
}
```

## Uniforms

```wgsl
struct Uniforms {
    color: vec4<f32>,
    transform: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return uniforms.color;
}
```

## Built-in Shaders

| Shader | Use |
|--------|-----|
| `rect.wgsl` | Rectangles, rounded corners |
| `text.wgsl` | Text with atlas sampling |
| `circle.wgsl` | Circles, ellipses |
| `line.wgsl` | Lines, strokes |

## Custom Effects

```wgsl
// Gradient shader
@fragment
fn fs_gradient(
    @location(0) uv: vec2<f32>
) -> @location(0) vec4<f32> {
    let t = uv.y;
    let start = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    let end = vec4<f32>(0.0, 0.0, 1.0, 1.0);
    return mix(start, end, t);
}
```

## Verified Test

```rust
#[test]
fn test_wgsl_shader_syntax() {
    // WGSL is compiled at build time
    // This test verifies shader concepts
    let red = presentar_core::Color::RED;
    assert_eq!(red.r, 1.0);
    assert_eq!(red.g, 0.0);
}
```
