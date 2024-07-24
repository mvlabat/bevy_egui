use crate::{
    egui_node::DrawCommand,
    render_systems::{EguiPipelines, EguiTextureBindGroups, EguiTextureId, EguiTransforms},
    EguiRenderOutput, EguiRenderToTextureHandle, EguiSettings, WindowSize,
};
use bevy::{
    ecs::world::World,
    prelude::Entity,
    render::{
        render_asset::RenderAssets,
        render_graph::{Node, NodeRunError, RenderGraphContext, RenderLabel},
        render_resource::{
            Buffer, BufferAddress, BufferDescriptor, BufferUsages, IndexFormat, LoadOp, Operations,
            PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
    },
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
    window_entity: Entity,
    vertex_data: Vec<u8>,
    vertex_buffer_capacity: usize,
    vertex_buffer: Option<Buffer>,
    index_data: Vec<u8>,
    index_buffer_capacity: usize,
    index_buffer: Option<Buffer>,
    draw_commands: Vec<DrawCommand>,
}
impl EguiRenderToTextureNode {
    /// Constructs Egui render node.
    pub fn new(window_entity: Entity) -> Self {
        EguiRenderToTextureNode {
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
impl Node for EguiRenderToTextureNode {
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

        let extracted_render_to_texture: Option<&EguiRenderToTextureHandle> =
            world.get(self.window_entity);
        let render_to_texture_gpu_image = extracted_render_to_texture.map(|handle| {
            let w = world.get_resource::<RenderAssets<GpuImage>>().unwrap();
            w.get(&handle.0).unwrap()
        });
        let swap_chain_texture_view = match render_to_texture_gpu_image {
            None => return Ok(()),
            Some(rtt) => &rtt.texture_view,
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
                    label: Some("egui render to texture render pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: swap_chain_texture_view,
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

        let Some(pipeline_id) = egui_pipelines.get(&self.window_entity) else {
            bevy::log::error!("no egui_pipeline");
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

        let (physical_width, physical_height) = match render_to_texture_gpu_image {
            Some(rtt) => (rtt.size.x, rtt.size.y),
            None => unreachable!(),
        };

        let mut vertex_offset: u32 = 0;
        for draw_command in &self.draw_commands {
            if draw_command.clipping_zone.0 < physical_width
                && draw_command.clipping_zone.1 < physical_height
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
                    draw_command
                        .clipping_zone
                        .2
                        .min(physical_width.saturating_sub(draw_command.clipping_zone.0)),
                    draw_command
                        .clipping_zone
                        .3
                        .min(physical_height.saturating_sub(draw_command.clipping_zone.1)),
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
