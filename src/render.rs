use std::collections::HashMap;

use glam::*;
use miniquad::*;

use crate::assets::{image::Image, mesh::Mesh, AssetId};
use crate::ui::Rect;

const MAX_VERTICES_PER_TEXTURE: usize = 0x10000;

#[derive(Debug, Clone)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_mat4(mat4: Mat4) -> Self {
        let (scale, rotation, translation) = mat4.to_scale_rotation_translation();
        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn lerp(&self, oth: &Transform, s: f32) -> Self {
        Self {
            translation: self.translation.lerp(oth.translation, s),
            scale: self.scale.lerp(oth.scale, s),
            rotation: self.rotation.slerp(oth.rotation, s),
        }
    }
}

pub fn atlas_to_uv(atlas_image: &Image, tile_size: Vec2, atlas_pos: Vec2) -> Rect {
    atlas_to_uv_pad_offset(atlas_image, tile_size, atlas_pos, Vec2::ZERO, Vec2::ZERO)
}

pub fn atlas_to_uv_pad_offset(
    atlas_image: &Image,
    tile_size: Vec2,
    atlas_pos: Vec2,
    pad: Vec2,
    offset: Vec2,
) -> Rect {
    let image_size = Vec2::new(atlas_image.width as f32, atlas_image.height as f32);
    let pos = (offset + atlas_pos * (tile_size + pad)) / image_size;
    let size = tile_size / image_size;
    Rect::new(pos, size)
}

#[derive(Debug, Clone)]
pub struct RenderMesh {
    // defines a set of triangles and their transform
    pub mesh_id: AssetId,
    pub transform: Transform,
    // uniform or directly in vertex data
    pub color: Vec4,
    // texture
    pub image_id: Option<AssetId>,
}

#[derive(Debug, Clone)]
pub struct RenderTile {
    // defines two triangles and their position
    pub world_rect: Rect,
    pub clip_rect: Rect,
    pub z: f32,
    // uniform or directly in vertex data
    pub color: Vec4,
}

impl Default for RenderTile {
    fn default() -> Self {
        Self {
            world_rect: Rect::default(),
            clip_rect: Rect::new(Vec2::ZERO, Vec2::ONE),
            color: Vec4::ONE,
            z: 0.,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderTileCommands {
    textured: HashMap<AssetId, Vec<RenderTile>>,
    solid: Vec<RenderTile>,
}

impl RenderTileCommands {
    pub fn draw(&mut self, tile: RenderTile) {
        self.solid.push(tile);
    }
    pub fn draw_textured(&mut self, tile: RenderTile, texture: AssetId) {
        self.textured.entry(texture).or_default().push(tile);
    }
    pub fn clear(&mut self) {
        self.textured.clear();
        self.solid.clear();
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderMeshCommands {
    meshes: HashMap<AssetId, Vec<RenderMesh>>,
}

impl RenderMeshCommands {
    pub fn draw(&mut self, mesh: RenderMesh) {
        self.meshes
            .entry(mesh.mesh_id.clone())
            .or_default()
            .push(mesh);
    }
    pub fn clear(&mut self) {
        self.meshes.clear();
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MeshBuffers {
    vertex_buffer: BufferId,
    index_buffer: BufferId,
    indices_len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Vertex2d {
    pos: Vec3,
    uv: Vec2,
    color: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Vertex3d {
    pos: Vec3,
    uv: Vec2,
}

pub struct Camera {
    pub mode: CameraMode,
    pub view: Mat4,
}

impl Camera {
    pub fn ui() -> Self {
        Self {
            mode: CameraMode::Ui {
                near: -1000.,
                far: 1000.,
            },
            view: Mat4::IDENTITY,
        }
    }

    pub fn orthographic(scale: f32) -> Self {
        Self {
            mode: CameraMode::Orthographic {
                scale,
                near: 0.01,
                far: 100.,
            },
            view: Mat4::look_at_rh(
                vec3(0.0, 0.0, 10.0),
                vec3(0.0, 0.0, 0.0),
                vec3(0.0, 1.0, 0.0),
            ),
        }
    }

    pub fn perspective(fov: f32) -> Self {
        Self {
            mode: CameraMode::Perspective {
                fov,
                near: 0.01,
                far: 100.,
            },
            view: Mat4::look_at_rh(
                vec3(0.0, 0.0, 10.0),
                vec3(0.0, 0.0, 0.0),
                vec3(0.0, 1.0, 0.0),
            ),
        }
    }

    pub fn view_projection(&self, resolution: Vec2) -> Mat4 {
        self.mode.projection(resolution) * self.view
    }

    pub fn ray_from_cursor(&self, cursor: &Vec2, resolution: &Vec2) -> (Vec3, Vec3) {
        // PERF: slow calls to .inverse()
        let projection_view = self.view_projection(*resolution).inverse();
        let uv = *cursor / *resolution;
        let clip_space = Vec2::new(1.0, -1.0) * (uv * 2.0 - 1.0);
        let clip_far = projection_view * Vec4::new(clip_space.x, clip_space.y, 1.0, 1.0);
        let clip_near = projection_view * Vec4::new(clip_space.x, clip_space.y, 0.1, 1.0);
        let world_far = clip_far.xyz() / clip_far.w;
        let world_near = clip_near.xyz() / clip_near.w;
        let ray_direction = Vec3::normalize(world_far - world_near);
        let ray_origin = self.view.inverse().to_scale_rotation_translation().2;
        (ray_origin, ray_direction)
    }
}

pub enum CameraMode {
    Orthographic { scale: f32, near: f32, far: f32 },
    Perspective { fov: f32, near: f32, far: f32 },
    Ui { near: f32, far: f32 },
}

impl CameraMode {
    pub fn projection(&self, resolution: Vec2) -> Mat4 {
        match self {
            CameraMode::Perspective { fov, near, far } => {
                Mat4::perspective_rh_gl(*fov, resolution.x / resolution.y, *near, *far)
            }
            CameraMode::Orthographic { scale, near, far } => Mat4::orthographic_rh_gl(
                -resolution.x / resolution.y * scale,
                resolution.x / resolution.y * scale,
                -*scale,
                *scale,
                *near,
                *far,
            ),
            CameraMode::Ui { near, far } => {
                Mat4::orthographic_rh_gl(0., resolution.x, resolution.y, 0., *near, *far)
            }
        }
    }
}

// 2d and 3d OpenGLES2 immediate renderer
pub struct Renderer {
    pipeline_2d: Pipeline,
    pipeline_3d: Pipeline,
    bindings: Bindings,
    textures: HashMap<AssetId, TextureId>,
    texture_white_pixel: TextureId,
    mesh_buffers: HashMap<AssetId, MeshBuffers>,
    pub camera: Camera,
}

impl Renderer {
    pub fn new(ctx: &mut Box<dyn RenderingBackend>, camera: Camera) -> Self {
        let vertices = vec![Vertex2d::default(); 4 * MAX_VERTICES_PER_TEXTURE];
        let vertex_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::slice(&vertices),
        );

        let indices = [0_u16; 6 * MAX_VERTICES_PER_TEXTURE];
        let index_buffer = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Stream,
            BufferSource::slice(&indices),
        );

        let texture_white_pixel = ctx.new_texture_from_rgba8(1, 1, &[255, 255, 255, 255]);
        ctx.texture_set_filter(
            texture_white_pixel,
            FilterMode::Nearest,
            MipmapFilterMode::None,
        );

        let bindings = Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer,
            images: vec![texture_white_pixel],
        };

        let shader_2d = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: shader_2d::VERTEX,
                    fragment: shader_2d::FRAGMENT,
                },
                shader_2d::meta(),
            )
            .unwrap();

        let pipeline_2d = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("vertex_pos", VertexFormat::Float3),
                VertexAttribute::new("vertex_uv", VertexFormat::Float2),
                VertexAttribute::new("vertex_color", VertexFormat::Float4),
            ],
            shader_2d,
            PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Zero,
                    BlendFactor::One,
                )),
                depth_test: Comparison::LessOrEqual,
                depth_write: true,
                ..Default::default()
            },
        );

        let shader_3d = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: shader_3d::VERTEX,
                    fragment: shader_3d::FRAGMENT,
                },
                shader_3d::meta(),
            )
            .unwrap();

        let pipeline_3d = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("vertex_pos", VertexFormat::Float3),
                VertexAttribute::new("vertex_uv", VertexFormat::Float2),
            ],
            shader_3d,
            PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Zero,
                    BlendFactor::One,
                )),
                depth_test: Comparison::LessOrEqual,
                depth_write: true,
                ..Default::default()
            },
        );

        Self {
            pipeline_2d,
            pipeline_3d,
            bindings,
            textures: HashMap::new(),
            texture_white_pixel,
            mesh_buffers: HashMap::new(),
            camera,
        }
    }

    pub fn check_load_texture(
        &mut self,
        ctx: &mut Box<dyn RenderingBackend>,
        image: &Image,
        id: &AssetId,
        filter_mode: FilterMode,
    ) {
        if !self.textures.contains_key(id) {
            let texture =
                ctx.new_texture_from_rgba8(image.width as u16, image.height as u16, &image.raw);
            ctx.texture_set_filter(texture, filter_mode, MipmapFilterMode::None);
            self.textures.insert(id.clone(), texture);
        }
    }

    pub fn check_load_mesh(
        &mut self,
        ctx: &mut Box<dyn RenderingBackend>,
        mesh: &Mesh,
        id: &AssetId,
    ) {
        if !self.mesh_buffers.contains_key(id) {
            let mut vertices: Vec<Vertex3d> = vec![];
            for i in 0..mesh.vertices.len() {
                vertices.push(Vertex3d {
                    pos: mesh.vertices[i],
                    uv: *mesh.uvs.get(i).unwrap_or(&Vec2::ZERO),
                });
            }
            let vertex_buffer = ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(&vertices),
            );
            let index_buffer = ctx.new_buffer(
                BufferType::IndexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(&mesh.indices),
            );
            self.mesh_buffers.insert(
                id.clone(),
                MeshBuffers {
                    vertex_buffer,
                    index_buffer,
                    indices_len: mesh.indices.len(),
                },
            );
        }
    }

    pub fn draw(
        &self,
        ctx: &mut Box<dyn RenderingBackend>,
        tiles: &RenderTileCommands,
        meshes: &RenderMeshCommands,
        resolution: Vec2,
    ) {
        self.pass_3d(ctx, meshes, resolution);
        self.pass_2d(ctx, tiles, resolution);
    }

    pub fn pass_2d(
        &self,
        ctx: &mut Box<dyn RenderingBackend>,
        tile_commands: &RenderTileCommands,
        resolution: Vec2,
    ) {
        ctx.apply_pipeline(&self.pipeline_2d);

        if !tile_commands.solid.is_empty() {
            let mut tile_buffer = TileBuffer::new(resolution, self.texture_white_pixel);
            tile_buffer.tiles_to_triangles(&tile_commands.solid);
            tile_buffer.render(ctx, &self);
        }

        for (asset_id, tiles) in tile_commands.textured.iter() {
            let Some(texture_id) = self.textures.get(&asset_id) else {
                eprintln!("No texture for asset_id: {:?}", asset_id);
                return;
            };
            let mut tile_buffer = TileBuffer::new(resolution, *texture_id);
            tile_buffer.tiles_to_triangles(&tiles);
            tile_buffer.render(ctx, &self);
        }
    }

    pub fn pass_3d(
        &self,
        ctx: &mut Box<dyn RenderingBackend>,
        mesh_commands: &RenderMeshCommands,
        resolution: Vec2,
    ) {
        ctx.apply_pipeline(&self.pipeline_3d);

        let view_proj = self.camera.view_projection(resolution);

        for (mesh_id, meshes) in mesh_commands.meshes.iter() {
            for render_mesh in meshes.iter() {
                let Some(texture_id) = (match render_mesh.image_id {
                    Some(ref image_id) => self.textures.get(&image_id).copied(),
                    None => Some(self.texture_white_pixel),
                }) else {
                    eprintln!("No texture for mesh_id: {:?}", mesh_id);
                    return;
                };

                let Some(mesh) = self.mesh_buffers.get(mesh_id) else {
                    eprintln!("No mesh buffers for mesh_id: {:?}", mesh_id);
                    return;
                };

                ctx.apply_bindings(&Bindings {
                    vertex_buffers: vec![mesh.vertex_buffer],
                    index_buffer: mesh.index_buffer,
                    images: vec![texture_id],
                });

                let transform = Mat4::from_scale_rotation_translation(
                    render_mesh.transform.scale,
                    render_mesh.transform.rotation,
                    render_mesh.transform.translation,
                );
                let mvp = view_proj * transform;

                ctx.apply_uniforms(UniformsSource::table(&shader_3d::Uniforms {
                    world_transform: mvp,
                    color: render_mesh.color,
                }));
                ctx.draw(0, mesh.indices_len as i32, 1);
            }
        }
    }
}

struct TileBuffer {
    vertices: Vec<Vertex2d>,
    indices: Vec<u16>,
    written: u32,
    resolution: Vec2,
    texture_id: TextureId,
}

impl TileBuffer {
    fn new(resolution: Vec2, texture_id: TextureId) -> Self {
        Self {
            vertices: vec![],
            indices: vec![],
            written: 0,
            resolution,
            texture_id,
        }
    }

    fn tiles_to_triangles(&mut self, tiles: &Vec<RenderTile>) {
        for tile in tiles {
            let mut vs = [
                Vertex2d {
                    pos: Vec3::new(0., 0., 0.),
                    uv: Vec2::new(0., 0.),
                    color: tile.color,
                },
                Vertex2d {
                    pos: Vec3::new(1., 0., 0.),
                    uv: Vec2::new(1., 0.),
                    color: tile.color,
                },
                Vertex2d {
                    pos: Vec3::new(1., 1., 0.),
                    uv: Vec2::new(1., 1.),
                    color: tile.color,
                },
                Vertex2d {
                    pos: Vec3::new(0., 1., 0.),
                    uv: Vec2::new(0., 1.),
                    color: tile.color,
                },
            ];
            for v in &mut vs {
                let vertex_pos =
                    Vec2::new(v.pos.x, v.pos.y) * tile.world_rect.size + tile.world_rect.pos;
                let vertex_pos = (vertex_pos / self.resolution - Vec2::new(0.5, 0.5))
                    * Vec2::new(2.0, 2.0)
                    * Vec2::new(1.0, -1.0);
                v.pos = Vec3::new(vertex_pos.x, vertex_pos.y, tile.z);
                v.uv = tile.clip_rect.pos + v.uv * tile.clip_rect.size;
            }
            self.vertices.extend(vs);
            self.indices
                .extend([0, 1, 2, 0, 2, 3].map(|index| index + self.written as u16 * 4));
            self.written += 1;
        }
    }

    fn render(&self, ctx: &mut Box<dyn RenderingBackend>, renderer: &Renderer) {
        // TODO: divide the buffer into multiple 64k buffers instead of panicking
        ctx.buffer_update(
            renderer.bindings.vertex_buffers[0],
            BufferSource::slice(&self.vertices),
        );
        ctx.buffer_update(
            renderer.bindings.index_buffer,
            BufferSource::slice(&self.indices),
        );
        ctx.apply_bindings(&Bindings {
            images: vec![self.texture_id],
            ..renderer.bindings.clone()
        });
        ctx.draw(0, self.indices.len() as i32, 1);
    }
}

mod shader_2d {
    use miniquad::*;

    pub const VERTEX: &str = r#"#version 100
    attribute vec3 vertex_pos;
    attribute vec2 vertex_uv;
    attribute vec4 vertex_color;
    varying lowp vec2 texcoord;
    varying lowp vec4 color;
    void main() {
        gl_Position = vec4(vertex_pos, 1);
        texcoord = vertex_uv;
        color = vertex_color;
    }"#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec2 texcoord;
    varying lowp vec4 color;
    uniform sampler2D tex;
    void main() {
        gl_FragColor = texture2D(tex, texcoord) * color;
        if (gl_FragColor.a <= 0.1) { discard; }
    }"#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout { uniforms: vec![] },
        }
    }
}

mod shader_3d {
    use miniquad::*;

    pub const VERTEX: &str = r#"#version 100
    attribute vec3 vertex_pos;
    attribute vec2 vertex_uv;
    uniform mat4 world_transform;
    uniform vec4 color;
    varying lowp vec4 forward_color;
    varying lowp vec2 texcoord;
    void main() {
        vec4 pos = vec4(vertex_pos, 1);
        gl_Position = world_transform * pos;
        forward_color = color;
        texcoord = vertex_uv;
    }"#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec4 forward_color;
    varying lowp vec2 texcoord;
    uniform sampler2D tex;
    void main() {
        gl_FragColor = texture2D(tex, texcoord) * forward_color;
        if (gl_FragColor.a <= 0.1) { discard; }
    }"#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout {
                uniforms: vec![
                    UniformDesc::new("world_transform", UniformType::Mat4),
                    UniformDesc::new("color", UniformType::Float4),
                ],
            },
        }
    }

    #[repr(C)]
    pub struct Uniforms {
        pub world_transform: glam::Mat4,
        pub color: glam::Vec4,
    }
}
