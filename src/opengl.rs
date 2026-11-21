//! Native desktop OpenGL 4.3 executor.
//!
//! This executor leaves the draw framebuffer and graphics pipeline configured
//! for the glow composite. Rebind your pipeline before the next renderer pass.

use crate::shaders::{self, Api};
use crate::{Backend, ClassicPasses, Extent};
use glow::HasContext as _;
use std::marker::PhantomData;
use std::rc::Rc;

pub struct Target {
    /// Draw framebuffer with one sRGB color output. Use `None` for the default framebuffer.
    pub framebuffer: Option<glow::Framebuffer>,
    /// Enable the reference's nonzero stencil test on the draw framebuffer.
    pub stencil: bool,
}

pub struct OpenGl<'a> {
    gl: &'a glow::Context,
    vertex_array: glow::VertexArray,
    sampler: glow::Sampler,
    blur: Program<'a>,
    composite: Program<'a>,
    scratch: Option<Scratch<'a>>,
    thread_bound: PhantomData<Rc<()>>,
}

struct Program<'a> {
    gl: &'a glow::Context,
    handle: glow::Program,
    parameters: glow::UniformLocation,
}

struct Scratch<'a> {
    extent: Extent,
    horizontal: RenderTexture<'a>,
    vertical: RenderTexture<'a>,
}

struct RenderTexture<'a> {
    gl: &'a glow::Context,
    texture: glow::Texture,
    framebuffer: glow::Framebuffer,
}

impl<'a> OpenGl<'a> {
    /// # Safety
    ///
    /// `gl` must refer to the current context during construction, use and
    /// destruction on this thread. Masks and framebuffers must be valid in
    /// that context and have the dimensions declared by the draw.
    pub unsafe fn new(gl: &'a glow::Context) -> Result<Self, String> {
        let version = gl.version();
        if version.is_embedded || (version.major, version.minor) < (4, 3) {
            return Err("Classic glow requires desktop OpenGL 4.3".into());
        }

        let shaders = shaders::classic(Api::OpenGl);
        let vertex = shaders.fullscreen_vertex.source;
        let blur = Program::new(gl, vertex, shaders.blur_fragment.source)?;
        let composite = Program::new(gl, vertex, shaders.composite_fragment.source)?;
        // The construction contract supplies a current OpenGL 4.3 context.
        unsafe {
            let vertex_array = gl.create_vertex_array()?;
            let sampler = gl
                .create_sampler()
                .inspect_err(|_| gl.delete_vertex_array(vertex_array))?;
            for (state, value) in [
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
            ] {
                gl.sampler_parameter_i32(sampler, state, value as i32);
            }
            Ok(Self {
                gl,
                vertex_array,
                sampler,
                blur,
                composite,
                scratch: None,
                thread_bound: PhantomData,
            })
        }
    }
}

impl Backend for OpenGl<'_> {
    type Mask = glow::Texture;
    type Target<'a> = Target;
    type Error = String;

    fn draw_classic(
        &mut self,
        mask: &Self::Mask,
        target: &Self::Target<'_>,
        passes: ClassicPasses,
    ) -> Result<(), String> {
        let gl = self.gl;
        let mask_size = viewport_size(passes.mask_extent)?;
        let output_size = viewport_size(passes.output_extent)?;
        // The construction contract keeps the context current and the caller's
        // framebuffer and mask valid. The scratch FBOs each have one attachment.
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            if self
                .scratch
                .as_ref()
                .is_none_or(|scratch| scratch.extent != passes.mask_extent)
            {
                self.scratch = Some(Scratch {
                    extent: passes.mask_extent,
                    horizontal: RenderTexture::new(gl, mask_size, glow::RGBA8)?,
                    // With FRAMEBUFFER_SRGB disabled, blur writes directly to
                    // the stored bytes. The composite samples with sRGB decoding.
                    vertical: RenderTexture::new(gl, mask_size, glow::SRGB8_ALPHA8)?,
                });
            }
            let scratch = self.scratch.as_ref().unwrap();
            gl.bind_vertex_array(Some(self.vertex_array));
            gl.bind_sampler(0, Some(self.sampler));
            gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            for capability in [
                glow::DEPTH_TEST,
                glow::STENCIL_TEST,
                glow::SCISSOR_TEST,
                glow::CULL_FACE,
                glow::RASTERIZER_DISCARD,
                glow::SAMPLE_ALPHA_TO_COVERAGE,
                glow::SAMPLE_ALPHA_TO_ONE,
                glow::SAMPLE_COVERAGE,
                glow::SAMPLE_MASK,
                glow::COLOR_LOGIC_OP,
                glow::DITHER,
                glow::FRAMEBUFFER_SRGB,
                glow::BLEND,
            ] {
                gl.disable(capability);
            }
            gl.color_mask(true, true, true, true);
            gl.depth_mask(false);
            gl.viewport(0, 0, mask_size.0, mask_size.1);
            self.blur.draw(
                *mask,
                Some(scratch.horizontal.framebuffer),
                &passes.horizontal,
            );
            self.blur.draw(
                scratch.horizontal.texture,
                Some(scratch.vertical.framebuffer),
                &passes.vertical,
            );

            gl.viewport(0, 0, output_size.0, output_size.1);
            gl.enable(glow::FRAMEBUFFER_SRGB);
            gl.enable(glow::BLEND);
            gl.blend_equation(glow::FUNC_ADD);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            if target.stencil {
                gl.enable(glow::STENCIL_TEST);
                gl.stencil_func(glow::NOTEQUAL, 0, 0xff);
                gl.stencil_mask(0);
                gl.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
            }
            self.composite.draw(
                scratch.vertical.texture,
                target.framebuffer,
                &passes.composite,
            );
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
        Ok(())
    }
}

impl Drop for OpenGl<'_> {
    fn drop(&mut self) {
        // The constructor requires this context to be current at destruction.
        unsafe {
            self.gl.delete_sampler(self.sampler);
            self.gl.delete_vertex_array(self.vertex_array);
        }
    }
}

impl<'a> Program<'a> {
    fn new(gl: &'a glow::Context, vertex: &str, fragment: &str) -> Result<Self, String> {
        // The executor is created and used with a current context.
        unsafe {
            let vs = compile(gl, glow::VERTEX_SHADER, vertex)?;
            let fs = compile(gl, glow::FRAGMENT_SHADER, fragment)
                .inspect_err(|_| gl.delete_shader(vs))?;
            let created = gl.create_program();
            if let Ok(program) = created {
                gl.attach_shader(program, vs);
                gl.attach_shader(program, fs);
                gl.link_program(program);
                gl.detach_shader(program, fs);
                gl.detach_shader(program, vs);
            }
            gl.delete_shader(fs);
            gl.delete_shader(vs);

            let handle = created?;
            if !gl.get_program_link_status(handle) {
                let log = gl.get_program_info_log(handle);
                gl.delete_program(handle);
                return Err(log);
            }
            let Some(parameters) = gl.get_uniform_location(handle, "parameters") else {
                gl.delete_program(handle);
                return Err("Classic glow shader is missing its parameters uniform".into());
            };
            Ok(Self {
                gl,
                handle,
                parameters,
            })
        }
    }

    unsafe fn draw(
        &self,
        texture: glow::Texture,
        framebuffer: Option<glow::Framebuffer>,
        parameters: &[f32; 4],
    ) {
        // The caller binds the fullscreen VAO and sampler and ensures that
        // the sampled texture and destination attachment are separate.
        unsafe {
            self.gl
                .bind_framebuffer(glow::DRAW_FRAMEBUFFER, framebuffer);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl.use_program(Some(self.handle));
            self.gl
                .uniform_4_f32_slice(Some(&self.parameters), parameters);
            self.gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }
    }
}

impl Drop for Program<'_> {
    fn drop(&mut self) {
        unsafe { self.gl.delete_program(self.handle) };
    }
}

impl<'a> RenderTexture<'a> {
    unsafe fn new(gl: &'a glow::Context, size: (i32, i32), format: u32) -> Result<Self, String> {
        // The draw owns the GL bindings used for allocation.
        unsafe {
            let texture = gl.create_texture()?;
            let framebuffer = gl
                .create_framebuffer()
                .inspect_err(|_| gl.delete_texture(texture))?;
            let target = Self {
                gl,
                texture,
                framebuffer,
            };
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_storage_2d(glow::TEXTURE_2D, 1, format, size.0, size.1);
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(framebuffer));
            gl.framebuffer_texture_2d(
                glow::DRAW_FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
            let status = gl.check_framebuffer_status(glow::DRAW_FRAMEBUFFER);
            if status != glow::FRAMEBUFFER_COMPLETE {
                return Err(format!(
                    "Classic glow scratch framebuffer is incomplete (0x{status:x})"
                ));
            }
            Ok(target)
        }
    }
}

impl Drop for RenderTexture<'_> {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_framebuffer(self.framebuffer);
            self.gl.delete_texture(self.texture);
        }
    }
}

unsafe fn compile(gl: &glow::Context, kind: u32, source: &str) -> Result<glow::Shader, String> {
    unsafe {
        let shader = gl.create_shader(kind)?;
        gl.shader_source(shader, source);
        gl.compile_shader(shader);
        if gl.get_shader_compile_status(shader) {
            return Ok(shader);
        }
        let log = gl.get_shader_info_log(shader);
        gl.delete_shader(shader);
        Err(log)
    }
}

fn viewport_size(extent: Extent) -> Result<(i32, i32), String> {
    match (
        i32::try_from(extent.width()),
        i32::try_from(extent.height()),
    ) {
        (Ok(width), Ok(height)) => Ok((width, height)),
        _ => Err("Classic glow extent exceeds the OpenGL viewport range".into()),
    }
}
