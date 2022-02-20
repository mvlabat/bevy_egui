use bevy::{
    core_pipeline::{draw_3d_graph, node, AlphaMask3d, Opaque3d, Transparent3d},
    prelude::*,
    render::{
        camera::{ActiveCameras, ExtractedCameraNames},
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, SlotValue},
        render_phase::RenderPhase,
        renderer::RenderContext,
        RenderApp, RenderStage,
    },
    window::{CreateWindow, WindowId},
};
use bevy_egui::{EguiContext, EguiPlugin};
use once_cell::sync::Lazy;

static SECOND_WINDOW_ID: Lazy<WindowId> = Lazy::new(WindowId::new);

struct Images {
    bevy_icon: Handle<Image>,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .init_resource::<SharedUiState>()
        .add_startup_system(load_assets)
        .add_startup_system(create_new_window)
        .add_system(ui_first_window)
        .add_system(ui_second_window);

    let render_app = app.sub_app_mut(RenderApp);
    render_app.add_system_to_stage(RenderStage::Extract, extract_secondary_camera_phases);
    let mut graph = render_app.world.get_resource_mut::<RenderGraph>().unwrap();
    graph.add_node(SECONDARY_PASS_DRIVER, SecondaryCameraDriver);
    graph
        .add_node_edge(node::MAIN_PASS_DEPENDENCIES, SECONDARY_PASS_DRIVER)
        .unwrap();

    bevy_egui::setup_pipeline(
        &mut graph,
        bevy_egui::RenderGraphConfig {
            window_id: *SECOND_WINDOW_ID,
            egui_pass: SECONDARY_EGUI_PASS,
        },
    );

    app.run();
}

fn extract_secondary_camera_phases(mut commands: Commands, active_cameras: Res<ActiveCameras>) {
    if let Some(secondary) = active_cameras.get(SECONDARY_CAMERA_NAME) {
        if let Some(entity) = secondary.entity {
            commands.get_or_spawn(entity).insert_bundle((
                RenderPhase::<Opaque3d>::default(),
                RenderPhase::<AlphaMask3d>::default(),
                RenderPhase::<Transparent3d>::default(),
            ));
        }
    }
}

const SECONDARY_CAMERA_NAME: &str = "Secondary";
const SECONDARY_PASS_DRIVER: &str = "secondary_pass_driver";
const SECONDARY_EGUI_PASS: &str = "secondary_egui_pass";

fn create_new_window(
    mut create_window_events: EventWriter<CreateWindow>,
    mut commands: Commands,
    mut active_cameras: ResMut<ActiveCameras>,
) {
    // sends out a "CreateWindow" event, which will be received by the windowing backend
    create_window_events.send(CreateWindow {
        id: *SECOND_WINDOW_ID,
        descriptor: WindowDescriptor {
            width: 800.,
            height: 600.,
            vsync: false,
            title: "Second window".to_string(),
            ..Default::default()
        },
    });
    // second window camera
    commands.spawn_bundle(PerspectiveCameraBundle {
        camera: Camera {
            window: *SECOND_WINDOW_ID,
            name: Some(SECONDARY_CAMERA_NAME.into()),
            ..Default::default()
        },
        transform: Transform::from_xyz(6.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });

    active_cameras.add(SECONDARY_CAMERA_NAME);
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Images {
        bevy_icon: assets.load("icon.png"),
    });
}

struct SecondaryCameraDriver;
impl Node for SecondaryCameraDriver {
    fn run(
        &self,
        graph: &mut RenderGraphContext,
        _render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let extracted_cameras = world.get_resource::<ExtractedCameraNames>().unwrap();
        if let Some(camera_3d) = extracted_cameras.entities.get(SECONDARY_CAMERA_NAME) {
            graph.run_sub_graph(
                crate::draw_3d_graph::NAME,
                vec![SlotValue::Entity(*camera_3d)],
            )?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct UiState {
    input: String,
}

#[derive(Default)]
struct SharedUiState {
    shared_input: String,
}

fn ui_first_window(
    mut egui_context: ResMut<EguiContext>,
    mut ui_state: Local<UiState>,
    mut shared_ui_state: ResMut<SharedUiState>,
    images: Res<Images>,
) {
    let bevy_texture_id = egui_context.add_image(images.bevy_icon.clone_weak());
    egui::Window::new("First Window")
        .vscroll(true)
        .show(egui_context.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(&mut ui_state.input);
            });
            ui.horizontal(|ui| {
                ui.label("Shared input: ");
                ui.text_edit_singleline(&mut shared_ui_state.shared_input);
            });

            ui.add(egui::widgets::Image::new(
                egui::TextureId::User(bevy_texture_id),
                [256.0, 256.0],
            ));
        });
}

fn ui_second_window(
    mut egui_context: ResMut<EguiContext>,
    mut ui_state: Local<UiState>,
    mut shared_ui_state: ResMut<SharedUiState>,
    images: Res<Images>,
) {
    let bevy_texture_id = egui_context.add_image(images.bevy_icon.clone_weak());
    let ctx = match egui_context.try_ctx_for_window_mut(*SECOND_WINDOW_ID) {
        Some(ctx) => ctx,
        None => return,
    };
    egui::Window::new("Second Window")
        .vscroll(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Write something else: ");
                ui.text_edit_singleline(&mut ui_state.input);
            });
            ui.horizontal(|ui| {
                ui.label("Shared input: ");
                ui.text_edit_singleline(&mut shared_ui_state.shared_input);
            });

            ui.add(egui::widgets::Image::new(
                egui::TextureId::User(bevy_texture_id),
                [256.0, 256.0],
            ));
        });
}
