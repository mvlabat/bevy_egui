use crate::{
    egui_node::{
        DrawCommand, DrawPrimitive, EguiBevyPaintCallback, EguiDraw, EguiPipelineKey,
        PaintCallbackDraw,
    },
    render_systems::{EguiPipelines, EguiTextureBindGroups, EguiTextureId, EguiTransforms},
    EguiRenderOutput, EguiRenderToTextureHandle, EguiSettings, RenderTargetSize,
};
use bevy_ecs::world::World;
use bevy_render::{
    render_asset::RenderAssets,
    render_graph::{Node, NodeRunError, RenderGraphContext, RenderLabel},
    render_phase::TrackedRenderPass,
    render_resource::{
        Buffer, BufferAddress, BufferDescriptor, BufferUsages, IndexFormat, LoadOp, Operations,
        PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue},
    sync_world::{MainEntity, RenderEntity},
    texture::GpuImage,
};

use bytemuck::cast_slice;

/// [`RenderLabel`] type for the Egui Render to Texture pass.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct EguiRenderToTexturePass {
    /// Index of the window entity.
    pub entity_index: u32,
    /// Generation of the window entity.
    pub entity_generation: u32,
}

/// Egui render to texture node.
pub struct EguiRenderToTextureNode {
    render_to_texture_target_render: RenderEntity,
    render_to_texture_target_main: MainEntity,
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
impl EguiRenderToTextureNode {
    /// Constructs Egui render node.
    pub fn new(
        render_to_texture_target_render: RenderEntity,
        render_to_texture_target_main: MainEntity,
    ) -> Self {
        EguiRenderToTextureNode {
            render_to_texture_target_render,
            render_to_texture_target_main,
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
impl Node for EguiRenderToTextureNode {
    fn update(&mut self, world: &mut World) {
        let Ok(image_handle) = world
            .query::<&EguiRenderToTextureHandle>()
            .get(world, self.render_to_texture_target_render.id())
            .map(|handle| handle.0.clone_weak())
        else {
            return;
        };
        let Some(key) = world
            .get_resource::<RenderAssets<GpuImage>>()
            .and_then(|render_assets| render_assets.get(&image_handle))
            .map(EguiPipelineKey::from_gpu_image)
        else {
            return;
        };

        let mut render_target_query =
            world.query::<(&EguiSettings, &RenderTargetSize, &mut EguiRenderOutput)>();
        let Ok((egui_settings, render_target_size, mut render_output)) =
            render_target_query.get_mut(world, self.render_to_texture_target_render.id())
        else {
            return;
        };

        let render_target_size = *render_target_size;
        let paint_jobs = std::mem::take(&mut render_output.paint_jobs);

        self.pixels_per_point = render_target_size.scale_factor * egui_settings.scale_factor;
        if render_target_size.physical_width == 0.0 || render_target_size.physical_height == 0.0 {
            return;
        }

        let render_device = world.get_resource::<RenderDevice>().unwrap();
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
            let clip_urect = bevy_math::URect {
                min: bevy_math::UVec2 {
                    x: (clip_rect.min.x * self.pixels_per_point).round() as u32,
                    y: (clip_rect.min.y * self.pixels_per_point).round() as u32,
                },
                max: bevy_math::UVec2 {
                    x: (clip_rect.max.x * self.pixels_per_point).round() as u32,
                    y: (clip_rect.max.y * self.pixels_per_point).round() as u32,
                },
            };

            if clip_urect
                .intersect(bevy_math::URect::new(
                    0,
                    0,
                    render_target_size.physical_width as u32,
                    render_target_size.physical_height as u32,
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
                egui::TextureId::Managed(id) => {
                    EguiTextureId::Managed(self.render_to_texture_target_main, id)
                }
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
                    render_target_size.physical_width as u32,
                    render_target_size.physical_height as u32,
                ],
            };
            command
                .callback
                .cb()
                .update(info, self.render_to_texture_target_render, key, world);
        }
    }

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let egui_pipelines = &world.get_resource::<EguiPipelines>().unwrap().0;
        let pipeline_cache = world.get_resource::<PipelineCache>().unwrap();

        let extracted_render_to_texture: Option<&EguiRenderToTextureHandle> =
            world.get(self.render_to_texture_target_render.id());
        let Some(render_to_texture_gpu_image) = extracted_render_to_texture else {
            return Ok(());
        };

        let gpu_images = world.get_resource::<RenderAssets<GpuImage>>().unwrap();
        let gpu_image = gpu_images.get(&render_to_texture_gpu_image.0).unwrap();
        let key = EguiPipelineKey::from_gpu_image(gpu_image);

        let render_queue = world.get_resource::<RenderQueue>().unwrap();

        let (vertex_buffer, index_buffer) = match (&self.vertex_buffer, &self.index_buffer) {
            (Some(vertex), Some(index)) => (vertex, index),
            _ => return Ok(()),
        };

        render_queue.write_buffer(vertex_buffer, 0, &self.vertex_data);
        render_queue.write_buffer(index_buffer, 0, &self.index_data);

        for draw_command in &self.draw_commands {
            match &draw_command.primitive {
                DrawPrimitive::Egui(_command) => {}
                DrawPrimitive::PaintCallback(command) => {
                    let info = egui::PaintCallbackInfo {
                        viewport: command.rect,
                        clip_rect: draw_command.clip_rect,
                        pixels_per_point: self.pixels_per_point,
                        screen_size_px: [gpu_image.size.x, gpu_image.size.y],
                    };

                    command.callback.cb().prepare_render(
                        info,
                        render_context,
                        self.render_to_texture_target_render,
                        key,
                        world,
                    );
                }
            }
        }

        let bind_groups = &world.get_resource::<EguiTextureBindGroups>().unwrap();

        let egui_transforms = world.get_resource::<EguiTransforms>().unwrap();

        let device = world.get_resource::<RenderDevice>().unwrap();

        let render_pass =
            render_context
                .command_encoder()
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("egui render to texture render pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &gpu_image.texture_view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(wgpu_types::Color::TRANSPARENT),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

        let mut render_pass = TrackedRenderPass::new(device, render_pass);

        let Some(pipeline_id) = egui_pipelines.get(&self.render_to_texture_target_main) else {
            bevy_log::error!("no egui_pipeline");
            return Ok(());
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(*pipeline_id) else {
            return Ok(());
        };

        let transform_buffer_offset = egui_transforms.offsets[&self.render_to_texture_target_main];
        let transform_buffer_bind_group = &egui_transforms.bind_group.as_ref().unwrap().1;

        let mut requires_reset = true;

        let mut vertex_offset: u32 = 0;
        for draw_command in &self.draw_commands {
            if requires_reset {
                render_pass.set_viewport(
                    0.,
                    0.,
                    gpu_image.size.x as f32,
                    gpu_image.size.y as f32,
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

            let clip_urect = bevy_math::URect {
                min: bevy_math::UVec2 {
                    x: (draw_command.clip_rect.min.x * self.pixels_per_point).round() as u32,
                    y: (draw_command.clip_rect.min.y * self.pixels_per_point).round() as u32,
                },
                max: bevy_math::UVec2 {
                    x: (draw_command.clip_rect.max.x * self.pixels_per_point).round() as u32,
                    y: (draw_command.clip_rect.max.y * self.pixels_per_point).round() as u32,
                },
            };
            let scrissor_rect = clip_urect.intersect(bevy_math::URect::from_corners(
                bevy_math::UVec2::ZERO,
                gpu_image.size,
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
                        screen_size_px: [gpu_image.size.x, gpu_image.size.y],
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
                            self.render_to_texture_target_render,
                            key,
                            world,
                        );
                    }
                }
            }

            // if (draw_command.clip_rect.min.x as u32) < physical_width
            //     && (draw_command.clip_rect.min.y as u32) < physical_height
            // {
            //     let draw_primitive = match &draw_command.primitive {
            //         DrawPrimitive::Egui(draw_primitive) => draw_primitive,
            //         DrawPrimitive::PaintCallback(_) => unimplemented!(),
            //     };
            //     let texture_bind_group = match bind_groups.get(&draw_primitive.egui_texture) {
            //         Some(texture_resource) => texture_resource,
            //         None => {
            //             vertex_offset += draw_primitive.vertices_count as u32;
            //             continue;
            //         }
            //     };
            //
            //     render_pass.set_bind_group(1, texture_bind_group, &[]);
            //
            //     render_pass.set_scissor_rect(
            //         draw_command.clip_rect.min.x as u32,
            //         draw_command.clip_rect.min.y as u32,
            //         (draw_command.clip_rect.width() as u32)
            //             .min(physical_width.saturating_sub(draw_command.clip_rect.min.x as u32)),
            //         (draw_command.clip_rect.height() as u32)
            //             .min(physical_height.saturating_sub(draw_command.clip_rect.min.y as u32)),
            //     );
            //
            //     render_pass.draw_indexed(
            //         vertex_offset..(vertex_offset + draw_primitive.vertices_count as u32),
            //         0,
            //         0..1,
            //     );
            //     vertex_offset += draw_primitive.vertices_count as u32;
            // }
        }

        Ok(())
    }
}
