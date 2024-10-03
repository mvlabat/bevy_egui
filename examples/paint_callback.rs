use bevy::{
    asset::{embedded_asset, AssetPath},
    prelude::*,
    render::{
        mesh::PrimitiveTopology,
        render_resource::{
            BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState,
            MultisampleState, PipelineCache, PolygonMode, PrimitiveState, RenderPipelineDescriptor,
            SpecializedRenderPipeline, SpecializedRenderPipelines,
        },
        RenderApp,
    },
};
use bevy_egui::{
    egui_node::{EguiBevyPaintCallback, EguiBevyPaintCallbackImpl, EguiPipelineKey},
    EguiContexts, EguiPlugin, EguiRenderToTextureHandle,
};
use std::path::Path;
use wgpu_types::{Extent3d, TextureUsages};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, EguiPlugin::default(), CustomPipelinePlugin))
        .add_systems(Startup, setup_worldspace)
        .add_systems(
            Update,
            (ui_example_system, ui_render_to_texture_example_system),
        )
        .run();
}

struct CustomPipelinePlugin;

impl Plugin for CustomPipelinePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "examples/", "paint_callback.wgsl");
        app.get_sub_app_mut(RenderApp)
            .unwrap()
            .insert_resource(SpecializedRenderPipelines::<CustomPipeline>::default())
            .init_resource::<CustomPipeline>();
    }
}

struct CustomPaintCallback;

#[derive(Component)]
struct CustomPaintPipelineIdComp {
    pipeline_id: CachedRenderPipelineId,
}

impl EguiBevyPaintCallbackImpl for CustomPaintCallback {
    fn update(
        &self,
        _info: egui::PaintCallbackInfo,
        window_entity: Entity,
        key: EguiPipelineKey,
        world: &mut World,
    ) {
        let pipeline_id =
            world.resource_scope(
                |world,
                 mut specialized_custom_pipelines: Mut<
                    SpecializedRenderPipelines<CustomPipeline>,
                >| {
                    let specialized_pipeline = world.get_resource().unwrap();
                    let pipeline_cache = world.get_resource().unwrap();

                    let pipeline_id = specialized_custom_pipelines.specialize(
                        pipeline_cache,
                        specialized_pipeline,
                        key,
                    );

                    world
                        .entity_mut(window_entity)
                        .insert(CustomPaintPipelineIdComp { pipeline_id });
                    pipeline_id
                },
            );

        let mut pipeline_cache = world.get_resource_mut::<PipelineCache>().unwrap();
        pipeline_cache.block_on_render_pipeline(pipeline_id);
    }

    fn render<'pass>(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut bevy::render::render_phase::TrackedRenderPass<'pass>,
        window_entity: Entity,
        _key: EguiPipelineKey,
        world: &'pass World,
    ) {
        let Some(pipeline) = world
            .get_entity(window_entity)
            .and_then(|entity| entity.get::<CustomPaintPipelineIdComp>())
            .and_then(|comp| {
                world
                    .get_resource::<PipelineCache>()
                    .and_then(|cache| cache.get_render_pipeline(comp.pipeline_id))
            })
        else {
            return;
        };

        render_pass.set_render_pipeline(pipeline);
        render_pass.draw(0..3, 0..1);
    }
}

#[derive(Debug, Resource)]
struct CustomPipeline {
    shader: Handle<Shader>,
}

impl FromWorld for CustomPipeline {
    fn from_world(world: &mut World) -> Self {
        let shader = world.resource::<AssetServer>().load(
            AssetPath::from_path(Path::new("paint_callback/paint_callback.wgsl"))
                .with_source("embedded"),
        );

        Self { shader }
    }
}

impl SpecializedRenderPipeline for CustomPipeline {
    type Key = EguiPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("custom pipeline".into()),
            layout: vec![],
            push_constant_ranges: Vec::new(),
            vertex: bevy::render::render_resource::VertexState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: "vertex".into(),
                buffers: vec![],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: bevy::render::render_resource::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: key.texture_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
        }
    }
}

fn ui_example_system(mut ctx: EguiContexts) {
    for id in 0..4 {
        egui::Window::new(id.to_string()).show(ctx.ctx_mut(), |ui| {
            let (resp, painter) =
                ui.allocate_painter(egui::Vec2 { x: 200., y: 200. }, egui::Sense::hover());

            painter.add(EguiBevyPaintCallback::new_paint_callback(
                resp.rect,
                CustomPaintCallback,
            ));
        });
    }
}

// The following systems are used to render UI in world space to demonstrate that paint callbacks
// work for them as well (they aren't needed to set up pain callbacks for regular screen-space UI,
// so feel free to skip them):

fn setup_worldspace(
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let output_texture = images.add({
        let size = Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        };
        let mut output_texture = Image {
            // You should use `0` so that the pixels are transparent.
            data: vec![0; (size.width * size.height * 4) as usize],
            ..default()
        };
        output_texture.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
        output_texture.texture_descriptor.size = size;
        output_texture
    });

    commands.spawn(PbrBundle {
        mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0).mesh()),
        material: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(Handle::clone(&output_texture)),
            alpha_mode: AlphaMode::Blend,
            // Remove this if you want it to use the world's lighting.
            unlit: true,
            ..default()
        }),
        ..default()
    });
    commands.spawn(EguiRenderToTextureHandle(output_texture));
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(1.5, 1.5, 1.5).looking_at(Vec3::new(0., 0., 0.), Vec3::Y),
        ..default()
    });
}

fn ui_render_to_texture_example_system(
    mut contexts: Query<&mut bevy_egui::EguiContext, With<EguiRenderToTextureHandle>>,
) {
    for mut ctx in contexts.iter_mut() {
        egui::Window::new("Worldspace UI").show(ctx.get_mut(), |ui| {
            let (resp, painter) =
                ui.allocate_painter(egui::Vec2 { x: 200., y: 200. }, egui::Sense::hover());

            painter.add(EguiBevyPaintCallback::new_paint_callback(
                resp.rect,
                CustomPaintCallback,
            ));
        });
    }
}
