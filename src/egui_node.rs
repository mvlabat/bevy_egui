use bevy::{
    core::{bytes_of, cast_slice},
    prelude::{FromWorld, World},
    render::{
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{
            BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
            BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType,
            BlendComponent, BlendFactor, BlendOperation, BlendState, Buffer, BufferSize,
            BufferUsages, ColorTargetState, ColorWrites, Extent3d, FrontFace, IndexFormat, LoadOp,
            MultisampleState, Operations, PipelineLayoutDescriptor, PrimitiveState,
            RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, ShaderStages,
            TextureDimension, TextureFormat, TextureSampleType, TextureViewDimension,
            VertexAttribute, VertexFormat, VertexStepMode,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::{BevyDefault, Image},
        view::ExtractedWindows,
    },
    window::WindowId,
};
use wgpu::{
    BufferBinding, BufferDescriptor, SamplerBindingType, ShaderModuleDescriptor, ShaderSource,
};

use crate::render_systems::{
    EguiTexture, EguiTextureBindGroups, EguiTransform, ExtractedEguiContext, ExtractedEguiSettings,
    ExtractedShapes, ExtractedWindowSizes,
};

pub struct EguiPipeline {
    pipeline: RenderPipeline,

    transform_buffer: Buffer,
    transform_bind_group: BindGroup,

    pub texture_bind_group_layout: BindGroupLayout,
}

impl FromWorld for EguiPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();

        let shader_source = ShaderSource::Wgsl(include_str!("egui.wgsl").into());
        let shader_module = render_device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("egui shader"),
            source: shader_source,
        });

        let transform_buffer_size =
            BufferSize::new(std::mem::size_of::<EguiTransform>() as u64).unwrap();
        let transform_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("egui transform buffer"),
            size: transform_buffer_size.get(),
            usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let transform_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("egui transform bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(transform_buffer_size),
                    },
                    count: None,
                }],
            });
        let transform_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("egui transform bind gruop"),
            layout: &transform_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &transform_buffer,
                    offset: 0,
                    size: Some(transform_buffer_size),
                }),
            }],
        });

        let texture_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("egui texture bind group layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("egui pipeline layout"),
            bind_group_layouts: &[&transform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline =
            render_device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("egui render pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: "vs_main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 32,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &[
                            VertexAttribute {
                                format: VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            },
                            VertexAttribute {
                                format: VertexFormat::Float32x2,
                                offset: 8,
                                shader_location: 1,
                            },
                            VertexAttribute {
                                format: VertexFormat::Float32x4,
                                offset: 16,
                                shader_location: 2,
                            },
                        ],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: "fs_main",
                    targets: &[ColorTargetState {
                        format: TextureFormat::bevy_default(),
                        blend: Some(BlendState {
                            color: BlendComponent {
                                src_factor: BlendFactor::One,
                                dst_factor: BlendFactor::OneMinusSrcAlpha,
                                operation: BlendOperation::Add,
                            },
                            alpha: BlendComponent {
                                src_factor: BlendFactor::One,
                                dst_factor: BlendFactor::OneMinusSrcAlpha,
                                operation: BlendOperation::Add,
                            },
                        }),
                        write_mask: ColorWrites::ALL,
                    }],
                }),
                primitive: PrimitiveState {
                    front_face: FrontFace::Cw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: MultisampleState::default(),
                multiview: None,
            });

        EguiPipeline {
            pipeline: render_pipeline,
            transform_buffer,
            transform_bind_group,
            texture_bind_group_layout,
        }
    }
}

#[derive(Debug)]
struct DrawCommand {
    vertices_count: usize,
    egui_texture: EguiTexture,
    clipping_zone: (u32, u32, u32, u32), // x, y, w, h
}

pub struct EguiNode {
    window_id: WindowId,
    vertex_data: Vec<u8>,
    vertex_buffer_capacity: usize,
    vertex_buffer: Option<Buffer>,
    index_data: Vec<u8>,
    index_buffer_capacity: usize,
    index_buffer: Option<Buffer>,
    draw_commands: Vec<DrawCommand>,
}

impl EguiNode {
    pub fn new(window_id: WindowId) -> Self {
        EguiNode {
            window_id,
            draw_commands: Vec::new(),
            vertex_data: Vec::new(),
            vertex_buffer_capacity: 0,
            vertex_buffer: None,
            index_data: Vec::new(),
            index_buffer_capacity: 0,
            index_buffer: None,
        }
    }
}

impl Node for EguiNode {
    fn update(&mut self, world: &mut World) {
        let mut shapes = world.get_resource_mut::<ExtractedShapes>().unwrap();
        let shapes = shapes.0.get_mut(&self.window_id).unwrap();
        let shapes = std::mem::take(&mut shapes.shapes);

        let window_size =
            &world.get_resource::<ExtractedWindowSizes>().unwrap().0[&self.window_id].0;
        let egui_settings = &world.get_resource::<ExtractedEguiSettings>().unwrap().0;
        let egui_context = &world.get_resource::<ExtractedEguiContext>().unwrap().0;

        let render_device = world.get_resource::<RenderDevice>().unwrap();

        let scale_factor = window_size.scale_factor * egui_settings.scale_factor as f32;
        if window_size.physical_width == 0.0 || window_size.physical_height == 0.0 {
            return;
        }

        let egui_paint_jobs = egui_context[&self.window_id].tessellate(shapes);

        let mut index_offset = 0;

        self.draw_commands.clear();
        self.vertex_data.clear();
        self.index_data.clear();

        for egui::ClippedMesh(rect, triangles) in &egui_paint_jobs {
            let (x, y, w, h) = (
                (rect.min.x * scale_factor).round() as u32,
                (rect.min.y * scale_factor).round() as u32,
                (rect.width() * scale_factor).round() as u32,
                (rect.height() * scale_factor).round() as u32,
            );

            if w < 1
                || h < 1
                || x >= window_size.physical_width as u32
                || y >= window_size.physical_height as u32
            {
                continue;
            }

            for vertex in &triangles.vertices {
                self.vertex_data
                    .extend_from_slice(bytes_of(&[vertex.pos.x, vertex.pos.y]));
                self.vertex_data
                    .extend_from_slice(bytes_of(&[vertex.uv.x, vertex.uv.y]));
                self.vertex_data
                    .extend_from_slice(bytes_of(&vertex.color.to_array().map(|c| c as f32)));
            }
            let indices_with_offset = triangles
                .indices
                .iter()
                .map(|i| i + index_offset)
                .collect::<Vec<_>>();
            self.index_data
                .extend_from_slice(cast_slice(indices_with_offset.as_slice()));
            index_offset += triangles.vertices.len() as u32;

            let texture_handle = match triangles.texture_id {
                egui::TextureId::Egui => EguiTexture::Font(self.window_id),
                egui::TextureId::User(id) => EguiTexture::User(id),
            };

            let x_viewport_clamp = (x + w).saturating_sub(window_size.physical_width as u32);
            let y_viewport_clamp = (y + h).saturating_sub(window_size.physical_height as u32);
            self.draw_commands.push(DrawCommand {
                vertices_count: triangles.indices.len(),
                egui_texture: texture_handle,
                clipping_zone: (
                    x,
                    y,
                    w.saturating_sub(x_viewport_clamp).max(1),
                    h.saturating_sub(y_viewport_clamp).max(1),
                ),
            });
        }

        if self.vertex_data.len() > self.vertex_buffer_capacity {
            self.vertex_buffer_capacity = if self.vertex_data.len().is_power_of_two() {
                self.vertex_data.len()
            } else {
                self.vertex_data.len().next_power_of_two()
            };
            self.vertex_buffer = Some(render_device.create_buffer(&BufferDescriptor {
                label: Some("egui vertex buffer"),
                size: self.vertex_buffer_capacity as wgpu::BufferAddress,
                usage: BufferUsages::COPY_DST | BufferUsages::VERTEX,
                mapped_at_creation: false,
            }));
        }
        if self.index_data.len() > self.index_buffer_capacity {
            self.index_buffer_capacity = if self.index_data.len().is_power_of_two() {
                self.index_data.len()
            } else {
                self.index_data.len().next_power_of_two()
            };
            self.index_buffer = Some(render_device.create_buffer(&BufferDescriptor {
                label: Some("egui index buffer"),
                size: self.index_buffer_capacity as wgpu::BufferAddress,
                usage: BufferUsages::COPY_DST | BufferUsages::INDEX,
                mapped_at_creation: false,
            }));
        }
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let egui_shaders = world.get_resource::<EguiPipeline>().unwrap();
        let render_queue = world.get_resource::<RenderQueue>().unwrap();

        render_queue.write_buffer(self.vertex_buffer.as_ref().unwrap(), 0, &self.vertex_data);
        render_queue.write_buffer(self.index_buffer.as_ref().unwrap(), 0, &self.index_data);

        let egui_transform =
            world.get_resource::<ExtractedWindowSizes>().unwrap().0[&self.window_id].1;

        let bind_groups = &world
            .get_resource::<EguiTextureBindGroups>()
            .unwrap()
            .bind_groups;

        render_queue.write_buffer(
            &egui_shaders.transform_buffer,
            0,
            bytemuck::bytes_of(&egui_transform),
        );

        let extracted_window =
            &world.get_resource::<ExtractedWindows>().unwrap().windows[&self.window_id];
        let swap_chain_texture = extracted_window
            .swap_chain_texture
            .as_ref()
            .unwrap()
            .clone();

        let mut render_pass =
            render_context
                .command_encoder
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("egui render pass"),
                    color_attachments: &[RenderPassColorAttachment {
                        view: &swap_chain_texture,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: true,
                        },
                    }],
                    depth_stencil_attachment: None,
                });

        render_pass.set_pipeline(&egui_shaders.pipeline);
        render_pass.set_vertex_buffer(0, *self.vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(
            *self.index_buffer.as_ref().unwrap().slice(..),
            IndexFormat::Uint32,
        );

        render_pass.set_bind_group(0, &egui_shaders.transform_bind_group, &[]);

        let mut vertex_offset: u32 = 0;
        for draw_command in &self.draw_commands {
            let texture_bind_group = match bind_groups.get(&draw_command.egui_texture) {
                Some(texture_resource) => texture_resource,
                None => {
                    vertex_offset += draw_command.vertices_count as u32;
                    continue;
                }
            };

            render_pass.set_bind_group(1, texture_bind_group, &[]);

            render_pass.set_scissor_rect(
                draw_command.clipping_zone.0,
                draw_command.clipping_zone.1,
                draw_command.clipping_zone.2,
                draw_command.clipping_zone.3,
            );
            render_pass.draw_indexed(
                vertex_offset..(vertex_offset + draw_command.vertices_count as u32),
                0,
                0..1,
            );
            vertex_offset += draw_command.vertices_count as u32;
        }

        Ok(())
    }
}

pub fn as_wgpu_image(egui_texture: &egui::Texture) -> Image {
    let mut pixels = Vec::new();
    pixels.reserve(4 * pixels.len());
    for &alpha in egui_texture.pixels.iter() {
        pixels.extend(
            egui::color::Color32::from_white_alpha(alpha)
                .to_array()
                .iter(),
        );
    }

    Image::new(
        Extent3d {
            width: egui_texture.width as u32,
            height: egui_texture.height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
    )
}
