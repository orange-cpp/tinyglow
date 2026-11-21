//! Native Direct3D 10 executor.
//!
//! Supply a linear, single-sample silhouette SRV at the mask extent and an
//! sRGB scene RTV at the output extent. Rebind your pipeline for the next pass.

use crate::d3d_shader::{blob_bytes, compile, is_srgb, out};
use crate::shaders::{self, Api};
use crate::{Backend, ClassicPasses, Extent};
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct3D::{
    D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, D3D_SRV_DIMENSION_TEXTURE2D,
};
use windows::Win32::Graphics::Direct3D10::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Error, Result};

pub struct Target<'a> {
    pub color: &'a ID3D10RenderTargetView,
    /// The glow is drawn where this stencil is nonzero.
    pub stencil: Option<&'a ID3D10DepthStencilView>,
}

pub struct D3d10 {
    device: ID3D10Device,
    vertex: ID3D10VertexShader,
    blur: ID3D10PixelShader,
    composite: ID3D10PixelShader,
    constants: ID3D10Buffer,
    sampler: ID3D10SamplerState,
    rasterizer: ID3D10RasterizerState,
    blend: ID3D10BlendState,
    depth_disabled: ID3D10DepthStencilState,
    stencil_test: ID3D10DepthStencilState,
    scratch: Option<Scratch>,
}

struct Scratch {
    extent: Extent,
    horizontal: Texture,
    vertical: Texture,
}

struct Texture {
    color: ID3D10RenderTargetView,
    sample: ID3D10ShaderResourceView,
}

impl D3d10 {
    /// Creates the Classic shaders and pipeline state with D3DCompiler_47.
    pub fn new(device: &ID3D10Device) -> Result<Self> {
        let sources = shaders::classic(Api::Direct3D10);
        let vertex = compile(sources.fullscreen_vertex)?;
        let blur = compile(sources.blur_fragment)?;
        let composite = compile(sources.composite_fragment)?;
        let sampler = D3D10_SAMPLER_DESC {
            Filter: D3D10_FILTER_MIN_MAG_LINEAR_MIP_POINT,
            AddressU: D3D10_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D10_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D10_TEXTURE_ADDRESS_CLAMP,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D10_COMPARISON_ALWAYS,
            ..Default::default()
        };
        let rasterizer = D3D10_RASTERIZER_DESC {
            FillMode: D3D10_FILL_SOLID,
            CullMode: D3D10_CULL_NONE,
            DepthClipEnable: true.into(),
            MultisampleEnable: true.into(),
            ..Default::default()
        };
        let face = D3D10_DEPTH_STENCILOP_DESC {
            StencilFailOp: D3D10_STENCIL_OP_KEEP,
            StencilDepthFailOp: D3D10_STENCIL_OP_KEEP,
            StencilPassOp: D3D10_STENCIL_OP_KEEP,
            StencilFunc: D3D10_COMPARISON_NOT_EQUAL,
        };
        let mut depth = D3D10_DEPTH_STENCIL_DESC {
            DepthWriteMask: D3D10_DEPTH_WRITE_MASK_ZERO,
            DepthFunc: D3D10_COMPARISON_ALWAYS,
            StencilReadMask: 0xff,
            FrontFace: face,
            BackFace: face,
            ..Default::default()
        };
        let mut blend = D3D10_BLEND_DESC {
            SrcBlend: D3D10_BLEND_ONE,
            DestBlend: D3D10_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D10_BLEND_OP_ADD,
            SrcBlendAlpha: D3D10_BLEND_ONE,
            DestBlendAlpha: D3D10_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D10_BLEND_OP_ADD,
            ..Default::default()
        };
        blend.BlendEnable[0] = true.into();
        blend.RenderTargetWriteMask[0] = D3D10_COLOR_WRITE_ENABLE_ALL.0 as u8;
        let constants = D3D10_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D10_USAGE_DEFAULT,
            BindFlags: D3D10_BIND_CONSTANT_BUFFER.0 as u32,
            ..Default::default()
        };

        // The descriptors and bytecode live through creation and every result is checked.
        unsafe {
            let depth_disabled = out(|out| device.CreateDepthStencilState(&depth, Some(out)))?;
            depth.StencilEnable = true.into();
            Ok(Self {
                device: device.clone(),
                vertex: out(|out| device.CreateVertexShader(blob_bytes(&vertex), Some(out)))?,
                blur: out(|out| device.CreatePixelShader(blob_bytes(&blur), Some(out)))?,
                composite: out(|out| device.CreatePixelShader(blob_bytes(&composite), Some(out)))?,
                constants: out(|out| device.CreateBuffer(&constants, None, Some(out)))?,
                sampler: out(|out| device.CreateSamplerState(&sampler, Some(out)))?,
                rasterizer: out(|out| device.CreateRasterizerState(&rasterizer, Some(out)))?,
                blend: out(|out| device.CreateBlendState(&blend, Some(out)))?,
                stencil_test: out(|out| device.CreateDepthStencilState(&depth, Some(out)))?,
                depth_disabled,
                scratch: None,
            })
        }
    }

    fn texture(&self, extent: Extent, sample_format: DXGI_FORMAT) -> Result<Texture> {
        let desc = D3D10_TEXTURE2D_DESC {
            Width: extent.width(),
            Height: extent.height(),
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_TYPELESS,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D10_USAGE_DEFAULT,
            BindFlags: (D3D10_BIND_RENDER_TARGET.0 | D3D10_BIND_SHADER_RESOURCE.0) as u32,
            ..Default::default()
        };
        let color = D3D10_RENDER_TARGET_VIEW_DESC {
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            ViewDimension: D3D10_RTV_DIMENSION_TEXTURE2D,
            Anonymous: D3D10_RENDER_TARGET_VIEW_DESC_0 {
                Texture2D: D3D10_TEX2D_RTV { MipSlice: 0 },
            },
        };
        let sample = D3D10_SHADER_RESOURCE_VIEW_DESC {
            Format: sample_format,
            ViewDimension: D3D_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D10_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D10_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        // Both views hold references to the texture once created.
        unsafe {
            let texture = self.device.CreateTexture2D(&desc, None)?;
            Ok(Texture {
                color: out(|out| {
                    self.device
                        .CreateRenderTargetView(&texture, Some(&color), Some(out))
                })?,
                sample: out(|out| {
                    self.device
                        .CreateShaderResourceView(&texture, Some(&sample), Some(out))
                })?,
            })
        }
    }

    fn pass(
        &self,
        source: &ID3D10ShaderResourceView,
        target: &Target<'_>,
        extent: Extent,
        parameters: [f32; 4],
    ) {
        let pipeline = &self.device;
        let viewport = D3D10_VIEWPORT {
            Width: extent.width(),
            Height: extent.height(),
            MaxDepth: 1.0,
            ..Default::default()
        };
        // The input and output views are separate. UpdateSubresource copies 16 bytes.
        unsafe {
            pipeline.RSSetViewports(Some(&[viewport]));
            pipeline.PSSetShaderResources(0, Some(&[None]));
            pipeline.OMSetRenderTargets(Some(&[Some(target.color.clone())]), target.stencil);
            pipeline.PSSetShaderResources(0, Some(&[Some(source.clone())]));
            pipeline.UpdateSubresource(&self.constants, 0, None, parameters.as_ptr().cast(), 0, 0);
            pipeline.Draw(3, 0);
        }
    }
}

impl Backend for D3d10 {
    type Mask = ID3D10ShaderResourceView;
    type Target<'a> = Target<'a>;
    type Error = Error;

    fn draw_classic(
        &mut self,
        mask: &Self::Mask,
        target: &Self::Target<'_>,
        passes: ClassicPasses,
    ) -> Result<()> {
        let mut target_desc = D3D10_RENDER_TARGET_VIEW_DESC::default();
        // The typed COM view initializes this descriptor.
        unsafe { target.color.GetDesc(&mut target_desc) };
        if !is_srgb(target_desc.Format) {
            return Err(Error::new(
                E_INVALIDARG,
                "Classic glow requires an sRGB scene color view",
            ));
        }

        if self
            .scratch
            .as_ref()
            .is_none_or(|s| s.extent != passes.mask_extent)
        {
            self.scratch = Some(Scratch {
                extent: passes.mask_extent,
                horizontal: self.texture(passes.mask_extent, DXGI_FORMAT_R8G8B8A8_UNORM)?,
                vertical: self.texture(passes.mask_extent, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB)?,
            });
        }
        let scratch = self.scratch.as_ref().unwrap();
        let pipeline = &self.device;
        // These passes own the touched bindings. The caller rebinds its pipeline afterwards.
        unsafe {
            pipeline.IASetInputLayout(None::<&ID3D10InputLayout>);
            pipeline.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            pipeline.SetPredication(None::<&ID3D10Predicate>, false);
            pipeline.SOSetTargets(0, None, None);
            pipeline.GSSetShader(None::<&ID3D10GeometryShader>);
            pipeline.VSSetShader(&self.vertex);
            pipeline.PSSetShader(&self.blur);
            pipeline.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            pipeline.PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            pipeline.RSSetState(&self.rasterizer);
            pipeline.OMSetBlendState(None::<&ID3D10BlendState>, &[0.0; 4], u32::MAX);
            pipeline.OMSetDepthStencilState(&self.depth_disabled, 0);
            self.pass(
                mask,
                &Target {
                    color: &scratch.horizontal.color,
                    stencil: None,
                },
                passes.mask_extent,
                passes.horizontal,
            );
            self.pass(
                &scratch.horizontal.sample,
                &Target {
                    color: &scratch.vertical.color,
                    stencil: None,
                },
                passes.mask_extent,
                passes.vertical,
            );

            pipeline.OMSetBlendState(&self.blend, &[0.0; 4], u32::MAX);
            if target.stencil.is_some() {
                pipeline.OMSetDepthStencilState(&self.stencil_test, 0);
            }
            pipeline.PSSetShader(&self.composite);
            self.pass(
                &scratch.vertical.sample,
                target,
                passes.output_extent,
                passes.composite,
            );
            pipeline.PSSetShaderResources(0, Some(&[None]));
        }
        Ok(())
    }
}
