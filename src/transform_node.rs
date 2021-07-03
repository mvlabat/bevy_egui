use crate::{EguiSettings, WindowSize, EGUI_TRANSFORM_RESOURCE_BINDING_NAME};
use bevy::{
    core::bytes_of,
    ecs::{
        system::{IntoSystem, Local, Res, ResMut, System},
        world::World,
    },
    render::{
        render_graph::{CommandQueue, Node, ResourceSlots, SystemNode},
        renderer::{
            BufferId, BufferInfo, BufferMapMode, BufferUsage, RenderContext, RenderResourceBinding,
            RenderResourceBindings, RenderResourceContext,
        },
    },
    utils::HashMap,
    window::WindowId,
};

#[derive(Debug)]
pub struct EguiTransformNode {
    window_id: WindowId,
    command_queue: CommandQueue,
}

impl EguiTransformNode {
    pub fn new(window_id: WindowId) -> Self {
        EguiTransformNode {
            window_id,
            command_queue: Default::default(),
        }
    }
}

impl Node for EguiTransformNode {
    fn update(
        &mut self,
        _world: &World,
        render_context: &mut dyn RenderContext,
        _input: &ResourceSlots,
        _output: &mut ResourceSlots,
    ) {
        self.command_queue.execute(render_context);
    }
}

impl SystemNode for EguiTransformNode {
    fn get_system(&self) -> Box<dyn System<In = (), Out = ()>> {
        let system = transform_node_system.system().config(|c| {
            c.0 = Some(TransformNodeState {
                window_id: self.window_id,
                command_queue: self.command_queue.clone(),
                transform_buffer: None,
                staging_buffer: None,
                prev_window_size: WindowSize::new(0.0, 0.0, 0.0),
                prev_scale_factor: 0.0,
            });
        });
        Box::new(system)
    }
}

#[derive(Default)]
pub struct TransformNodeState {
    window_id: WindowId,
    command_queue: CommandQueue,
    transform_buffer: Option<BufferId>,
    staging_buffer: Option<BufferId>,
    prev_window_size: WindowSize,
    prev_scale_factor: f64,
}

fn transform_node_system(
    mut state: Local<TransformNodeState>,
    render_resource_context: Res<Box<dyn RenderResourceContext>>,
    window_size: Res<HashMap<WindowId, WindowSize>>,
    egui_settings: Res<EguiSettings>,
    mut render_resource_bindings: ResMut<RenderResourceBindings>,
) {
    let window_size = &window_size[&state.window_id];

    #[allow(clippy::float_cmp)]
    if state.prev_window_size == *window_size
        && state.prev_scale_factor == egui_settings.scale_factor
    {
        return;
    }
    state.prev_window_size = window_size.clone();
    state.prev_scale_factor = egui_settings.scale_factor;

    let render_resource_context = &**render_resource_context;
    let transform_data_size = std::mem::size_of::<[[f32; 2]; 2]>();

    let staging_buffer = if let Some(staging_buffer) = state.staging_buffer {
        render_resource_context.map_buffer(staging_buffer, BufferMapMode::Write);
        staging_buffer
    } else {
        let buffer = render_resource_context.create_buffer(BufferInfo {
            size: transform_data_size,
            buffer_usage: BufferUsage::COPY_DST | BufferUsage::UNIFORM,
            ..Default::default()
        });
        render_resource_bindings.set(
            EGUI_TRANSFORM_RESOURCE_BINDING_NAME,
            RenderResourceBinding::Buffer {
                buffer,
                range: 0..transform_data_size as u64,
                dynamic_index: None,
            },
        );
        state.transform_buffer = Some(buffer);

        let staging_buffer = render_resource_context.create_buffer(BufferInfo {
            size: transform_data_size,
            buffer_usage: BufferUsage::COPY_SRC | BufferUsage::MAP_WRITE,
            mapped_at_creation: true,
        });

        state.staging_buffer = Some(staging_buffer);
        staging_buffer
    };

    let transform_data: [f32; 4] = [
        2.0 / (window_size.width() / egui_settings.scale_factor as f32),
        -2.0 / (window_size.height() / egui_settings.scale_factor as f32), // scale
        -1.0,
        1.0, // translation
    ];

    render_resource_context.write_mapped_buffer(
        staging_buffer,
        0..transform_data_size as u64,
        &mut |data, _renderer| {
            data[0..transform_data_size].copy_from_slice(bytes_of(&transform_data));
        },
    );
    render_resource_context.unmap_buffer(staging_buffer);

    let transform_buffer = state.transform_buffer.unwrap();
    state.command_queue.copy_buffer_to_buffer(
        staging_buffer,
        0,
        transform_buffer,
        0,
        transform_data_size as u64,
    );
}
