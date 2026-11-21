//! Native Direct3D 9 executor for a hardware vertex-processing device.
//!
//! Render the silhouette and prepare the scene stencil with your renderer.
//! Submit the glow inside the device's BeginScene/EndScene pair.

use crate::d3d_shader::compile;
use crate::shaders::{self, Api};
use crate::{Backend, ClassicPasses, Extent};
use std::ptr::null_mut;
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG};
use windows::Win32::Graphics::Direct3D9::*;
use windows::core::{Error, HRESULT, Result};

pub struct Target<'a> {
    pub color: &'a IDirect3DSurface9,
    /// The glow is drawn where this stencil is nonzero.
    pub stencil: Option<&'a IDirect3DSurface9>,
}

pub struct D3d9 {
    device: IDirect3DDevice9,
    target_slots: u32,
    vertex: IDirect3DVertexShader9,
    blur: IDirect3DPixelShader9,
    composite: IDirect3DPixelShader9,
    scratch: Option<Scratch>,
}

struct Scratch {
    extent: Extent,
    horizontal: Texture,
    vertical: Texture,
}

struct Texture {
    handle: IDirect3DTexture9,
    surface: IDirect3DSurface9,
}

impl D3d9 {
    /// Compiles the Classic shaders with D3DCompiler_47.
    pub fn new(device: &IDirect3DDevice9) -> Result<Self> {
        let sources = shaders::classic(Api::Direct3D9);
        let vertex = compile(sources.fullscreen_vertex)?;
        let blur = compile(sources.blur_fragment)?;
        let composite = compile(sources.composite_fragment)?;
        let mut caps = D3DCAPS9::default();
        // The compiled blobs remain alive through each shader creation call.
        unsafe {
            device.GetDeviceCaps(&mut caps)?;
            Ok(Self {
                device: device.clone(),
                target_slots: caps.NumSimultaneousRTs,
                vertex: device.CreateVertexShader(vertex.GetBufferPointer().cast())?,
                blur: device.CreatePixelShader(blur.GetBufferPointer().cast())?,
                composite: device.CreatePixelShader(composite.GetBufferPointer().cast())?,
                scratch: None,
            })
        }
    }

    /// Creates a single-sample render-target texture for the caller's silhouette.
    /// Clear level 0 to black and draw the selected geometry in white.
    pub fn create_mask(&self, extent: Extent) -> Result<IDirect3DTexture9> {
        Ok(self.create_texture(extent)?.handle)
    }

    /// Releases the cached default-pool textures before a device Reset.
    pub fn release_scratch(&mut self) {
        self.scratch = None;
    }

    fn create_texture(&self, extent: Extent) -> Result<Texture> {
        let create = |format| {
            let mut handle = None;
            // Direct3D initializes the COM out-parameter on success.
            unsafe {
                self.device.CreateTexture(
                    extent.width(),
                    extent.height(),
                    1,
                    D3DUSAGE_RENDERTARGET as u32,
                    format,
                    D3DPOOL_DEFAULT,
                    &mut handle,
                    null_mut(),
                )?;
                let handle = handle.ok_or_else(|| Error::from_hresult(E_FAIL))?;
                let surface = handle.GetSurfaceLevel(0)?;
                Ok(Texture { handle, surface })
            }
        };
        create(D3DFMT_X8R8G8B8).or_else(|_| create(D3DFMT_A8R8G8B8))
    }

    fn prepare(&mut self, extent: Extent) -> Result<()> {
        if self.scratch.as_ref().is_none_or(|s| s.extent != extent) {
            self.scratch = Some(Scratch {
                extent,
                horizontal: self.create_texture(extent)?,
                vertical: self.create_texture(extent)?,
            });
        }
        Ok(())
    }

    fn configure(&self) -> Result<()> {
        let device = &self.device;
        // All states are restored by the captured state block after the passes.
        unsafe {
            for slot in 1..self.target_slots {
                device.SetRenderTarget(slot, None::<&IDirect3DSurface9>)?;
            }
            device.SetDepthStencilSurface(None::<&IDirect3DSurface9>)?;
            device.SetVertexShader(&self.vertex)?;
            device.SetFVF(D3DFVF_XYZ | D3DFVF_TEX1)?;
            device.SetStreamSourceFreq(0, 1)?;
            for (state, value) in [
                (D3DRS_ZENABLE, 0),
                (D3DRS_ZWRITEENABLE, 0),
                (D3DRS_ALPHATESTENABLE, 0),
                (D3DRS_SCISSORTESTENABLE, 0),
                (D3DRS_CULLMODE, D3DCULL_NONE.0 as u32),
                (D3DRS_FILLMODE, D3DFILL_SOLID.0 as u32),
                (D3DRS_COLORWRITEENABLE, 0xf),
                (D3DRS_FOGENABLE, 0),
                (D3DRS_CLIPPING, 1),
                (D3DRS_CLIPPLANEENABLE, 0),
                (D3DRS_MULTISAMPLEMASK, u32::MAX),
                (D3DRS_ALPHABLENDENABLE, 0),
                (D3DRS_SRGBWRITEENABLE, 0),
                (D3DRS_STENCILENABLE, 0),
                (D3DRS_TWOSIDEDSTENCILMODE, 0),
            ] {
                device.SetRenderState(state, value)?;
            }
            for (state, value) in [
                (D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as u32),
                (D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as u32),
                (D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_MIPFILTER, D3DTEXF_NONE.0 as u32),
                (D3DSAMP_MAXMIPLEVEL, 0),
                (D3DSAMP_SRGBTEXTURE, 0),
            ] {
                device.SetSamplerState(0, state, value)?;
            }
        }
        Ok(())
    }

    fn configure_composite(&self, stencil: bool) -> Result<()> {
        let device = &self.device;
        // The composite writes zero alpha, so ONE / INVSRCALPHA adds RGB
        // while leaving the scene alpha intact.
        unsafe {
            for (state, value) in [
                (D3DRS_ALPHABLENDENABLE, 1),
                (D3DRS_BLENDOP, D3DBLENDOP_ADD.0 as u32),
                (D3DRS_SRCBLEND, D3DBLEND_ONE.0 as u32),
                (D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32),
                (D3DRS_SEPARATEALPHABLENDENABLE, 0),
                (D3DRS_SRGBWRITEENABLE, 1),
                (D3DRS_STENCILENABLE, u32::from(stencil)),
                (D3DRS_STENCILFUNC, D3DCMP_NOTEQUAL.0 as u32),
                (D3DRS_STENCILREF, 0),
                (D3DRS_STENCILMASK, 0xff),
                (D3DRS_STENCILWRITEMASK, 0),
                (D3DRS_STENCILPASS, D3DSTENCILOP_KEEP.0 as u32),
                (D3DRS_STENCILFAIL, D3DSTENCILOP_KEEP.0 as u32),
                (D3DRS_STENCILZFAIL, D3DSTENCILOP_KEEP.0 as u32),
            ] {
                device.SetRenderState(state, value)?;
            }
            device.SetSamplerState(0, D3DSAMP_SRGBTEXTURE, 1)?;
        }
        Ok(())
    }

    fn pass(
        &self,
        source: &IDirect3DTexture9,
        target: &Target<'_>,
        extent: Extent,
        parameters: [f32; 4],
    ) -> Result<()> {
        let device = &self.device;
        let x = 1.0 / extent.width() as f32;
        let y = 1.0 / extent.height() as f32;
        // XYZ and one UV set, with D3D9's half-pixel correction.
        let vertices = [
            [-1.0 - x, 1.0 + y, 0.0, 0.0, 0.0],
            [1.0 - x, 1.0 + y, 0.0, 1.0, 0.0],
            [-1.0 - x, -1.0 + y, 0.0, 0.0, 1.0],
            [1.0 - x, -1.0 + y, 0.0, 1.0, 1.0],
        ];
        // Resource bindings and shader constant storage are valid for the draw.
        unsafe {
            device.SetTexture(0, None::<&IDirect3DBaseTexture9>)?;
            device.SetRenderTarget(0, target.color)?;
            device.SetDepthStencilSurface(target.stencil)?;
            device.SetTexture(0, source)?;
            device.SetPixelShaderConstantF(0, parameters.as_ptr(), 1)?;
            device.DrawPrimitiveUP(
                D3DPT_TRIANGLESTRIP,
                2,
                vertices.as_ptr().cast(),
                size_of::<[f32; 5]>() as u32,
            )
        }
    }

    fn submit(
        &self,
        mask: &IDirect3DTexture9,
        target: &Target<'_>,
        passes: ClassicPasses,
        scratch: &Scratch,
    ) -> Result<()> {
        self.configure()?;
        // These shaders were created on the same device.
        unsafe { self.device.SetPixelShader(&self.blur)? };
        self.pass(
            mask,
            &Target {
                color: &scratch.horizontal.surface,
                stencil: None,
            },
            passes.mask_extent,
            passes.horizontal,
        )?;
        self.pass(
            &scratch.horizontal.handle,
            &Target {
                color: &scratch.vertical.surface,
                stencil: None,
            },
            passes.mask_extent,
            passes.vertical,
        )?;
        self.configure_composite(target.stencil.is_some())?;
        unsafe { self.device.SetPixelShader(&self.composite)? };
        self.pass(
            &scratch.vertical.handle,
            target,
            passes.output_extent,
            passes.composite,
        )
    }
}

impl Backend for D3d9 {
    type Mask = IDirect3DTexture9;
    type Target<'a> = Target<'a>;
    type Error = Error;

    fn draw_classic(
        &mut self,
        mask: &Self::Mask,
        target: &Self::Target<'_>,
        passes: ClassicPasses,
    ) -> Result<()> {
        let mut mask_desc = D3DSURFACE_DESC::default();
        let mut target_desc = D3DSURFACE_DESC::default();
        // Read the headers before any drawing or pipeline state changes.
        unsafe {
            mask.GetLevelDesc(0, &mut mask_desc)?;
            target.color.GetDesc(&mut target_desc)?;
        }
        if Extent::new(mask_desc.Width, mask_desc.Height) != Some(passes.mask_extent)
            || Extent::new(target_desc.Width, target_desc.Height) != Some(passes.output_extent)
        {
            return Err(Error::new(
                E_INVALIDARG,
                "Classic glow texture sizes do not match the pass extents",
            ));
        }

        self.prepare(passes.mask_extent)?;
        let state = SavedState::capture(&self.device, self.target_slots)?;
        let draw = self.submit(mask, target, passes, self.scratch.as_ref().unwrap());
        let restore = state.restore(&self.device);
        draw.and(restore)
    }
}

struct SavedState {
    state: IDirect3DStateBlock9,
    primary: IDirect3DSurface9,
    additional: Vec<Option<IDirect3DSurface9>>,
    depth: Option<IDirect3DSurface9>,
}

impl SavedState {
    fn capture(device: &IDirect3DDevice9, slots: u32) -> Result<Self> {
        // D3DSBT_ALL captures pipeline state. Render targets and the depth
        // surface are retrieved separately.
        unsafe {
            Ok(Self {
                state: device.CreateStateBlock(D3DSBT_ALL)?,
                primary: device.GetRenderTarget(0)?,
                additional: (1..slots)
                    .map(|slot| optional_surface(device.GetRenderTarget(slot)))
                    .collect::<Result<_>>()?,
                depth: optional_surface(device.GetDepthStencilSurface())?,
            })
        }
    }

    fn restore(self, device: &IDirect3DDevice9) -> Result<()> {
        // Try every restoration even if an individual call fails.
        unsafe {
            let mut result = device.SetTexture(0, None::<&IDirect3DBaseTexture9>);
            result = result.and(device.SetDepthStencilSurface(None::<&IDirect3DSurface9>));
            result = result.and(device.SetRenderTarget(0, &self.primary));
            for (slot, target) in (1..).zip(&self.additional) {
                result = result.and(device.SetRenderTarget(slot, target.as_ref()));
            }
            result = result.and(device.SetDepthStencilSurface(self.depth.as_ref()));
            result.and(self.state.Apply())
        }
    }
}

fn optional_surface(result: Result<IDirect3DSurface9>) -> Result<Option<IDirect3DSurface9>> {
    const D3DERR_NOTFOUND: HRESULT = HRESULT(0x8876_0866_u32 as i32);
    match result {
        Err(error) if error.code() == D3DERR_NOTFOUND => Ok(None),
        other => other.map(Some),
    }
}
