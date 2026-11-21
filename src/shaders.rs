//! Shader sources for renderer integrations.

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
