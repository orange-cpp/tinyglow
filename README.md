# tinyglow

Rust library for a Classic outline glow effect. The renderer supplies a
screen-space silhouette and a scene destination. Tinyglow calculates
the passes and provides shaders for OpenGL 4.3 and Direct3D 9, 10, 11, and 12.

## Preview app

On Windows, run the Direct3D 9 preview from the repository root.

```powershell
cargo run --release --features preview --example preview
```

The left panel shows the original rotating model and the right panel runs
Tinyglow's Classic pass. Use the toolbar to toggle the effect, change its color,
thickness or intensity, switch between full and half resolution, or pause at a
specific angle.

## Direct3D 9

Enable the `d3d9` Cargo feature on Windows. The executor requires a device with
hardware vertex processing, Shader Model 3, and `d3dcompiler_47.dll`.

```rust,ignore
use tinyglow::{ClassicGlow, Extent, Quality};
use tinyglow::d3d9::{D3d9, Target};

let mut backend = D3d9::new(&device)?;
let glow = ClassicGlow {
    color: [80, 170, 255],
    quality: Quality::Half,
    ..ClassicGlow::default()
};
let extent = Extent::new(width, height).expect("a nonzero scene size");
let mask = backend.create_mask(glow.mask_extent(extent))?;

// Each frame, clear the mask to black and render the selected silhouettes in white.
// Prepare the scene stencil with zero on excluded pixels and nonzero elsewhere.
// Inside BeginScene / EndScene, after scene and mask rendering:
glow.render(
    &mut backend,
    &mask,
    &Target { color: &scene_surface, stencil: Some(&scene_depth_stencil) },
    extent,
)?;
```

Reuse the executor and mask across frames. Recreate the mask when its extent
changes. Before a Direct3D 9 device reset, release the mask and call
`release_scratch()`. The executor recreates scratch textures on the next draw.
A target with `stencil: None` reproduces the reference's no-stencil fallback,
which allows the glow to cover the silhouette interior.

## OpenGL

Enable the `opengl` feature for the native desktop OpenGL 4.3 executor. Supply a
single-sample, linear RGBA8 mask at `glow.mask_extent(extent)` and an sRGB
draw framebuffer with one color output. Keep the same context current whenever
you create, use, or drop the executor.

```rust,ignore
use tinyglow::opengl::{OpenGl, Target};

let mut backend = unsafe { OpenGl::new(&gl)? };
let target = Target { framebuffer: Some(scene_fbo), stencil: true };
glow.render(&mut backend, &mask_texture, &target, extent)?;
```

The executor caches its scratch textures and leaves the draw framebuffer and
pipeline configured for the composite. Rebind your pipeline for the next pass.
Use `framebuffer: None` with an sRGB-capable default framebuffer.

## Direct3D 10 and 11

Enable the `d3d10` or `d3d11` feature on Windows. The native executor needs a
linear, single-sample `Texture2D` silhouette SRV, an sRGB scene RTV, and an
optional stencil DSV matching the scene target. Render the silhouette at
`glow.mask_extent(extent)` and pass the scene dimensions as `extent`.

```rust,ignore
use tinyglow::d3d11::{D3d11, Target};

let mut backend = D3d11::new(&context)?;
let target = Target { color: &scene_rtv, stencil: Some(&scene_dsv) };
glow.render(&mut backend, &mask_srv, &target, extent)?;
```

Direct3D 10 uses `tinyglow::d3d10::{D3d10, Target}` and
`D3d10::new(&device)`. Both executors cache their scratch textures. Rebind
your pipeline for the next pass.

## Direct3D 12

Enable `d3d12` on Windows. Create an executor for the scene's single-sample
sRGB RTV format and optional stencil DSV format. Wrap the linear RGBA8
silhouette texture with `create_mask` to allocate its shader descriptors and
scratch textures. Reuse the mask as long as the underlying texture is reused
and its extent matches `glow.mask_extent(extent)`.

```rust,ignore
use tinyglow::d3d12::{D3d12, Target};

let mut backend = D3d12::new(&device, color_format, Some(stencil_format))?;
let mask = backend.create_mask(&silhouette_texture)?;

// After transitioning the renderer-owned resources into their draw states:
let target = unsafe { Target::new(&graphics_list, scene_rtv, Some(scene_dsv)) };
glow.render(&mut backend, &mask, &target, extent)?;
```

Tinyglow records the three passes and the scratch transitions onto your
direct command list. Your renderer manages the silhouette and scene
transitions, command submission and fences. Keep the backend, mask and scene
resources alive until the GPU completes the draw. Rebind your descriptor heaps
and pipeline for any subsequent passes. [Direct3D 12 resource barriers](https://learn.microsoft.com/en-us/windows/win32/direct3d12/using-resource-barriers-to-synchronize-resource-states-in-direct3d-12)

## Custom integrations

These APIs use your renderer's resource bindings, pipeline state and command
submission. Implement `Backend` with your renderer's mask and target types
and load the shader set from `shaders::classic(Api::…)`.

1. Render white silhouettes over black at `ClassicGlow::mask_extent(output)`.
   The reference renders this mask without a depth test. Prepare a separate
   output-resolution stencil using the scene's geometry and occlusion rules.
2. Run the horizontal and vertical passes into distinct single-sample,
   one-mip RGBA8 UNORM textures with clamped linear filtering. Write the blur
   passes through linear UNORM target views. Each pass receives its
   `ClassicPasses` vector with the texel step in XY and the gain in Z.
3. Sample the vertical texture through an sRGB view with the same filtering.
   Composite into the loaded scene color in linear space with
   `ADD / ONE / ONE_MINUS_SRC_ALPHA` and an sRGB output target. The shader
   emits zero alpha, preserving scene alpha. When a stencil is available, use
   `NOT_EQUAL` against zero, read mask `0xff`, and keep all stencil values.
   Disable depth testing and writes. Composite parameters are linear RGB in
   XYZ and intensity in W.

The shader bindings are `t0`, `s0`, and a single `float4` at `b0` for
Direct3D 10–12. OpenGL uses texture unit 0 and the `parameters` uniform. Both
use a fullscreen triangle generated from vertex IDs 0 through 2. For Direct3D
9, use sampler `s0` and constant `c0` with an XYZ/UV strip and the reference
half-pixel offset. For the vertical blur texture, Direct3D 10–12 can use linear
and sRGB views on a typeless RGBA8 resource. OpenGL can use an sRGB texture
view on immutable RGBA8 storage and enable `GL_FRAMEBUFFER_SRGB` for the
composite. [Direct3D color-space conversions](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/converting-data-color-space)
and [OpenGL Core specification](https://registry.khronos.org/OpenGL/specs/gl/glspec43.core.pdf).

Classic preserves the reference's 13 weighted taps, its width-based vertical
step, 8-bit intermediate targets, and color conversion with exponent 2.2.
Thickness controls the vertical pass gain. Half quality uses floor-divided
dimensions with a minimum of one pixel per axis.
