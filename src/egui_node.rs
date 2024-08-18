use crate::{
    render_systems::{
        EguiPipelines, EguiTextureBindGroups, EguiTextureId, EguiTransform, EguiTransforms,
    },
    EguiRenderOutput, EguiSettings, RenderTargetSize,
};
use bevy::{
    ecs::world::{FromWorld, World},
    prelude::{Entity, Handle, Resource},
    render::{
        render_asset::RenderAssetUsages,
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_phase::TrackedRenderPass,
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
        texture::{
            GpuImage, Image, ImageAddressMode, ImageFilterMode, ImageSampler,
            ImageSamplerDescriptor,
        },
        view::{ExtractedWindow, ExtractedWindows},
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

impl EguiPipelineKey {
    /// Constructs a pipeline key from a window.
    pub fn from_extracted_window(window: &ExtractedWindow) -> Option<Self> {
        Some(Self {
            texture_format: window.swap_chain_texture_format?.add_srgb_suffix(),
        })
    }

    /// Constructs a pipeline key from a gpu image.
    pub fn from_gpu_image(image: &GpuImage) -> Self {
        EguiPipelineKey {
            texture_format: image.texture_format.add_srgb_suffix(),
        }
    }
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

pub(crate) struct DrawCommand {
    pub(crate) clip_rect: egui::Rect,
    pub(crate) primitive: DrawPrimitive,
}

pub(crate) enum DrawPrimitive {
    Egui(EguiDraw),
    PaintCallback(PaintCallbackDraw),
}

pub(crate) struct PaintCallbackDraw {
    pub(crate) callback: std::sync::Arc<EguiBevyPaintCallback>,
    pub(crate) rect: egui::Rect,
}

pub(crate) struct EguiDraw {
    pub(crate) vertices_count: usize,
    pub(crate) egui_texture: EguiTextureId,
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
    postponed_updates: Vec<(egui::Rect, PaintCallbackDraw)>,
    pixels_per_point: f32,
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
            postponed_updates: Vec::new(),
            pixels_per_point: 1.,
        }
    }
}

impl Node for EguiNode {
    fn update(&mut self, world: &mut World) {
        let Some(key) = world
            .get_resource::<ExtractedWindows>()
            .and_then(|windows| windows.windows.get(&self.window_entity))
            .and_then(EguiPipelineKey::from_extracted_window)
        else {
            return;
        };

        let mut render_target_size = world.query::<(&RenderTargetSize, &mut EguiRenderOutput)>();

        let Ok((window_size, mut render_output)) =
            render_target_size.get_mut(world, self.window_entity)
        else {
            return;
        };
        let window_size = *window_size;
        let paint_jobs = std::mem::take(&mut render_output.paint_jobs);

        let egui_settings = &world.get_resource::<EguiSettings>().unwrap();

        let render_device = world.get_resource::<RenderDevice>().unwrap();

        self.pixels_per_point = window_size.scale_factor * egui_settings.scale_factor;
        if window_size.physical_width == 0.0 || window_size.physical_height == 0.0 {
            return;
        }

        let mut index_offset = 0;

        self.draw_commands.clear();
        self.vertex_data.clear();
        self.index_data.clear();
        self.postponed_updates.clear();

        for egui::epaint::ClippedPrimitive {
            clip_rect,
            primitive,
        } in paint_jobs
        {
            let clip_urect = bevy::math::URect {
                min: bevy::math::UVec2 {
                    x: (clip_rect.min.x * self.pixels_per_point).round() as u32,
                    y: (clip_rect.min.y * self.pixels_per_point).round() as u32,
                },
                max: bevy::math::UVec2 {
                    x: (clip_rect.max.x * self.pixels_per_point).round() as u32,
                    y: (clip_rect.max.y * self.pixels_per_point).round() as u32,
                },
            };

            if clip_urect
                .intersect(bevy::math::URect::new(
                    0,
                    0,
                    window_size.physical_width as u32,
                    window_size.physical_height as u32,
                ))
                .is_empty()
            {
                continue;
            }

            let mesh = match primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh,
                egui::epaint::Primitive::Callback(paint_callback) => {
                    let Ok(callback) = paint_callback.callback.downcast::<EguiBevyPaintCallback>()
                    else {
                        unimplemented!("Unsupported egui paint callback type");
                    };

                    self.postponed_updates.push((
                        clip_rect,
                        PaintCallbackDraw {
                            callback: callback.clone(),
                            rect: paint_callback.rect,
                        },
                    ));

                    self.draw_commands.push(DrawCommand {
                        primitive: DrawPrimitive::PaintCallback(PaintCallbackDraw {
                            callback,
                            rect: paint_callback.rect,
                        }),
                        clip_rect,
                    });
                    continue;
                }
            };

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

            self.draw_commands.push(DrawCommand {
                primitive: DrawPrimitive::Egui(EguiDraw {
                    vertices_count: mesh.indices.len(),
                    egui_texture: texture_handle,
                }),
                clip_rect,
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

        for (clip_rect, command) in self.postponed_updates.drain(..) {
            let info = egui::PaintCallbackInfo {
                viewport: command.rect,
                clip_rect,
                pixels_per_point: self.pixels_per_point,
                screen_size_px: [
                    window_size.physical_width as u32,
                    window_size.physical_height as u32,
                ],
            };
            command
                .callback
                .cb()
                .update(info, self.window_entity, key, world);
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
        let extracted_window = extracted_windows.get(&self.window_entity);
        let swap_chain_texture_view =
            match extracted_window.and_then(|v| v.swap_chain_texture_view.as_ref()) {
                None => return Ok(()),
                Some(window) => window,
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

        let device = world.get_resource::<RenderDevice>().unwrap();

        let render_pass =
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
        let mut render_pass = TrackedRenderPass::new(device, render_pass);

        let (physical_width, physical_height, pipeline_key) = match extracted_window {
            Some(window) => (
                window.physical_width,
                window.physical_height,
                EguiPipelineKey::from_extracted_window(window),
            ),
            None => unreachable!(),
        };
        let Some(key) = pipeline_key else {
            return Ok(());
        };

        let pipeline_id = egui_pipelines.get(&self.window_entity).unwrap();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(*pipeline_id) else {
            return Ok(());
        };

        let transform_buffer_offset = egui_transforms.offsets[&self.window_entity];
        let transform_buffer_bind_group = &egui_transforms.bind_group.as_ref().unwrap().1;

        let mut requires_reset = true;

        let mut vertex_offset: u32 = 0;
        for draw_command in &self.draw_commands {
            if requires_reset {
                render_pass.set_viewport(
                    0.,
                    0.,
                    physical_width as f32,
                    physical_height as f32,
                    0.,
                    1.,
                );
                render_pass.set_render_pipeline(pipeline);
                render_pass.set_bind_group(
                    0,
                    transform_buffer_bind_group,
                    &[transform_buffer_offset],
                );

                requires_reset = false;
            }

            let clip_urect = bevy::math::URect {
                min: bevy::math::UVec2 {
                    x: (draw_command.clip_rect.min.x * self.pixels_per_point).round() as u32,
                    y: (draw_command.clip_rect.min.y * self.pixels_per_point).round() as u32,
                },
                max: bevy::math::UVec2 {
                    x: (draw_command.clip_rect.max.x * self.pixels_per_point).round() as u32,
                    y: (draw_command.clip_rect.max.y * self.pixels_per_point).round() as u32,
                },
            };

            let scrissor_rect = clip_urect.intersect(bevy::math::URect::new(
                0,
                0,
                physical_width,
                physical_height,
            ));
            if scrissor_rect.is_empty() {
                continue;
            }

            render_pass.set_scissor_rect(
                scrissor_rect.min.x,
                scrissor_rect.min.y,
                scrissor_rect.width(),
                scrissor_rect.height(),
            );

            match &draw_command.primitive {
                DrawPrimitive::Egui(command) => {
                    let texture_bind_group = match bind_groups.get(&command.egui_texture) {
                        Some(texture_resource) => texture_resource,
                        None => {
                            vertex_offset += command.vertices_count as u32;
                            continue;
                        }
                    };

                    render_pass.set_bind_group(1, texture_bind_group, &[]);

                    render_pass
                        .set_vertex_buffer(0, self.vertex_buffer.as_ref().unwrap().slice(..));
                    render_pass.set_index_buffer(
                        self.index_buffer.as_ref().unwrap().slice(..),
                        0,
                        IndexFormat::Uint32,
                    );

                    render_pass.draw_indexed(
                        vertex_offset..(vertex_offset + command.vertices_count as u32),
                        0,
                        0..1,
                    );

                    vertex_offset += command.vertices_count as u32;
                }
                DrawPrimitive::PaintCallback(command) => {
                    let info = egui::PaintCallbackInfo {
                        viewport: command.rect,
                        clip_rect: draw_command.clip_rect,
                        pixels_per_point: self.pixels_per_point,
                        screen_size_px: [physical_width, physical_height],
                    };

                    let viewport = info.viewport_in_pixels();
                    if viewport.width_px > 0 && viewport.height_px > 0 {
                        requires_reset = true;
                        render_pass.set_viewport(
                            viewport.left_px as f32,
                            viewport.top_px as f32,
                            viewport.width_px as f32,
                            viewport.height_px as f32,
                            0.,
                            1.,
                        );

                        command.callback.cb().render(
                            info,
                            &mut render_pass,
                            self.window_entity,
                            key,
                            world,
                        );
                    }
                }
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

/// Callback to execute custom 'wgpu' rendering inside [`EguiNode`] render graph node.
///
/// Rendering can be implemented using for example:
/// * native wgpu rendering libraries,
/// * or with [`bevy::render::render_phase`] approach.
pub struct EguiBevyPaintCallback(Box<dyn EguiBevyPaintCallbackImpl>);

impl EguiBevyPaintCallback {
    /// Creates a new [`egui::epaint::PaintCallback`] from a callback trait instance.
    pub fn new_paint_callback<T>(rect: egui::Rect, callback: T) -> egui::epaint::PaintCallback
    where
        T: EguiBevyPaintCallbackImpl + 'static,
    {
        let callback = Self(Box::new(callback));
        egui::epaint::PaintCallback {
            rect,
            callback: std::sync::Arc::new(callback),
        }
    }

    pub(crate) fn cb(&self) -> &dyn EguiBevyPaintCallbackImpl {
        self.0.as_ref()
    }
}

/// Callback that executes custom rendering logic
pub trait EguiBevyPaintCallbackImpl: Send + Sync {
    /// Paint callback will be rendered in near future, all data must be finalized for render step
    fn update(
        &self,
        info: egui::PaintCallbackInfo,
        window_entity: Entity,
        pipeline_key: EguiPipelineKey,
        world: &mut World,
    );

    /// Paint callback render step
    ///
    /// Native wgpu RenderPass can be retrieved from [`TrackedRenderPass`] by calling
    /// [`TrackedRenderPass::wgpu_pass`].
    fn render<'pass>(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut TrackedRenderPass<'pass>,
        window_entity: Entity,
        pipeline_key: EguiPipelineKey,
        world: &'pass World,
    );
}
