//! Native Direct3D 12 command-list executor.
//!
//! The renderer owns command submission and synchronization. Tinyglow owns
//! the root signature, pipeline states, scratch textures, and shader heap.

use crate::d3d_shader::{blob_bytes, compile, diagnostic, is_srgb, out};
use crate::shaders::{self, Api, Shader};
use crate::{Backend, ClassicPasses, Extent};
use std::mem::ManuallyDrop;
use windows::Win32::Foundation::{E_INVALIDARG, RECT};
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Error, Result};

pub struct Target<'a> {
    list: &'a ID3D12GraphicsCommandList,
    color: D3D12_CPU_DESCRIPTOR_HANDLE,
    stencil: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
}

impl<'a> Target<'a> {
    /// Describes the command list and output views for a glow draw.
    ///
    /// # Safety
    ///
    /// Use a recording direct graphics list on the executor's device, outside
    /// any render pass. The CPU descriptors must describe a single-sample
    /// scene RTV and optional matching DSV in the formats configured on the
    /// executor. Their dimensions must match the extent passed to `render`.
    /// Put the scene color in RENDER_TARGET state, the silhouette in
    /// PIXEL_SHADER_RESOURCE, and the depth/stencil in the state required by
    /// its DSV. Keep GPU resources, the executor and the mask alive until the
    /// fence for this draw completes. Use the mask on one ordered direct queue,
    /// or synchronize accesses to it across queues.
    pub unsafe fn new(
        list: &'a ID3D12GraphicsCommandList,
        color: D3D12_CPU_DESCRIPTOR_HANDLE,
        stencil: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
    ) -> Self {
        Self {
            list,
            color,
            stencil,
        }
    }
}

/// A silhouette SRV and its pair of reusable scratch textures.
pub struct Mask {
    extent: Extent,
    device: ID3D12Device,
    _source: ID3D12Resource,
    shader_heap: ID3D12DescriptorHeap,
    _rtv_heap: ID3D12DescriptorHeap,
    srvs: [D3D12_GPU_DESCRIPTOR_HANDLE; 3],
    rtvs: [D3D12_CPU_DESCRIPTOR_HANDLE; 2],
    scratch: [ID3D12Resource; 2],
}

pub struct D3d12 {
    device: ID3D12Device,
    root: ID3D12RootSignature,
    blur: ID3D12PipelineState,
    composite: ID3D12PipelineState,
    stencil_composite: Option<ID3D12PipelineState>,
}

impl D3d12 {
    /// Creates pipeline states for a single-sample sRGB scene color format.
    /// Supply the scene stencil view format to enable stencil-masked draws.
    pub fn new(
        device: &ID3D12Device,
        color_format: DXGI_FORMAT,
        stencil_format: Option<DXGI_FORMAT>,
    ) -> Result<Self> {
        if !is_srgb(color_format) {
            return Err(Error::new(
                E_INVALIDARG,
                "Classic glow requires an sRGB scene color view",
            ));
        }
        if stencil_format.is_some_and(|format| {
            !matches!(
                format,
                DXGI_FORMAT_D24_UNORM_S8_UINT | DXGI_FORMAT_D32_FLOAT_S8X24_UINT
            )
        }) {
            return Err(Error::new(
                E_INVALIDARG,
                "The configured depth format has no stencil plane",
            ));
        }

        let shaders = shaders::classic(Api::Direct3D12);
        // D3D12 accepts Shader Model 5 DXBC, allowing the same compiler DLL as DX9–11.
        let vertex = compile(Shader {
            profile: "vs_5_0",
            ..shaders.fullscreen_vertex
        })?;
        let blur = compile(Shader {
            profile: "ps_5_0",
            ..shaders.blur_fragment
        })?;
        let composite = compile(Shader {
            profile: "ps_5_0",
            ..shaders.composite_fragment
        })?;
        let root = root_signature(device)?;
        let blur_pipeline = pipeline(
            device,
            &root,
            &vertex,
            &blur,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            false,
            None,
        )?;
        let composite_pipeline =
            pipeline(device, &root, &vertex, &composite, color_format, true, None)?;
        let stencil_composite = stencil_format
            .map(|format| {
                pipeline(
                    device,
                    &root,
                    &vertex,
                    &composite,
                    color_format,
                    true,
                    Some(format),
                )
            })
            .transpose()?;
        Ok(Self {
            device: device.clone(),
            root,
            blur: blur_pipeline,
            composite: composite_pipeline,
            stencil_composite,
        })
    }

    /// Creates a linear SRV and scratch resources for the supplied texture.
    ///
    /// Accepted inputs are shader-readable RGBA8 UNORM or TYPELESS textures
    /// with one mip, one array slice, and one sample.
    pub fn create_mask(&self, source: &ID3D12Resource) -> Result<Mask> {
        // The resource header and owning device are available without mapping it.
        let desc = unsafe { source.GetDesc() };
        let mut device = None;
        unsafe { source.GetDevice(&mut device)? };
        let extent = u32::try_from(desc.Width)
            .ok()
            .and_then(|width| Extent::new(width, desc.Height));
        let Some(extent) = extent.filter(|_| {
            device.as_ref() == Some(&self.device)
                && desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D
                && desc.DepthOrArraySize == 1
                && desc.MipLevels == 1
                && desc.SampleDesc.Count == 1
                && !desc
                    .Flags
                    .contains(D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE)
                && matches!(
                    desc.Format,
                    DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_TYPELESS
                )
        }) else {
            return Err(Error::new(
                E_INVALIDARG,
                "Classic glow requires a single-sample linear RGBA8 mask on its device",
            ));
        };
        let scratch = [
            scratch_texture(&self.device, extent)?,
            scratch_texture(&self.device, extent)?,
        ];

        // The heaps are private. View descriptors remain valid for their entire lifetime.
        unsafe {
            let shader_heap: ID3D12DescriptorHeap =
                self.device
                    .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                        Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                        NumDescriptors: 3,
                        Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                        NodeMask: 0,
                    })?;
            let rtv_heap: ID3D12DescriptorHeap =
                self.device
                    .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                        Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                        NumDescriptors: 2,
                        ..Default::default()
                    })?;
            let shader_stride = self
                .device
                .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
            let rtv_stride = self
                .device
                .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
            let srv_cpu = shader_heap.GetCPUDescriptorHandleForHeapStart().ptr;
            let srv_gpu = shader_heap.GetGPUDescriptorHandleForHeapStart().ptr;
            let rtv_cpu = rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr;
            let srvs = std::array::from_fn(|index| D3D12_GPU_DESCRIPTOR_HANDLE {
                ptr: srv_gpu + index as u64 * u64::from(shader_stride),
            });
            let rtvs = std::array::from_fn(|index| D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: rtv_cpu + index * rtv_stride as usize,
            });
            for (index, resource) in [source, &scratch[0], &scratch[1]].into_iter().enumerate() {
                let sample = D3D12_SHADER_RESOURCE_VIEW_DESC {
                    Format: if index == 2 {
                        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
                    } else {
                        DXGI_FORMAT_R8G8B8A8_UNORM
                    },
                    ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                    Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_SRV {
                            MipLevels: 1,
                            ..Default::default()
                        },
                    },
                };
                self.device.CreateShaderResourceView(
                    resource,
                    Some(&sample),
                    D3D12_CPU_DESCRIPTOR_HANDLE {
                        ptr: srv_cpu + index * shader_stride as usize,
                    },
                );
            }
            let render = D3D12_RENDER_TARGET_VIEW_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D,
                Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 {
                    Texture2D: D3D12_TEX2D_RTV::default(),
                },
            };
            for (resource, descriptor) in scratch.iter().zip(rtvs) {
                self.device
                    .CreateRenderTargetView(resource, Some(&render), descriptor);
            }
            Ok(Mask {
                device: self.device.clone(),
                extent,
                _source: source.clone(),
                shader_heap,
                _rtv_heap: rtv_heap,
                srvs,
                rtvs,
                scratch,
            })
        }
    }
}

impl Backend for D3d12 {
    type Mask = Mask;
    type Target<'a> = Target<'a>;
    type Error = Error;

    fn draw_classic(
        &mut self,
        mask: &Mask,
        target: &Target<'_>,
        passes: ClassicPasses,
    ) -> Result<()> {
        if mask.extent != passes.mask_extent || mask.device != self.device {
            return Err(Error::new(
                E_INVALIDARG,
                "Classic glow mask does not match the extent or device",
            ));
        }
        // Scissors use signed coordinates. Check the conversion before recording anything.
        let mask_rect = scissor(passes.mask_extent)?;
        let output_rect = scissor(passes.output_extent)?;
        let composite = match target.stencil {
            Some(_) => self.stencil_composite.as_ref().ok_or_else(|| {
                Error::new(
                    E_INVALIDARG,
                    "The executor was created without a stencil format",
                )
            })?,
            None => &self.composite,
        };
        let list = target.list;
        // The target contract keeps the descriptors and recorded objects live.
        unsafe {
            list.SetDescriptorHeaps(&[Some(mask.shader_heap.clone())]);
            list.SetGraphicsRootSignature(&self.root);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            list.SetPredication(None::<&ID3D12Resource>, 0, D3D12_PREDICATION_OP_EQUAL_ZERO);
            list.OMSetStencilRef(0);
            list.SetPipelineState(&self.blur);

            set_viewport(list, mask_rect);
            draw(list, mask.srvs[0], mask.rtvs[0], None, passes.horizontal);
            transition(
                list,
                &mask.scratch[0],
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
            );
            draw(list, mask.srvs[1], mask.rtvs[1], None, passes.vertical);
            transition(
                list,
                &mask.scratch[1],
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
            );

            list.SetPipelineState(composite);
            set_viewport(list, output_rect);
            draw(
                list,
                mask.srvs[2],
                target.color,
                target.stencil,
                passes.composite,
            );
            for scratch in &mask.scratch {
                transition(
                    list,
                    scratch,
                    D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                    D3D12_RESOURCE_STATE_RENDER_TARGET,
                );
            }
        }
        Ok(())
    }
}

fn root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
    let range = D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
        NumDescriptors: 1,
        ..Default::default()
    };
    let params = [
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Constants: D3D12_ROOT_CONSTANTS {
                    Num32BitValues: 4,
                    ..Default::default()
                },
            },
        },
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: &range,
                },
            },
        },
    ];
    let sampler = D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT,
        AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D12_COMPARISON_FUNC_ALWAYS,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        ..Default::default()
    };
    let desc = D3D12_ROOT_SIGNATURE_DESC {
        NumParameters: params.len() as u32,
        pParameters: params.as_ptr(),
        NumStaticSamplers: 1,
        pStaticSamplers: &sampler,
        ..Default::default()
    };
    let mut diagnostics = None;
    // All pointers in the root signature descriptor refer to local, live values.
    let code = out(|blob| unsafe {
        D3D12SerializeRootSignature(
            &desc,
            D3D_ROOT_SIGNATURE_VERSION_1,
            blob,
            Some(&mut diagnostics),
        )
    })
    .map_err(|error| diagnostic(error, diagnostics))?;
    unsafe { device.CreateRootSignature(0, blob_bytes(&code)) }
}

fn pipeline(
    device: &ID3D12Device,
    root: &ID3D12RootSignature,
    vertex: &ID3DBlob,
    fragment: &ID3DBlob,
    color: DXGI_FORMAT,
    blend: bool,
    stencil: Option<DXGI_FORMAT>,
) -> Result<ID3D12PipelineState> {
    let code = |blob: &ID3DBlob| {
        let bytes = blob_bytes(blob);
        D3D12_SHADER_BYTECODE {
            pShaderBytecode: bytes.as_ptr().cast(),
            BytecodeLength: bytes.len(),
        }
    };
    let color_blend = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: blend.into(),
        SrcBlend: D3D12_BLEND_ONE,
        DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ONE,
        DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        LogicOp: D3D12_LOGIC_OP_NOOP,
        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
        ..Default::default()
    };
    let face = D3D12_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D12_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
        StencilPassOp: D3D12_STENCIL_OP_KEEP,
        StencilFunc: D3D12_COMPARISON_FUNC_NOT_EQUAL,
    };
    let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: ManuallyDrop::new(Some(root.clone())),
        VS: code(vertex),
        PS: code(fragment),
        BlendState: D3D12_BLEND_DESC {
            RenderTarget: [color_blend; 8],
            ..Default::default()
        },
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_NONE,
            DepthClipEnable: true.into(),
            ..Default::default()
        },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
            DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO,
            DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            StencilEnable: stencil.is_some().into(),
            StencilReadMask: 0xff,
            FrontFace: face,
            BackFace: face,
            ..Default::default()
        },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        DSVFormat: stencil.unwrap_or(DXGI_FORMAT_UNKNOWN),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        ..Default::default()
    };
    desc.RTVFormats[0] = color;
    // Shader blobs live through creation. Release the COM reference in the FFI struct.
    unsafe {
        let result = device.CreateGraphicsPipelineState(&desc);
        ManuallyDrop::drop(&mut desc.pRootSignature);
        result
    }
}

fn scratch_texture(device: &ID3D12Device, extent: Extent) -> Result<ID3D12Resource> {
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_DEFAULT,
        CreationNodeMask: 1,
        VisibleNodeMask: 1,
        ..Default::default()
    };
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Width: u64::from(extent.width()),
        Height: extent.height(),
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_R8G8B8A8_TYPELESS,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
        ..Default::default()
    };
    // Both descriptors live through resource creation.
    unsafe {
        out(|resource| {
            device.CreateCommittedResource(
                &heap,
                D3D12_HEAP_FLAG_NONE,
                &desc,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                None,
                resource,
            )
        })
    }
}

fn scissor(extent: Extent) -> Result<RECT> {
    let invalid = |_| {
        Error::new(
            E_INVALIDARG,
            "Classic glow dimensions exceed the scissor range",
        )
    };
    Ok(RECT {
        right: i32::try_from(extent.width()).map_err(invalid)?,
        bottom: i32::try_from(extent.height()).map_err(invalid)?,
        ..Default::default()
    })
}

unsafe fn set_viewport(list: &ID3D12GraphicsCommandList, rect: RECT) {
    let viewport = D3D12_VIEWPORT {
        Width: rect.right as f32,
        Height: rect.bottom as f32,
        MaxDepth: 1.0,
        ..Default::default()
    };
    unsafe {
        list.RSSetViewports(&[viewport]);
        list.RSSetScissorRects(&[rect]);
    }
}

unsafe fn draw(
    list: &ID3D12GraphicsCommandList,
    sample: D3D12_GPU_DESCRIPTOR_HANDLE,
    color: D3D12_CPU_DESCRIPTOR_HANDLE,
    stencil: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
    constants: [f32; 4],
) {
    unsafe {
        list.OMSetRenderTargets(
            1,
            Some(&color),
            false,
            stencil.as_ref().map(std::ptr::from_ref),
        );
        list.SetGraphicsRootDescriptorTable(1, sample);
        list.SetGraphicsRoot32BitConstants(0, 4, constants.as_ptr().cast(), 0);
        list.DrawInstanced(3, 1, 0, 0);
    }
}

unsafe fn transition(
    list: &ID3D12GraphicsCommandList,
    resource: &ID3D12Resource,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
) {
    let mut barrier = D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: ManuallyDrop::new(Some(resource.clone())),
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: before,
                StateAfter: after,
            }),
        },
    };
    // The scratch object owns the resource past command submission.
    unsafe {
        list.ResourceBarrier(std::slice::from_ref(&barrier));
        let transition = &mut *barrier.Anonymous.Transition;
        ManuallyDrop::drop(&mut transition.pResource);
    }
}
