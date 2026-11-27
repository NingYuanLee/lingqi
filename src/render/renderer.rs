//! 自写的 wgpu 渲染器。
//!
//! 与 egui-wgpu 的差别：只处理 epaint 产出的网格原语，不含 UI 框架
//! （输入、布局、字体管理都在别处），因此代码短、行为可预期，
//! 也便于逐像素对拍。
//!
//! 顶点布局与 egui-wgpu 保持一致（位置 / uv / 颜色，共 20 字节），
//! 混合方式也一样（预乘 alpha），因此同样的原语画出来应当完全一致。
//!
//! 着色器改写自 egui-wgpu 的 `egui.wgsl`（MIT / Apache-2.0，见 THIRD_PARTY_NOTICES）。

use std::borrow::Cow;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::ops::Range;

use epaint::textures::{TextureFilter, TextureOptions, TextureWrapMode};
use epaint::{ClippedPrimitive, ImageData, ImageDelta, TextureId};

const SHADER: &str = include_str!("shader.wgsl");

/// 渲染目标与像素比。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenDescriptor {
    /// 屏幕尺寸（物理像素）。
    pub size_in_pixels: [u32; 2],
    /// 每个逻辑点对应多少物理像素。
    pub pixels_per_point: f32,
}

impl ScreenDescriptor {
    fn screen_size_in_points(&self) -> [f32; 2] {
        [
            self.size_in_pixels[0] as f32 / self.pixels_per_point,
            self.size_in_pixels[1] as f32 / self.pixels_per_point,
        ]
    }
}

/// 统一缓冲区：屏幕尺寸（逻辑点）与抖动开关，共 16 字节。
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
struct UniformBuffer {
    screen_size_in_points: [f32; 2],
    dithering: u32,
    _padding: u32,
}

impl UniformBuffer {
    fn bytes(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&self.screen_size_in_points[0].to_le_bytes());
        out[4..8].copy_from_slice(&self.screen_size_in_points[1].to_le_bytes());
        out[8..12].copy_from_slice(&self.dithering.to_le_bytes());
        out
    }
}

/// 增长式缓冲：一次上传多个原语，按原语记录切片。
struct SlicedBuffer {
    buffer: wgpu::Buffer,
    slices: Vec<Range<usize>>,
    capacity: u64,
}

impl SlicedBuffer {
    fn new(device: &wgpu::Device, usage: wgpu::BufferUsages, label: &str) -> Self {
        let capacity = 1024 * 64;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: capacity,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { buffer, slices: Vec::new(), capacity }
    }

    /// 保证容量够用，不够就翻倍重建。
    fn ensure(&mut self, device: &wgpu::Device, required: u64, label: &str) {
        if self.capacity >= required {
            return;
        }
        self.capacity = (self.capacity * 2).max(required);
        self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: self.capacity,
            usage: self.buffer.usage(),
            mapped_at_creation: false,
        });
    }
}

/// 一张纹理及其绑定组。
struct Texture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    options: TextureOptions,
}

/// 自写渲染器：把 epaint 的渲染原语画到 wgpu 目标。
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    index_buffer: SlicedBuffer,
    vertex_buffer: SlicedBuffer,
    uniform_buffer: wgpu::Buffer,
    previous_uniform: Option<UniformBuffer>,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    textures: HashMap<TextureId, Texture>,
    samplers: HashMap<TextureOptions, wgpu::Sampler>,
    dithering: bool,
}

impl Renderer {
    /// 建立渲染器。`output_color_format` 建议用 `Rgba8Unorm` 或 `Bgra8Unorm`（伽马空间）。
    pub fn new(device: &wgpu::Device, output_color_format: wgpu::TextureFormat, dithering: bool) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lingqi-render"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lingqi-uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lingqi-uniform-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(16),
                    ty: wgpu::BufferBindingType::Uniform,
                },
                count: None,
            }],
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lingqi-uniform"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lingqi-texture-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lingqi-pipeline-layout"),
            bind_group_layouts: &[&uniform_layout, &texture_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lingqi-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 5 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    // 0: 位置；1: uv；2: 颜色
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                unclipped_depth: false,
                conservative: false,
                cull_mode: None,
                front_face: wgpu::FrontFace::default(),
                polygon_mode: wgpu::PolygonMode::default(),
                strip_index_format: None,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                alpha_to_coverage_enabled: false,
                count: 1,
                mask: !0,
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(if output_color_format.is_srgb() {
                    "fs_main_linear_framebuffer"
                } else {
                    "fs_main_gamma_framebuffer"
                }),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_color_format,
                    blend: Some(wgpu::BlendState {
                        // 预乘 alpha：颜色乘自身 alpha，alpha 用「一减目标 alpha」。
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            index_buffer: SlicedBuffer::new(device, wgpu::BufferUsages::INDEX, "lingqi-indices"),
            vertex_buffer: SlicedBuffer::new(device, wgpu::BufferUsages::VERTEX, "lingqi-vertices"),
            uniform_buffer,
            previous_uniform: None,
            uniform_bind_group,
            texture_bind_group_layout: texture_layout,
            textures: HashMap::new(),
            samplers: HashMap::new(),
            dithering,
        }
    }

    /// 上传这一帧要用的顶点、索引与统一缓冲区。绘制前调用一次。
    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        paint_jobs: &[ClippedPrimitive],
        screen: &ScreenDescriptor,
    ) {
        let uniform = UniformBuffer {
            screen_size_in_points: screen.screen_size_in_points(),
            dithering: u32::from(self.dithering),
            _padding: 0,
        };
        if self.previous_uniform != Some(uniform) {
            queue.write_buffer(&self.uniform_buffer, 0, &uniform.bytes());
            self.previous_uniform = Some(uniform);
        }

        // 顶点与索引各自拼成一大块上传，按原语记录切片。
        let mut vertices: Vec<u8> = Vec::new();
        let mut indices: Vec<u8> = Vec::new();
        self.index_buffer.slices.clear();
        self.vertex_buffer.slices.clear();

        let mut vertex_base: u32 = 0;
        for job in paint_jobs {
            let epaint::Primitive::Mesh(mesh) = &job.primitive else { continue };
            let index_start = indices.len();
            let vertex_start = vertices.len();
            for v in &mesh.vertices {
                vertices.extend_from_slice(&v.pos.x.to_le_bytes());
                vertices.extend_from_slice(&v.pos.y.to_le_bytes());
                vertices.extend_from_slice(&v.uv.x.to_le_bytes());
                vertices.extend_from_slice(&v.uv.y.to_le_bytes());
                vertices.extend_from_slice(&v.color.to_array());
            }
            for i in &mesh.indices {
                indices.extend_from_slice(&(i + vertex_base).to_le_bytes());
            }
            vertex_base += mesh.vertices.len() as u32;
            self.index_buffer.slices.push(index_start..indices.len());
            self.vertex_buffer.slices.push(vertex_start..vertices.len());
        }

        if !indices.is_empty() {
            self.index_buffer.ensure(device, indices.len() as u64, "lingqi-indices");
            self.vertex_buffer.ensure(device, vertices.len() as u64, "lingqi-vertices");
            queue.write_buffer(&self.index_buffer.buffer, 0, &indices);
            queue.write_buffer(&self.vertex_buffer.buffer, 0, &vertices);
        }
    }

    /// 画出这一帧的原语。
    pub fn render(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        paint_jobs: &[ClippedPrimitive],
        screen: &ScreenDescriptor,
    ) {
        let size = screen.size_in_pixels;
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_viewport(0.0, 0.0, size[0] as f32, size[1] as f32, 0.0, 1.0);

        let mut index_slices = self.index_buffer.slices.iter();
        let mut vertex_slices = self.vertex_buffer.slices.iter();

        for job in paint_jobs {
            let epaint::Primitive::Mesh(mesh) = &job.primitive else { continue };
            // 零尺寸的裁剪区要跳过，但缓冲切片仍要按顺序消费掉，否则后面的偏移全乱。
            let Some(rect) = ScissorRect::new(job.clip_rect, screen.pixels_per_point, size) else { continue };
            let Some(index_slice) = index_slices.next() else { continue };
            let Some(vertex_slice) = vertex_slices.next() else { continue };

            let Some(texture) = self.textures.get(&mesh.texture_id) else { continue };
            render_pass.set_bind_group(1, &texture.bind_group, &[]);
            render_pass.set_scissor_rect(rect.x, rect.y, rect.width, rect.height);
            render_pass.set_index_buffer(
                self.index_buffer.buffer.slice(index_slice.start as u64..index_slice.end as u64),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.set_vertex_buffer(0, self.vertex_buffer.buffer.slice(vertex_slice.start as u64..vertex_slice.end as u64));
            render_pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
        }
        render_pass.set_scissor_rect(0, 0, size[0], size[1]);
    }

    /// 上传或更新一张纹理（字形图集、用户贴图都走这里）。
    pub fn update_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: TextureId,
        delta: &ImageDelta,
    ) {
        let width = delta.image.width() as u32;
        let height = delta.image.height() as u32;
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };

        let pixels: Vec<epaint::Color32> = match &delta.image {
            ImageData::Color(image) => image.pixels.clone(),
            ImageData::Font(image) => image.srgba_pixels(None).collect(),
        };
        let bytes: Vec<u8> = pixels.iter().flat_map(|c| c.to_array()).collect();

        // pos 为 Some 表示只更新已有纹理的一块（图集增量）。
        let (texture, origin, reuse_bind_group) = match delta.pos {
            Some(pos) => {
                let existing = self.textures.remove(&id).expect("更新纹理前必须先建过");
                let origin = wgpu::Origin3d { x: pos[0] as u32, y: pos[1] as u32, z: 0 };
                let same = existing.options == delta.options;
                (existing.texture, origin, same.then_some(existing.bind_group))
            }
            None => {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("lingqi-tex"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                (texture, wgpu::Origin3d::ZERO, None)
            }
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let bind_group = match reuse_bind_group {
            Some(bg) => bg,
            None => {
                let sampler = self
                    .samplers
                    .entry(delta.options)
                    .or_insert_with(|| create_sampler(delta.options, device));
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("lingqi-tex-bind-group"),
                    layout: &self.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture.create_view(&wgpu::TextureViewDescriptor::default())),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                    ],
                })
            }
        };

        self.textures.insert(id, Texture { texture, bind_group, options: delta.options });
    }
}

/// 裁剪矩形：逻辑坐标 → 物理像素，并夹到画布内。
struct ScissorRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl ScissorRect {
    fn new(clip_rect: epaint::Rect, pixels_per_point: f32, size: [u32; 2]) -> Option<Self> {
        let min_x = (pixels_per_point * clip_rect.min.x).round() as u32;
        let max_x = (pixels_per_point * clip_rect.max.x).round() as u32;
        let min_y = (pixels_per_point * clip_rect.min.y).round() as u32;
        let max_y = (pixels_per_point * clip_rect.max.y).round() as u32;
        let min_x = min_x.min(size[0]);
        let min_y = min_y.min(size[1]);
        let max_x = max_x.clamp(min_x, size[0]);
        let max_y = max_y.clamp(min_y, size[1]);
        let width = max_x - min_x;
        let height = max_y - min_y;
        (width > 0 && height > 0).then_some(Self { x: min_x, y: min_y, width, height })
    }
}

/// 按纹理选项建采样器。
fn create_sampler(options: TextureOptions, device: &wgpu::Device) -> wgpu::Sampler {
    let clamp = options.wrap_mode == TextureWrapMode::ClampToEdge;
    let address_mode = if clamp {
        wgpu::AddressMode::ClampToEdge
    } else {
        wgpu::AddressMode::Repeat
    };
    let filter = if options.magnification == TextureFilter::Nearest {
        wgpu::FilterMode::Nearest
    } else {
        wgpu::FilterMode::Linear
    };
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: None,
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        address_mode_w: address_mode,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: filter,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        anisotropy_clamp: 1,
        compare: None,
        border_color: None,
    })
}
