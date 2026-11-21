#![cfg(all(windows, feature = "d3d9"))]

use std::ptr::{null, null_mut};
use tinyglow::d3d9::{D3d9, Target};
use tinyglow::{ClassicGlow, Extent};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct3D9::*;
use windows::core::Result;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetDesktopWindow() -> HWND;
}

#[test]
#[ignore = "requires a desktop session and a Direct3D 9 device"]
fn classic_draws_halo_and_preserves_stencil_interior() -> Result<()> {
    let (width, height) = (80, 64);
    let extent = Extent::new(width, height).unwrap();
    // All COM resources stay alive through their device calls. The readback
    // pointer is used only while the system-memory surface is locked.
    unsafe {
        let window = GetDesktopWindow();
        let d3d = Direct3DCreate9(D3D_SDK_VERSION).expect("Direct3D 9 runtime");
        let mut params = D3DPRESENT_PARAMETERS {
            BackBufferWidth: width,
            BackBufferHeight: height,
            BackBufferFormat: D3DFMT_A8R8G8B8,
            SwapEffect: D3DSWAPEFFECT_DISCARD,
            hDeviceWindow: window,
            Windowed: true.into(),
            ..Default::default()
        };
        let mut device = None;
        d3d.CreateDevice(
            0,
            D3DDEVTYPE_HAL,
            window,
            D3DCREATE_HARDWARE_VERTEXPROCESSING as u32,
            &mut params,
            &mut device,
        )?;
        let device = device.unwrap();
        let glow = ClassicGlow {
            color: [255, 0, 0],
            ..ClassicGlow::default()
        };
        let mut backend = D3d9::new(&device)?;
        let mask = backend.create_mask(extent)?;
        let mask_surface = mask.GetSurfaceLevel(0)?;
        device.ColorFill(&mask_surface, null(), 0xff00_0000)?;
        let shape = RECT {
            left: 30,
            top: 22,
            right: 50,
            bottom: 42,
        };
        device.ColorFill(&mask_surface, &shape, 0xffff_ffff)?;

        let mut stencil = None;
        device.CreateDepthStencilSurface(
            width,
            height,
            D3DFMT_D24S8,
            D3DMULTISAMPLE_NONE,
            0,
            false,
            &mut stencil,
            null_mut(),
        )?;
        let stencil = stencil.unwrap();
        let scene = device.GetRenderTarget(0)?;
        device.SetDepthStencilSurface(&stencil)?;
        let background = 0x7f12_3456;
        device.Clear(
            0,
            null(),
            D3DCLEAR_TARGET as u32 | D3DCLEAR_STENCIL as u32,
            background,
            1.0,
            1,
        )?;
        let cutout = D3DRECT {
            x1: shape.left,
            y1: shape.top,
            x2: shape.right,
            y2: shape.bottom,
        };
        device.Clear(1, &cutout, D3DCLEAR_STENCIL as u32, 0, 1.0, 0)?;

        device.SetRenderState(D3DRS_SCISSORTESTENABLE, 1)?;
        device.BeginScene()?;
        let draw = glow.render(
            &mut backend,
            &mask,
            &Target {
                color: &scene,
                stencil: Some(&stencil),
            },
            extent,
        );
        device.EndScene()?;
        draw?;
        assert_eq!(device.GetRenderTarget(0)?, scene);
        assert_eq!(device.GetDepthStencilSurface()?, stencil);
        let mut scissor = 0;
        device.GetRenderState(D3DRS_SCISSORTESTENABLE, &mut scissor)?;
        assert_eq!(scissor, 1);

        let mut readback = None;
        device.CreateOffscreenPlainSurface(
            width,
            height,
            D3DFMT_A8R8G8B8,
            D3DPOOL_SYSTEMMEM,
            &mut readback,
            null_mut(),
        )?;
        let readback = readback.unwrap();
        device.GetRenderTargetData(&scene, &readback)?;
        let mut locked = D3DLOCKED_RECT::default();
        readback.LockRect(&mut locked, null(), D3DLOCK_READONLY as u32)?;
        let pixel = |x: isize, y: isize| {
            locked
                .pBits
                .cast::<u8>()
                .offset(y * locked.Pitch as isize + x * 4)
                .cast::<u32>()
                .read_unaligned()
        };
        let pixels = [pixel(0, 0), pixel(40, 32), pixel(28, 32), pixel(40, 20)];
        readback.UnlockRect()?;

        assert_eq!(pixels[0], background, "far from silhouette");
        assert_eq!(pixels[1], background, "inside stencil cutout");
        for halo in &pixels[2..] {
            assert_eq!(halo & 0xff00_ffff, background & 0xff00_ffff);
            assert!(halo & 0x00ff_0000 > background & 0x00ff_0000);
        }
    }
    Ok(())
}
