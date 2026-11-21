#![cfg(all(windows, feature = "d3d11"))]

use std::time::Duration;
use tinyglow::d3d11::{D3d11, Target};
use tinyglow::{ClassicGlow, Extent, GlowStyle, StyledGlow};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_WARP;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::{Common::*, IDXGIAdapter};
use windows::core::Result;

#[test]
fn classic_and_styled_shaders_link_and_draw_on_warp() -> Result<()> {
    const SIDE: u32 = 32;
    let extent = Extent::new(SIDE, SIDE).unwrap();
    // The resources and initial data stay alive through their API calls.
    unsafe {
        let mut device = None;
        let mut context = None;
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG::default(),
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
        let device = device.unwrap();
        let context = context.unwrap();
        let mut backend = D3d11::new(&context)?;
        let base = D3D11_TEXTURE2D_DESC {
            Width: SIDE,
            Height: SIDE,
            MipLevels: 1,
            ArraySize: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };

        let mut mask_pixels = [0xff00_0000_u32; (SIDE * SIDE) as usize];
        for y in 12..20 {
            let row = y * SIDE as usize;
            mask_pixels[row + 12..row + 20].fill(u32::MAX);
        }
        let mask_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: mask_pixels.as_ptr().cast(),
            SysMemPitch: SIDE * 4,
            ..Default::default()
        };
        let mut mask = None;
        device.CreateTexture2D(
            &D3D11_TEXTURE2D_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                ..base
            },
            Some(&mask_data),
            Some(&mut mask),
        )?;
        let mut view = None;
        device.CreateShaderResourceView(&mask.unwrap(), None, Some(&mut view))?;
        let view = view.unwrap();

        let scene_format = DXGI_FORMAT_R8G8B8A8_UNORM_SRGB;
        let mut scene = None;
        device.CreateTexture2D(
            &D3D11_TEXTURE2D_DESC {
                Format: scene_format,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                ..base
            },
            None,
            Some(&mut scene),
        )?;
        let scene = scene.unwrap();
        let mut color = None;
        device.CreateRenderTargetView(&scene, None, Some(&mut color))?;
        let color = color.unwrap();
        context.ClearRenderTargetView(&color, &[0.0, 0.0, 0.0, 127.0 / 255.0]);

        let glow = ClassicGlow {
            color: [255, 0, 0],
            thickness: 1.0,
            ..Default::default()
        };
        glow.render(
            &mut backend,
            &view,
            &Target {
                color: &color,
                stencil: None,
            },
            extent,
        )?;

        let mut readback = None;
        device.CreateTexture2D(
            &D3D11_TEXTURE2D_DESC {
                Format: scene_format,
                Usage: D3D11_USAGE_STAGING,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                ..base
            },
            None,
            Some(&mut readback),
        )?;
        let readback = readback.unwrap();
        let pixel = |scene: &ID3D11Texture2D, x: usize, y: usize| -> Result<[u8; 4]> {
            context.CopyResource(&readback, scene);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            context.Map(&readback, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let offset = y * mapped.RowPitch as usize + x * 4;
            let rgba = mapped
                .pData
                .cast::<u8>()
                .add(offset)
                .cast::<[u8; 4]>()
                .read();
            context.Unmap(&readback, 0);
            Ok(rgba)
        };

        let rgba = pixel(&scene, 10, 16)?;
        assert!(rgba[0] > 0, "the glow should reach past the silhouette");
        assert_eq!(&rgba[1..], &[0, 0, 127]);

        let mut scene = None;
        device.CreateTexture2D(
            &D3D11_TEXTURE2D_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                ..base
            },
            None,
            Some(&mut scene),
        )?;
        let scene = scene.unwrap();
        let mut color = None;
        device.CreateRenderTargetView(&scene, None, Some(&mut color))?;
        let color = color.unwrap();
        let target = Target {
            color: &color,
            stencil: None,
        };
        for &style in GlowStyle::ALL {
            context.ClearRenderTargetView(&color, &[0.0, 0.0, 0.0, 127.0 / 255.0]);
            let effect = StyledGlow {
                glow,
                ..StyledGlow::new(style)
            };
            effect
                .render(
                    &mut backend,
                    &view,
                    &target,
                    extent,
                    Duration::from_millis(1250),
                )
                .unwrap_or_else(|error| panic!("{}: {error}", style.name()));
            if style == GlowStyle::Neon {
                assert!(
                    pixel(&scene, 10, 16)?[0] > rgba[0],
                    "styled Neon uses raw blur coverage"
                );
            }
            if style == GlowStyle::XRay {
                assert!(
                    pixel(&scene, 16, 16)?[0] > 0,
                    "X-Ray samples the original silhouette"
                );
            }
        }
    }
    Ok(())
}
