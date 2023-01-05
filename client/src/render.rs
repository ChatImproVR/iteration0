use anyhow::format_err;
use anyhow::Result;
use cimvr_common::FrameTime;
use cimvr_common::{render::*, Transform};
use cimvr_engine::interface::prelude::Component;
use cimvr_engine::interface::prelude::QueryComponent;
use cimvr_engine::{interface::prelude::Access, Engine};
use gl::HasContext;
use glutin::dpi::PhysicalSize;
use nalgebra::Matrix4;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub struct RenderPlugin {
    gl: Arc<glow::Context>,
    rdr: RenderEngine,
    /// Start time
    start_time: Instant,
    /// Time since last frame
    last_frame: Instant,
    screen_size: PhysicalSize<u32>,
}

impl RenderPlugin {
    pub fn new(gl: Arc<gl::Context>, engine: &mut Engine) -> Result<Self> {
        engine.subscribe::<RenderData>();

        let rdr = RenderEngine::new(&gl)?;

        Ok(Self {
            gl,
            rdr,
            screen_size: PhysicalSize::new(1024, 768),
            start_time: Instant::now(),
            last_frame: Instant::now(),
        })
    }

    pub fn set_screen_size(&mut self, size: PhysicalSize<u32>) {
        unsafe {
            self.gl.scissor(0, 0, size.width as i32, size.height as i32);
            self.gl
                .viewport(0, 0, size.width as i32, size.height as i32);
            self.screen_size = size;
        }
    }

    pub fn frame(&mut self, engine: &mut Engine) -> Result<()> {
        // Upload render data
        for msg in engine.inbox::<RenderData>() {
            self.rdr.upload(&self.gl, &msg)?;
        }

        // Find camera, if any
        let camera_entity = match engine.ecs().find(&[CameraComponent::ID, Transform::ID]) {
            Some(c) => c,
            None => {
                log::warn!("No Camera found! Did you attach both Transform and CameraComponent?");
                return Ok(());
            }
        };

        // Set up camera matrices. TODO: Determine projection via plugin!
        let camera_transf = engine.ecs().get::<Transform>(camera_entity);
        let camera_comp = engine.ecs().get::<CameraComponent>(camera_entity);
        let proj = Matrix4::new_perspective(
            self.screen_size.width as f32 / self.screen_size.height as f32,
            45_f32.to_radians(),
            0.01,
            1000.,
        );

        // Prepare data
        let entities = engine.ecs().query(&[
            QueryComponent::new::<Render>(Access::Read),
            QueryComponent::new::<Transform>(Access::Read),
        ]);

        // TODO: Don't allocate here smh
        let mut transforms = vec![];
        let mut handles = vec![];

        for entity in entities {
            transforms.push(engine.ecs().get::<Transform>(entity));
            handles.push(engine.ecs().get::<Render>(entity));
        }

        // Send frame timing info
        engine.send(FrameTime {
            delta: self.last_frame.elapsed().as_secs_f32(),
            time: self.start_time.elapsed().as_secs_f32(),
        });

        // Draw!
        self.rdr.frame(
            &self.gl,
            proj,
            camera_transf.view(),
            camera_comp.clear_color,
            &transforms,
            &handles,
        )?;

        // Reset timing
        self.last_frame = Instant::now();

        Ok(())
    }
}

// TODO: destructors! (lol)
/// Rendering engine state
struct RenderEngine {
    meshes: HashMap<RenderHandle, GpuMesh>,
    shader: gl::Program,
}

struct GpuMesh {
    vao: gl::VertexArray,
    vbo: gl::NativeBuffer,
    ebo: gl::NativeBuffer,
    index_count: i32,
}

impl RenderEngine {
    pub fn new(gl: &gl::Context) -> Result<Self> {
        unsafe {
            // Enable backface culling
            gl.enable(gl::CULL_FACE);

            // Enable depth buffering
            gl.enable(gl::DEPTH_TEST);
            gl.depth_func(gl::LESS);

            // Compile shaders
            let shader = compile_glsl_program(
                &gl,
                &[
                    (gl::VERTEX_SHADER, include_str!("shaders/unlit.vert")),
                    (gl::FRAGMENT_SHADER, include_str!("shaders/unlit.frag")),
                ],
            )?;

            Ok(Self {
                meshes: HashMap::new(),
                shader,
            })
        }
    }

    /// Make the given render data available to the GPU
    pub fn upload(&mut self, gl: &gl::Context, data: &RenderData) -> Result<()> {
        // TODO: Use a different mesh type? Switch for upload frequency? Hmmm..
        if let Some(buf) = self.meshes.get_mut(&data.id) {
            update_mesh(gl, buf, &data.mesh);
        } else {
            let gpu_mesh =
                upload_mesh(gl, gl::DYNAMIC_DRAW, &data.mesh).expect("Failed to upload mesh");
            self.meshes.insert(data.id, gpu_mesh);
        }

        Ok(())
    }

    /// The given heads will be rendered using the provided projection matrix and view Transform
    /// position
    pub fn frame(
        &mut self,
        gl: &gl::Context,
        proj: Matrix4<f32>,
        view: Matrix4<f32>,
        clear_color: [f32; 3],
        transforms: &[Transform],
        handles: &[Render],
    ) -> Result<()> {
        unsafe {
            // Clear depth and color buffers
            gl.disable(gl::BLEND);
            gl.disable(gl::SCISSOR_TEST);
            gl.disable(gl::STENCIL_TEST);
            gl.disable(gl::FRAMEBUFFER_SRGB);

            gl.enable(gl::CULL_FACE);
            gl.enable(glow::DEPTH_TEST);

            let [r, g, b] = clear_color;
            gl.clear_color(r, g, b, 1.0);
            gl.depth_func(glow::LESS);
            gl.depth_mask(true);
            gl.depth_range_f32(0., 1.);
            gl.clear_depth_f32(1.0);

            gl.clear(gl::COLOR_BUFFER_BIT | gl::STENCIL_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

            // Draw map
            // Must bind program before setting uniforms!!!
            gl.use_program(Some(self.shader));

            // Set camera matrix
            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(self.shader, "view").as_ref(),
                false,
                view.as_slice(),
            );

            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(self.shader, "proj").as_ref(),
                false,
                proj.as_slice(),
            );

            let transf_loc = gl.get_uniform_location(self.shader, "transf");

            // TODO: Literally ANY optimization
            for (transf, rdr_comp) in transforms.iter().zip(handles) {
                if let Some(mesh) = self.meshes.get(&rdr_comp.id) {
                    // Set transform
                    let matrix = transf.to_homogeneous();
                    gl.uniform_matrix_4_f32_slice(
                        transf_loc.as_ref(),
                        false,
                        bytemuck::cast_slice(matrix.as_ref()),
                    );

                    // Translate draw call
                    let primitive = match rdr_comp.primitive {
                        Primitive::Lines => gl::LINES,
                        Primitive::Points => gl::POINTS,
                        Primitive::Triangles => gl::TRIANGLES,
                    };

                    let limit: i32 = match rdr_comp.limit {
                        None => mesh.index_count,
                        Some(lim) => lim.try_into().unwrap(),
                    };

                    // Draw mesh data
                    if limit <= mesh.index_count {
                        gl.bind_vertex_array(Some(mesh.vao));
                        gl.draw_elements(primitive, limit, gl::UNSIGNED_INT, 0);
                    } else {
                        log::warn!(
                            "Invalid draw limit, got {} but mesh has {} indices",
                            limit,
                            mesh.index_count
                        );
                    }
                    //gl.bind_vertex_array(None);
                } else {
                    log::warn!(
                        "Warning: Attempted to access absent mesh data {:?}",
                        rdr_comp
                    );
                }
            }

            gl.use_program(None);

            Ok(())
        }
    }
}

/// Compiles (*_SHADER, <source>) into a shader program for OpenGL
fn compile_glsl_program(gl: &gl::Context, sources: &[(u32, &str)]) -> Result<gl::Program> {
    // Compile default shaders
    unsafe {
        let program = gl.create_program().expect("Cannot create program");

        let mut shaders = vec![];

        for (stage, shader_source) in sources {
            let shader = gl.create_shader(*stage).expect("Cannot create shader");

            gl.shader_source(shader, shader_source);

            gl.compile_shader(shader);

            if !gl.get_shader_compile_status(shader) {
                return Err(format_err!(
                    "OpenGL compile shader: {}",
                    gl.get_shader_info_log(shader)
                ));
            }

            gl.attach_shader(program, shader);

            shaders.push(shader);
        }

        gl.link_program(program);

        if !gl.get_program_link_status(program) {
            return Err(format_err!(
                "OpenGL link shader: {}",
                gl.get_program_info_log(program)
            ));
        }

        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }

        Ok(program)
    }
}

fn set_vertex_attrib(gl: &gl::Context) {
    unsafe {
        // Set vertex attributes
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(
            0,
            3,
            gl::FLOAT,
            false,
            std::mem::size_of::<Vertex>() as i32,
            0,
        );

        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(
            1,
            3,
            gl::FLOAT,
            false,
            std::mem::size_of::<Vertex>() as i32,
            3 * std::mem::size_of::<f32>() as i32,
        );
    }
}

/// Uploads a mesh; does not unbind vertex array
fn upload_mesh(gl: &gl::Context, usage: u32, mesh: &Mesh) -> Result<GpuMesh, String> {
    unsafe {
        // Map buffer
        let vao = gl.create_vertex_array()?;
        let vbo = gl.create_buffer()?;
        let ebo = gl.create_buffer()?;

        gl.bind_vertex_array(Some(vao));

        // Write vertices
        gl.bind_buffer(gl::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(
            gl::ARRAY_BUFFER,
            bytemuck::cast_slice(&mesh.vertices),
            usage,
        );

        // Write vertices
        gl.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, Some(ebo));
        gl.buffer_data_u8_slice(
            gl::ELEMENT_ARRAY_BUFFER,
            bytemuck::cast_slice(&mesh.indices),
            usage,
        );

        // Set vertex attributes
        set_vertex_attrib(gl);

        // Unbind vertex array
        gl.bind_vertex_array(None);

        Ok(GpuMesh {
            vao,
            vbo,
            ebo,
            index_count: mesh.indices.len() as i32,
        })
    }
}

fn update_mesh(gl: &gl::Context, buf: &mut GpuMesh, mesh: &Mesh) {
    unsafe {
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(buf.vbo));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            bytemuck::cast_slice(&mesh.vertices),
            glow::DYNAMIC_DRAW,
        );

        gl.bind_buffer(glow::ARRAY_BUFFER, Some(buf.ebo));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            bytemuck::cast_slice(&mesh.indices),
            glow::DYNAMIC_DRAW,
        );

        buf.index_count = mesh.indices.len() as i32;
    }
}
