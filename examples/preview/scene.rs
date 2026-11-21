use std::f32::consts::TAU;
use std::ptr::null;
use std::time::Duration;
use tinyglow::d3d9::{D3d9, Target};
use tinyglow::{ClassicGlow, Extent, GlowStyle, StyledGlow};
use windows::Win32::Foundation::{E_FAIL, HWND};
use windows::Win32::Graphics::Direct3D9::*;
use windows::core::{Error, Result};

pub const WIDTH: u32 = 944;
pub const HEIGHT: u32 = 468;
const PANEL: f32 = WIDTH as f32 / 2.0;

#[derive(Clone, Copy)]
pub struct Effect {
    pub glow: ClassicGlow,
    pub style: Option<GlowStyle>,
    pub elapsed: Duration,
}

pub struct Scene {
    device: IDirect3DDevice9,
    scene: IDirect3DSurface9,
    depth: IDirect3DSurface9,
    backend: D3d9,
    mask: Option<(Extent, IDirect3DTexture9, IDirect3DSurface9)>,
    mesh: Vec<Triangle>,
    backdrop: Vec<Vertex>,
    vertices: Vec<Vertex>,
}

impl Scene {
    pub fn new(window: HWND, viewport: HWND) -> Result<Self> {
        // The window procedure creates both windows on this render thread.
        unsafe {
            let d3d = Direct3DCreate9(D3D_SDK_VERSION)
                .ok_or_else(|| Error::new(E_FAIL, "Direct3D 9 is unavailable"))?;
            let mut present = D3DPRESENT_PARAMETERS {
                BackBufferWidth: WIDTH,
                BackBufferHeight: HEIGHT,
                BackBufferFormat: D3DFMT_X8R8G8B8,
                SwapEffect: D3DSWAPEFFECT_DISCARD,
                hDeviceWindow: viewport,
                Windowed: true.into(),
                EnableAutoDepthStencil: true.into(),
                AutoDepthStencilFormat: D3DFMT_D24S8,
                PresentationInterval: D3DPRESENT_INTERVAL_ONE as u32,
                ..Default::default()
            };
            let mut device = None;
            d3d.CreateDevice(
                0,
                D3DDEVTYPE_HAL,
                window,
                D3DCREATE_HARDWARE_VERTEXPROCESSING as u32,
                &mut present,
                &mut device,
            )?;
            let device = device.ok_or_else(|| Error::from_hresult(E_FAIL))?;
            let mesh = torus();
            Ok(Self {
                scene: device.GetRenderTarget(0)?,
                depth: device.GetDepthStencilSurface()?,
                backend: D3d9::new(&device)?,
                device,
                backdrop: backdrop(),
                vertices: Vec::with_capacity(mesh.len() * 3),
                mesh,
                mask: None,
            })
        }
    }

    pub fn render(&mut self, angle: f32, glow: Option<Effect>) -> Result<()> {
        let output = Extent::new(WIDTH, HEIGHT).unwrap();
        if let Some(glow) = glow {
            let extent = glow.glow.mask_extent(output);
            if self.mask.as_ref().is_none_or(|mask| mask.0 != extent) {
                let texture = self.backend.create_mask(extent)?;
                // This is a single-level render-target texture created above.
                let surface = unsafe { texture.GetSurfaceLevel(0)? };
                self.mask = Some((extent, texture, surface));
            }
        }

        // Finish the scene even if a drawing call fails.
        unsafe { self.device.BeginScene()? };
        let draw = self.draw(angle, glow, output);
        let end = unsafe { self.device.EndScene() };
        draw.and(end)?;
        // Present to the viewport selected at device creation.
        unsafe { self.device.Present(null(), null(), HWND::default(), null()) }
    }

    fn draw(&mut self, angle: f32, glow: Option<Effect>, output: Extent) -> Result<()> {
        let device = &self.device;
        // The device, current render target and all draw data belong to this thread.
        unsafe {
            device.Clear(
                0,
                null(),
                (D3DCLEAR_TARGET | D3DCLEAR_ZBUFFER | D3DCLEAR_STENCIL) as u32,
                0xff111925,
                1.0,
                1,
            )?;
            device.SetFVF(D3DFVF_XYZRHW | D3DFVF_DIFFUSE)?;
            device.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE.0 as u32)?;
            device.SetRenderState(D3DRS_LIGHTING, 0)?;
            device.SetRenderState(D3DRS_ZENABLE, 0)?;
            device.SetRenderState(D3DRS_STENCILENABLE, 0)?;
            draw_vertices(device, D3DPT_LINELIST, &self.backdrop, 2)?;

            let rotation = Rotation::new(angle);
            self.vertices.clear();
            for triangle in &self.mesh {
                triangle.project(rotation, &mut self.vertices);
            }

            device.SetRenderState(D3DRS_ZENABLE, 1)?;
            draw_vertices(device, D3DPT_TRIANGLELIST, &self.vertices, 3)?;
            for vertex in &mut self.vertices {
                vertex.x += PANEL;
            }

            // The visible mesh clears its own stencil coverage to zero.
            device.SetRenderState(D3DRS_STENCILENABLE, 1)?;
            device.SetRenderState(D3DRS_STENCILFUNC, D3DCMP_ALWAYS.0 as u32)?;
            device.SetRenderState(D3DRS_STENCILREF, 0)?;
            device.SetRenderState(D3DRS_STENCILWRITEMASK, 0xff)?;
            device.SetRenderState(D3DRS_STENCILPASS, D3DSTENCILOP_REPLACE.0 as u32)?;
            draw_vertices(device, D3DPT_TRIANGLELIST, &self.vertices, 3)?;
            device.SetRenderState(D3DRS_STENCILENABLE, 0)?;

            if let (Some(glow), Some((extent, texture, surface))) = (glow, &self.mask) {
                let sx = extent.width() as f32 / WIDTH as f32;
                let sy = extent.height() as f32 / HEIGHT as f32;
                for vertex in &mut self.vertices {
                    vertex.x = (vertex.x + 0.5) * sx - 0.5;
                    vertex.y = (vertex.y + 0.5) * sy - 0.5;
                    vertex.color = 0xffff_ffff;
                }
                device.SetDepthStencilSurface(None::<&IDirect3DSurface9>)?;
                device.SetRenderTarget(0, surface)?;
                device.Clear(0, null(), D3DCLEAR_TARGET as u32, 0xff000000, 1.0, 0)?;
                device.SetRenderState(D3DRS_ZENABLE, 0)?;
                let mask_draw = draw_vertices(device, D3DPT_TRIANGLELIST, &self.vertices, 3);
                device.SetRenderTarget(0, &self.scene)?;
                device.SetDepthStencilSurface(&self.depth)?;
                mask_draw?;
                let target = Target {
                    color: &self.scene,
                    stencil: Some(&self.depth),
                };
                if let Some(style) = glow.style {
                    let mut styled = StyledGlow::new(style);
                    styled.glow = glow.glow;
                    styled.render(&mut self.backend, texture, &target, output, glow.elapsed)?;
                } else {
                    glow.glow
                        .render(&mut self.backend, texture, &target, output)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Vertex {
    x: f32,
    y: f32,
    z: f32,
    rhw: f32,
    color: u32,
}

unsafe fn draw_vertices(
    device: &IDirect3DDevice9,
    primitive: D3DPRIMITIVETYPE,
    vertices: &[Vertex],
    vertices_per_primitive: usize,
) -> Result<()> {
    // Vertex stores exactly the XYZRHW and DIFFUSE data declared by SetFVF.
    unsafe {
        device.DrawPrimitiveUP(
            primitive,
            (vertices.len() / vertices_per_primitive) as u32,
            vertices.as_ptr().cast(),
            size_of::<Vertex>() as u32,
        )
    }
}

fn backdrop() -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let mut line = |from: [f32; 2], to: [f32; 2], color| {
        vertices.extend([from, to].map(|[x, y]| Vertex {
            x,
            y,
            z: 0.9,
            rhw: 1.0,
            color,
        }));
    };
    for panel in [0.0, PANEL] {
        for row in 0..8 {
            let depth = row as f32 / 7.0;
            let y = 317.0 + 111.0 * depth.powf(1.6);
            let width = 128.0 + depth * 265.0;
            let center = panel + PANEL * 0.5;
            line(
                [center - width * 0.5, y],
                [center + width * 0.5, y],
                0xff253447,
            );
        }
        for column in -8..=8 {
            let offset = column as f32;
            let center = panel + PANEL * 0.5;
            line(
                [center + offset * 8.0, 317.0],
                [center + offset * 24.5, 428.0],
                0xff253447,
            );
        }
    }
    line([PANEL, 0.0], [PANEL, HEIGHT as f32], 0xff324158);
    vertices
}

type Vec3 = [f32; 3];

#[derive(Clone, Copy)]
struct Point {
    position: Vec3,
    normal: Vec3,
}

struct Triangle([Point; 3]);

impl Triangle {
    fn project(&self, rotation: Rotation, vertices: &mut Vec<Vertex>) {
        let [nx, ny, nz] = rotation.apply(self.0[0].normal);
        let diffuse = (-0.45 * nx + 0.55 * ny - 0.7 * nz).max(0.0);
        let shine = diffuse.powi(9);
        let rgb = [
            35.0 + 95.0 * diffuse,
            57.0 + 108.0 * diffuse,
            79.0 + 123.0 * diffuse,
        ];
        let [r, g, b] = rgb.map(|channel| (channel + shine * 50.0) as u32);
        let color = 0xff000000 | r << 16 | g << 8 | b;
        vertices.extend(self.0.map(|point| {
            let [x, y, z] = rotation.apply(point.position);
            let rhw = (5.2 + z).recip();
            let scale = 535.0 * rhw;
            Vertex {
                x: PANEL * 0.5 + x * scale - 0.5,
                y: 216.0 - y * scale - 0.5,
                z: 0.5 + z * 0.2,
                rhw,
                color,
            }
        }));
    }
}

#[derive(Clone, Copy)]
struct Rotation {
    tilt: (f32, f32),
    turn: (f32, f32),
}

impl Rotation {
    fn new(angle: f32) -> Self {
        Self {
            tilt: (0.48 + 0.15 * angle.sin()).sin_cos(),
            turn: (0.22 + 0.75 * angle.sin()).sin_cos(),
        }
    }

    fn apply(self, [x, y, z]: Vec3) -> Vec3 {
        let (sx, cx) = self.tilt;
        let (sy, cy) = self.turn;
        let y_rotated = cx * y - sx * z;
        let z_rotated = sx * y + cx * z;
        [cy * x + sy * z_rotated, y_rotated, -sy * x + cy * z_rotated]
    }
}

fn torus() -> Vec<Triangle> {
    const SECTIONS: usize = 56;
    const SIDES: usize = 14;
    let at = |section, side| {
        let (su, cu) = (TAU * section as f32 / SECTIONS as f32).sin_cos();
        let (sv, cv) = (TAU * side as f32 / SIDES as f32).sin_cos();
        let radius = 1.13 + 0.37 * cv;
        Point {
            position: [radius * cu, radius * su, 0.37 * sv],
            normal: [cu * cv, su * cv, sv],
        }
    };
    let mut mesh = Vec::with_capacity(SECTIONS * SIDES * 2);
    for section in 0..SECTIONS {
        for side in 0..SIDES {
            let a = at(section, side);
            let b = at((section + 1) % SECTIONS, side);
            let c = at((section + 1) % SECTIONS, (side + 1) % SIDES);
            let d = at(section, (side + 1) % SIDES);
            mesh.extend([Triangle([a, b, c]), Triangle([a, c, d])]);
        }
    }
    mesh
}
