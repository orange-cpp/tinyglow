use crate::shaders::Api;
use crate::{Backend, ClassicGlow, Extent};
use std::time::Duration;

macro_rules! style_source {
    ($file:literal, $header:literal, $entry:literal) => {
        concat!(
            include_str!($header),
            "\n",
            include_str!("shaders/styles/common.hlsl"),
            "\n",
            include_str!(concat!("shaders/styles/", $file, ".hlsl")),
            "\n",
            include_str!("shaders/styles/main.hlsl"),
            "\n",
            $entry,
        )
    };
}

macro_rules! styles {
    ($($variant:ident, $name:literal, $anywhere:literal, $file:literal;)*) => {
        /// Stylized glow composites in their original order.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        #[non_exhaustive]
        pub enum GlowStyle { $($variant,)* }

        impl GlowStyle {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $name,)* }
            }

            /// Whether the style also draws inside the silhouette.
            pub const fn draws_inside(self) -> bool {
                match self { $(Self::$variant => $anywhere,)* }
            }

            pub(crate) const fn source(self, api: Api) -> &'static str {
                match self {
                    $(Self::$variant => match api {
                        Api::Direct3D9 => style_source!($file, "shaders/styles/d3d9.hlsli",
                            "float4 main(float2 uv : TEXCOORD0) : COLOR { return composite_style(uv); }"),
                        Api::OpenGl => style_source!($file, "shaders/styles/opengl.glsli",
                            "void main() { fragment = composite_style(uv); }"),
                        _ => style_source!($file, "shaders/styles/modern.hlsli",
                            "float4 main(float4 position : SV_Position, float2 uv : TEXCOORD0) : SV_Target0 { return composite_style(uv); }"),
                    },)*
                }
            }
        }
    };
}

styles! {
    Neon, "Neon", false, "neon";
    Rainbow, "Rainbow", false, "rainbow";
    RainbowContour, "Rainbow Contour", false, "rainbow_contour";
    PulseWave, "Pulse Wave", false, "pulse_wave";
    Flicker, "Flicker", false, "flicker";
    Scanline, "Scanline", false, "scanline";
    HotCore, "Hot Core", false, "hot_core";
    Sharp, "Sharp", false, "sharp";
    Electric, "Electric", false, "electric";
    Plasma, "Plasma", false, "plasma";
    Fire, "Fire", false, "fire";
    Shimmer, "Shimmer", false, "shimmer";
    Breathe, "Breathe", false, "breathe";
    Strobe, "Strobe", false, "strobe";
    DoubleOutline, "Double Outline", false, "double_outline";
    Chromatic, "Chromatic", false, "chromatic";
    Glitch, "Glitch", false, "glitch";
    Topographic, "Topographic", false, "topographic";
    Aurora, "Aurora", false, "aurora";
    ColoredFlame, "Colored Flame", false, "colored_flame";
    GhostFire, "Ghost Fire", false, "ghost_fire";
    Storm, "Storm", false, "storm";
    Hologram, "Hologram", false, "hologram";
    Frost, "Frost", false, "frost";
    Glitter, "Glitter", false, "glitter";
    Matrix, "Matrix", false, "matrix";
    GodRays, "God Rays", false, "god_rays";
    Toxic, "Toxic", false, "toxic";
    Sonar, "Sonar", false, "sonar";
    RimLight, "Rim Light", true, "rim_light";
    CrispOutline, "Crisp Outline", false, "crisp_outline";
    Comic, "Comic", true, "comic";
    Smoke, "Smoke", false, "smoke";
    EnergyShield, "Energy Shield", true, "energy_shield";
    XRay, "X-Ray", true, "x_ray";
    NeonTube, "Neon Tube", false, "neon_tube";
    Burn, "Burn", true, "burn";
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyledGlow {
    pub style: GlowStyle,
    /// Shared color, thickness, intensity, and quality controls.
    pub glow: ClassicGlow,
    /// Animation rate, clamped to 0.1–4.
    pub speed: f32,
    /// Scale of style-specific features, clamped to 0.25–4.
    pub size: f32,
}

/// Dimensions and shader constants for a single styled draw.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct StyledPasses {
    pub style: GlowStyle,
    pub mask_extent: Extent,
    pub output_extent: Extent,
    /// Same step and gain layout as [`crate::ClassicPasses::horizontal`].
    pub horizontal: [f32; 4],
    pub vertical: [f32; 4],
    /// Four vectors for the composite, in `c0` through `c3` order on D3D9,
    /// at `b0` on D3D10–12, or in the OpenGL `parameters` array.
    pub composite: [[f32; 4]; 4],
}

#[cfg(any(
    all(windows, any(feature = "d3d10", feature = "d3d11", feature = "d3d12")),
    all(not(target_arch = "wasm32"), feature = "opengl")
))]
pub(crate) struct DrawPasses {
    pub style: Option<GlowStyle>,
    pub mask_extent: Extent,
    pub output_extent: Extent,
    pub horizontal: [f32; 4],
    pub vertical: [f32; 4],
    composite: [[f32; 4]; 4],
}

#[cfg(any(
    all(windows, any(feature = "d3d10", feature = "d3d11", feature = "d3d12")),
    all(not(target_arch = "wasm32"), feature = "opengl")
))]
impl DrawPasses {
    pub fn classic(passes: crate::ClassicPasses) -> Self {
        Self {
            style: None,
            mask_extent: passes.mask_extent,
            output_extent: passes.output_extent,
            horizontal: passes.horizontal,
            vertical: passes.vertical,
            composite: [passes.composite, [0.0; 4], [0.0; 4], [0.0; 4]],
        }
    }

    pub fn styled(passes: StyledPasses) -> Self {
        Self {
            style: Some(passes.style),
            mask_extent: passes.mask_extent,
            output_extent: passes.output_extent,
            horizontal: passes.horizontal,
            vertical: passes.vertical,
            composite: passes.composite,
        }
    }

    pub fn composite(&self) -> &[[f32; 4]] {
        if self.style.is_some() {
            &self.composite
        } else {
            &self.composite[..1]
        }
    }

    pub fn uses_stencil(&self) -> bool {
        !self.style.is_some_and(GlowStyle::draws_inside)
    }
}

/// Executes the same blur passes as [`Backend`], then composites using the
/// blurred texture at slot 0 and the original mask at slot 1. Use clamped
/// bilinear sampling and a UNORM target, with sRGB reads and writes disabled.
/// Blend with ADD / ONE / ONE_MINUS_SRC_ALPHA. Use the Classic
/// stencil test only when a stencil is supplied and the style draws outside.
pub trait StyledBackend: Backend {
    fn draw_styled(
        &mut self,
        mask: &Self::Mask,
        target: &Self::Target<'_>,
        passes: StyledPasses,
    ) -> Result<(), Self::Error>;
}

impl StyledGlow {
    pub fn new(style: GlowStyle) -> Self {
        Self {
            style,
            glow: ClassicGlow::default(),
            speed: 1.0,
            size: 1.0,
        }
    }

    pub fn mask_extent(self, output: Extent) -> Extent {
        self.glow.mask_extent(output)
    }

    pub fn passes(self, output: Extent, elapsed: Duration) -> StyledPasses {
        let blur = self.glow.passes(output);
        let extent = blur.mask_extent;
        let speed = f64::from(self.speed.clamp(0.1, 4.0));
        let time = ((elapsed.as_secs_f64() % 3600.0) * speed) % 3600.0;
        let rgb = self.glow.color.map(|c| f32::from(c) / 255.0);
        let [r, g, b] = rgb;
        let [h, s, v] = hsv(rgb);
        StyledPasses {
            style: self.style,
            mask_extent: extent,
            output_extent: output,
            horizontal: blur.horizontal,
            vertical: blur.vertical,
            composite: [
                [r, g, b, 1.0],
                [
                    time as f32,
                    1.0 / extent.width() as f32,
                    1.0 / extent.height() as f32,
                    0.0,
                ],
                [blur.composite[3], self.size.clamp(0.25, 4.0), 0.0, 0.0],
                [h, s, v, 0.0],
            ],
        }
    }

    pub fn render<B: StyledBackend + ?Sized>(
        self,
        backend: &mut B,
        mask: &B::Mask,
        target: &B::Target<'_>,
        output: Extent,
        elapsed: Duration,
    ) -> Result<(), B::Error> {
        backend.draw_styled(mask, target, self.passes(output, elapsed))
    }
}

fn hsv([r, g, b]: [f32; 3]) -> [f32; 3] {
    let value = r.max(g).max(b);
    let chroma = value - r.min(g).min(b);
    if chroma == 0.0 {
        return [0.0, 0.0, value];
    }
    let hue = if value == r {
        (g - b) / chroma / 6.0
    } else if value == g {
        (b - r) / chroma / 6.0 + 1.0 / 3.0
    } else {
        (r - g) / chroma / 6.0 + 2.0 / 3.0
    };
    [hue - hue.floor(), chroma / value, value]
}
