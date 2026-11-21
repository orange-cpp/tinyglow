//! Shader sources for renderer integrations.

use crate::GlowStyle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Api {
    OpenGl,
    Direct3D9,
    Direct3D10,
    Direct3D11,
    Direct3D12,
}

#[derive(Clone, Copy, Debug)]
pub struct Shader {
    pub source: &'static str,
    pub entry: &'static str,
    pub profile: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct ShaderSet {
    pub fullscreen_vertex: Shader,
    pub blur_fragment: Shader,
    pub composite_fragment: Shader,
}

const HLSL: &str = include_str!("shaders/classic.hlsl");
const LEGACY_HLSL: &str = concat!(
    "#define SHADER_MODEL_LEGACY 1\n",
    include_str!("shaders/classic.hlsl")
);

pub const fn classic(api: Api) -> ShaderSet {
    match api {
        Api::OpenGl => ShaderSet {
            fullscreen_vertex: shader(include_str!("shaders/fullscreen.vert"), "main", "430 core"),
            blur_fragment: shader(include_str!("shaders/blur.frag"), "main", "430 core"),
            composite_fragment: shader(include_str!("shaders/composite.frag"), "main", "430 core"),
        },
        Api::Direct3D9 => ShaderSet {
            fullscreen_vertex: shader(LEGACY_HLSL, "legacy_vertex", "vs_3_0"),
            blur_fragment: shader(LEGACY_HLSL, "legacy_blur", "ps_3_0"),
            composite_fragment: shader(LEGACY_HLSL, "legacy_composite", "ps_3_0"),
        },
        Api::Direct3D10 => modern("vs_4_0", "ps_4_0"),
        Api::Direct3D11 => modern("vs_5_0", "ps_5_0"),
        Api::Direct3D12 => modern("vs_6_0", "ps_6_0"),
    }
}

/// Shader Model 3 program for a style composite. Use the Classic vertex and
/// blur shaders for the preceding passes, and see [`crate::StyledBackend`]
/// for the composite bindings and pipeline state.
pub const fn d3d9_style(style: GlowStyle) -> Shader {
    styled(Api::Direct3D9, style).composite_fragment
}

/// Shaders for the two silhouette blurs and a styled composite. Use a
/// four-vertex triangle strip for all styled draws. On Direct3D 9 supply
/// the same XYZ/UV vertices and half-pixel offset as Classic.
/// On OpenGL, draw the composite into a floating-point texture with blending
/// disabled, then use [`opengl_style_resolve`] to blend it into the scene.
pub const fn styled(api: Api, style: GlowStyle) -> ShaderSet {
    let mut shaders = classic(api);
    shaders.fullscreen_vertex = styled_vertex(api);
    shaders.composite_fragment = shader(
        style.source(api),
        "main",
        shaders.composite_fragment.profile,
    );
    shaders
}

/// OpenGL shaders that flip the intermediate style result into the scene's
/// orientation. Draw a fullscreen triangle. Bind the intermediate texture at
/// unit 0 and set `parameters.x` to its height in pixels.
pub const fn opengl_style_resolve() -> (Shader, Shader) {
    (
        classic(Api::OpenGl).fullscreen_vertex,
        shader(
            include_str!("shaders/style_resolve.frag"),
            "main",
            "430 core",
        ),
    )
}

pub(crate) const fn styled_vertex(api: Api) -> Shader {
    let vertex = classic(api).fullscreen_vertex;
    match api {
        Api::OpenGl => shader(
            include_str!("shaders/fullscreen_quad.vert"),
            "main",
            "430 core",
        ),
        Api::Direct3D9 => vertex,
        _ => shader(HLSL, "fullscreen_quad", vertex.profile),
    }
}

const fn modern(vs: &'static str, ps: &'static str) -> ShaderSet {
    ShaderSet {
        fullscreen_vertex: shader(HLSL, "fullscreen_vertex", vs),
        blur_fragment: shader(HLSL, "blur_fragment", ps),
        composite_fragment: shader(HLSL, "composite_fragment", ps),
    }
}

const fn shader(source: &'static str, entry: &'static str, profile: &'static str) -> Shader {
    Shader {
        source,
        entry,
        profile,
    }
}
