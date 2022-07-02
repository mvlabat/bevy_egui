use bevy::{
    prelude::*,
    render::{camera::RenderTarget, render_graph::RenderGraph, RenderApp},
    window::{CreateWindow, PresentMode, WindowId},
    winit::WinitSettings,
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
        // Optimal power saving and present mode settings for desktop apps.
        .insert_resource(WinitSettings::desktop_app())
        .insert_resource(WindowDescriptor {
            present_mode: PresentMode::Mailbox,
            ..Default::default()
        })
        .add_plugin(EguiPlugin)
        .init_resource::<SharedUiState>()
        .add_startup_system(load_assets)
        .add_startup_system(create_new_window)
        .add_system(ui_first_window)
        .add_system(ui_second_window);

    let render_app = app.sub_app_mut(RenderApp);
    let mut graph = render_app.world.get_resource_mut::<RenderGraph>().unwrap();

    bevy_egui::setup_pipeline(
        &mut graph,
        bevy_egui::RenderGraphConfig {
            window_id: *SECOND_WINDOW_ID,
            egui_pass: SECONDARY_EGUI_PASS,
        },
    );

    app.run();
}

const SECONDARY_EGUI_PASS: &str = "secondary_egui_pass";

fn create_new_window(mut create_window_events: EventWriter<CreateWindow>, mut commands: Commands) {
    // sends out a "CreateWindow" event, which will be received by the windowing backend
    create_window_events.send(CreateWindow {
        id: *SECOND_WINDOW_ID,
        descriptor: WindowDescriptor {
            width: 800.,
            height: 600.,
            present_mode: PresentMode::Mailbox,
            title: "Second window".to_string(),
            ..Default::default()
        },
    });
    // second window camera
    commands.spawn_bundle(Camera3dBundle {
        camera: Camera {
            target: RenderTarget::Window(*SECOND_WINDOW_ID),
            ..Default::default()
        },
        transform: Transform::from_xyz(6.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Images {
        bevy_icon: assets.load("icon.png"),
    });
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

            ui.add(egui::widgets::Image::new(bevy_texture_id, [256.0, 256.0]));
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

            ui.add(egui::widgets::Image::new(bevy_texture_id, [256.0, 256.0]));
        });
}
