use crate::{
    render_systems::{
        EguiPipelines, EguiTextureBindGroups, EguiTextureId, EguiTransform, EguiTransforms,
    },
    EguiRenderOutput, EguiSettings, WindowSize,
};
use bevy::{
    ecs::world::{FromWorld, World},
    prelude::{Entity, Handle, Resource},
    render::{
        render_asset::RenderAssetUsages,
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{
            BindGroupLayout, BindGroupLayoutEntry, BindingType, BlendComponent, BlendFactor,
            BlendOperation, BlendState, Buffer, BufferAddress, BufferBindingType, BufferDescriptor,
            BufferUsages, ColorTargetState, ColorWrites, Extent3d, FragmentState, FrontFace,
            IndexFormat, LoadOp, MultisampleState, Operations, PipelineCache, PrimitiveState,
            RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
            SamplerBindingType, Shader, ShaderStages, ShaderType, SpecializedRenderPipeline,
            StoreOp, TextureDimension, TextureFormat, TextureSampleType, TextureViewDimension,
            VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
        view::ExtractedWindows,
    },
};
use bytemuck::cast_slice;
use egui::{TextureFilter, TextureOptions};

/// Egui shader.
pub const EGUI_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(9898276442290979394);

/// Egui render pipeline.
#[derive(Resource)]
pub struct EguiPipeline {
    /// Transform bind group layout.
    pub transform_bind_group_layout: BindGroupLayout,
    /// Texture bind group layout.
    pub texture_bind_group_layout: BindGroupLayout,
}

impl FromWorld for EguiPipeline {
    fn from_world(render_world: &mut World) -> Self {
        let render_device = render_world.get_resource::<RenderDevice>().unwrap();

        let transform_bind_group_layout = render_device.create_bind_group_layout(
            "egui transform bind group layout",
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(EguiTransform::min_size()),
                },
                count: None,
            }],
        );

        let texture_bind_group_layout = render_device.create_bind_group_layout(
            "egui texture bind group layout",
            &[
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
        );

        EguiPipeline {
            transform_bind_group_layout,
            texture_bind_group_layout,
        }
    }
}

/// Key for specialized pipeline.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct EguiPipelineKey {
    /// Texture format of a window's swap chain to render to.
    pub texture_format: TextureFormat,
}

impl SpecializedRenderPipeline for EguiPipeline {
    type Key = EguiPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("egui render pipeline".into()),
            layout: vec![
                self.transform_bind_group_layout.clone(),
                self.texture_bind_group_layout.clone(),
            ],
            vertex: VertexState {
                shader: EGUI_SHADER_HANDLE,
                shader_defs: Vec::new(),
                entry_point: "vs_main".into(),
                buffers: vec![VertexBufferLayout::from_vertex_formats(
                    VertexStepMode::Vertex,
                    [
                        VertexFormat::Float32x2, // position
                        VertexFormat::Float32x2, // UV
                        VertexFormat::Unorm8x4,  // color (sRGB)
                    ],
                )],
            },
            fragment: Some(FragmentState {
                shader: EGUI_SHADER_HANDLE,
                shader_defs: Vec::new(),
                entry_point: "fs_main".into(),
                targets: vec![Some(ColorTargetState {
                    format: key.texture_format,
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
                })],
            }),
            primitive: PrimitiveState {
                front_face: FrontFace::Cw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            push_constant_ranges: vec![],
        }
    }
}

#[derive(Debug)]
struct DrawCommand {
    vertices_count: usize,
    egui_texture: EguiTextureId,
    clipping_zone: (u32, u32, u32, u32), // x, y, w, h
}

/// Egui render node.
pub struct EguiNode {
    window_entity: Entity,
    vertex_data: Vec<u8>,
    vertex_buffer_capacity: usize,
    vertex_buffer: Option<Buffer>,
    index_data: Vec<u8>,
    index_buffer_capacity: usize,
    index_buffer: Option<Buffer>,
    draw_commands: Vec<DrawCommand>,
}

impl EguiNode {
    /// Constructs Egui render node.
    pub fn new(window_entity: Entity) -> Self {
        EguiNode {
            window_entity,
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
        let mut window_sizes = world.query::<(&WindowSize, &mut EguiRenderOutput)>();

        let Ok((window_size, mut render_output)) = window_sizes.get_mut(world, self.window_entity)
        else {
            return;
        };
        let window_size = *window_size;
        let paint_jobs = std::mem::take(&mut render_output.paint_jobs);

        let egui_settings = &world.get_resource::<EguiSettings>().unwrap();

        let render_device = world.get_resource::<RenderDevice>().unwrap();

        let scale_factor = window_size.scale_factor * egui_settings.scale_factor;
        if window_size.physical_width == 0.0 || window_size.physical_height == 0.0 {
            return;
        }

        let mut index_offset = 0;

        self.draw_commands.clear();
        self.vertex_data.clear();
        self.index_data.clear();

        for egui::epaint::ClippedPrimitive {
            clip_rect,
            primitive,
        } in &paint_jobs
        {
            let mesh = match primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh,
                egui::epaint::Primitive::Callback(_) => {
                    unimplemented!("Paint callbacks aren't supported")
                }
            };

            let (x, y, w, h) = (
                (clip_rect.min.x * scale_factor).round() as u32,
                (clip_rect.min.y * scale_factor).round() as u32,
                (clip_rect.width() * scale_factor).round() as u32,
                (clip_rect.height() * scale_factor).round() as u32,
            );

            if w < 1
                || h < 1
                || x >= window_size.physical_width as u32
                || y >= window_size.physical_height as u32
            {
                continue;
            }

            self.vertex_data
                .extend_from_slice(cast_slice::<_, u8>(mesh.vertices.as_slice()));
            let indices_with_offset = mesh
                .indices
                .iter()
                .map(|i| i + index_offset)
                .collect::<Vec<_>>();
            self.index_data
                .extend_from_slice(cast_slice(indices_with_offset.as_slice()));
            index_offset += mesh.vertices.len() as u32;

            let texture_handle = match mesh.texture_id {
                egui::TextureId::Managed(id) => EguiTextureId::Managed(self.window_entity, id),
                egui::TextureId::User(id) => EguiTextureId::User(id),
            };

            let x_viewport_clamp = (x + w).saturating_sub(window_size.physical_width as u32);
            let y_viewport_clamp = (y + h).saturating_sub(window_size.physical_height as u32);
            self.draw_commands.push(DrawCommand {
                vertices_count: mesh.indices.len(),
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
                size: self.vertex_buffer_capacity as BufferAddress,
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
                size: self.index_buffer_capacity as BufferAddress,
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
        let egui_pipelines = &world.get_resource::<EguiPipelines>().unwrap().0;
        let pipeline_cache = world.get_resource::<PipelineCache>().unwrap();

        let extracted_windows = &world.get_resource::<ExtractedWindows>().unwrap().windows;
        let extracted_window =
            if let Some(extracted_window) = extracted_windows.get(&self.window_entity) {
                extracted_window
            } else {
                return Ok(()); // No window
            };

        let swap_chain_texture_view = if let Some(swap_chain_texture_view) =
            extracted_window.swap_chain_texture_view.as_ref()
        {
            swap_chain_texture_view
        } else {
            return Ok(()); // No swapchain texture
        };

        let render_queue = world.get_resource::<RenderQueue>().unwrap();

        let (vertex_buffer, index_buffer) = match (&self.vertex_buffer, &self.index_buffer) {
            (Some(vertex), Some(index)) => (vertex, index),
            _ => return Ok(()),
        };

        render_queue.write_buffer(vertex_buffer, 0, &self.vertex_data);
        render_queue.write_buffer(index_buffer, 0, &self.index_data);

        let bind_groups = &world.get_resource::<EguiTextureBindGroups>().unwrap();

        let egui_transforms = world.get_resource::<EguiTransforms>().unwrap();

        let mut render_pass =
            render_context
                .command_encoder()
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("egui render pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: swap_chain_texture_view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

        let Some(pipeline_id) = egui_pipelines.get(&extracted_window.entity) else {
            return Ok(());
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(*pipeline_id) else {
            return Ok(());
        };

        render_pass.set_pipeline(pipeline);
        render_pass.set_vertex_buffer(0, *self.vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(
            *self.index_buffer.as_ref().unwrap().slice(..),
            IndexFormat::Uint32,
        );

        let transform_buffer_offset = egui_transforms.offsets[&self.window_entity];
        let transform_buffer_bind_group = &egui_transforms.bind_group.as_ref().unwrap().1;
        render_pass.set_bind_group(0, transform_buffer_bind_group, &[transform_buffer_offset]);

        let mut vertex_offset: u32 = 0;
        for draw_command in &self.draw_commands {
            if draw_command.clipping_zone.0 < extracted_window.physical_width
                && draw_command.clipping_zone.1 < extracted_window.physical_height
            {
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
                    draw_command.clipping_zone.2.min(
                        extracted_window
                            .physical_width
                            .saturating_sub(draw_command.clipping_zone.0),
                    ),
                    draw_command.clipping_zone.3.min(
                        extracted_window
                            .physical_height
                            .saturating_sub(draw_command.clipping_zone.1),
                    ),
                );

                render_pass.draw_indexed(
                    vertex_offset..(vertex_offset + draw_command.vertices_count as u32),
                    0,
                    0..1,
                );
                vertex_offset += draw_command.vertices_count as u32;
            }
        }

        Ok(())
    }
}

pub(crate) fn as_color_image(image: egui::ImageData) -> egui::ColorImage {
    match image {
        egui::ImageData::Color(image) => (*image).clone(),
        egui::ImageData::Font(image) => alpha_image_as_color_image(&image),
    }
}

fn alpha_image_as_color_image(image: &egui::FontImage) -> egui::ColorImage {
    egui::ColorImage {
        size: image.size,
        pixels: image.srgba_pixels(None).collect(),
    }
}

pub(crate) fn color_image_as_bevy_image(
    egui_image: &egui::ColorImage,
    sampler_descriptor: ImageSampler,
) -> Image {
    let pixels = egui_image
        .pixels
        .iter()
        // We unmultiply Egui textures to premultiply them later in the fragment shader.
        // As user textures loaded as Bevy assets are not premultiplied (and there seems to be no
        // convenient way to convert them to premultiplied ones), we do the this with Egui ones.
        .flat_map(|color| color.to_srgba_unmultiplied())
        .collect();

    Image {
        sampler: sampler_descriptor,
        ..Image::new(
            Extent3d {
                width: egui_image.width() as u32,
                height: egui_image.height() as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
    }
}

pub(crate) fn texture_options_as_sampler_descriptor(
    options: &TextureOptions,
) -> ImageSamplerDescriptor {
    fn convert_filter(filter: &TextureFilter) -> ImageFilterMode {
        match filter {
            egui::TextureFilter::Nearest => ImageFilterMode::Nearest,
            egui::TextureFilter::Linear => ImageFilterMode::Linear,
        }
    }
    let address_mode = match options.wrap_mode {
        egui::TextureWrapMode::ClampToEdge => ImageAddressMode::ClampToEdge,
        egui::TextureWrapMode::Repeat => ImageAddressMode::Repeat,
        egui::TextureWrapMode::MirroredRepeat => ImageAddressMode::MirrorRepeat,
    };
    ImageSamplerDescriptor {
        mag_filter: convert_filter(&options.magnification),
        min_filter: convert_filter(&options.minification),
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        ..Default::default()
    }
}
