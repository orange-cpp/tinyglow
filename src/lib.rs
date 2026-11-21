#![doc = include_str!("../README.md")]

use std::num::NonZeroU32;

#[cfg(all(windows, feature = "d3d10"))]
pub mod d3d10;
#[cfg(all(windows, feature = "d3d11"))]
pub mod d3d11;
#[cfg(all(windows, feature = "d3d12"))]
pub mod d3d12;
#[cfg(all(windows, feature = "d3d9"))]
pub mod d3d9;
#[cfg(all(
    windows,
    any(
        feature = "d3d9",
        feature = "d3d10",
        feature = "d3d11",
        feature = "d3d12"
    )
))]
mod d3d_shader;
#[cfg(all(not(target_arch = "wasm32"), feature = "opengl"))]
pub mod opengl;
pub mod shaders;
mod styles;

pub use styles::{GlowStyle, StyledBackend, StyledGlow, StyledPasses};

/// The nonzero size of a two-dimensional render target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Extent {
    width: NonZeroU32,
    height: NonZeroU32,
}

impl Extent {
    pub const fn new(width: u32, height: u32) -> Option<Self> {
        match (NonZeroU32::new(width), NonZeroU32::new(height)) {
            (Some(width), Some(height)) => Some(Self { width, height }),
            _ => None,
        }
    }

    pub const fn width(self) -> u32 {
        self.width.get()
    }

    pub const fn height(self) -> u32 {
        self.height.get()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Quality {
    #[default]
    Full,
    Half,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassicGlow {
    /// Group color in 8-bit sRGB.
    pub color: [u8; 3],
    /// Vertical pass gain, clamped to 0–100.
    pub thickness: f32,
    /// Composite gain, clamped to 0.1–3.
    pub intensity: f32,
    pub quality: Quality,
}

impl Default for ClassicGlow {
    fn default() -> Self {
        Self {
            color: [255; 3],
            thickness: 5.0,
            intensity: 1.0,
            quality: Quality::Full,
        }
    }
}

/// Shader constants and dimensions for a single Classic draw.
///
/// Submit in order: silhouette to horizontal blur, horizontal to vertical blur,
/// then vertical blur to the scene. Use separate RGBA8 UNORM scratch textures.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ClassicPasses {
    pub mask_extent: Extent,
    pub output_extent: Extent,
    /// Packed as `[step_x, step_y, gain, 0]`.
    pub horizontal: [f32; 4],
    /// Same layout as `horizontal`.
    pub vertical: [f32; 4],
    /// Packed as `[linear_r, linear_g, linear_b, intensity]`.
    pub composite: [f32; 4],
}

/// Executes Classic passes against a renderer's own GPU resources.
///
/// The silhouette's red channel supplies coverage. Both blurs use clamped
/// bilinear UNORM reads and writes. The composite samples the vertical texture
/// as sRGB and additively blends into the scene in linear space. With a stencil,
/// compare against zero with NOT_EQUAL, read mask 0xff, and keep all values.
pub trait Backend {
    type Mask: ?Sized;
    type Target<'a>: ?Sized;
    type Error;

    fn draw_classic(
        &mut self,
        mask: &Self::Mask,
        target: &Self::Target<'_>,
        passes: ClassicPasses,
    ) -> Result<(), Self::Error>;
}

impl ClassicGlow {
    /// Extent at which to render the silhouette and run both blur passes.
    pub fn mask_extent(self, output: Extent) -> Extent {
        match self.quality {
            Quality::Full => output,
            Quality::Half => {
                Extent::new((output.width() / 2).max(1), (output.height() / 2).max(1)).unwrap()
            }
        }
    }

    pub fn passes(self, output: Extent) -> ClassicPasses {
        let mask = self.mask_extent(output);
        let step = 1.0 / mask.width() as f32;
        let [r, g, b] = self.color.map(|c| (f32::from(c) / 255.0).powf(2.2));
        ClassicPasses {
            mask_extent: mask,
            output_extent: output,
            horizontal: [step, 0.0, 1.0, 0.0],
            // The reference uses texture width here as well.
            vertical: [0.0, step, self.thickness.clamp(0.0, 100.0), 0.0],
            composite: [r, g, b, self.intensity.clamp(0.1, 3.0)],
        }
    }

    pub fn render<B: Backend + ?Sized>(
        self,
        backend: &mut B,
        mask: &B::Mask,
        target: &B::Target<'_>,
        output: Extent,
    ) -> Result<(), B::Error> {
        backend.draw_classic(mask, target, self.passes(output))
    }
}
