use bevy::{
    prelude::*,
    render::camera::RenderTarget,
    window::{PresentMode, PrimaryWindow, WindowRef, WindowResolution},
};
use bevy_egui::{EguiContext, EguiPlugin, EguiUserTextures};

#[derive(Resource)]
struct Images {
    bevy_icon: Handle<Image>,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .init_resource::<SharedUiState>()
        .add_systems(Startup, (load_assets_system, create_new_window_system))
        .add_systems(Update, (ui_first_window_system, ui_second_window_system))
        .run();
}

fn create_new_window_system(mut commands: Commands) {
    // Spawn a second window
    let second_window_id = commands
        .spawn(Window {
            title: "Second window".to_owned(),
            resolution: WindowResolution::new(800.0, 600.0),
            present_mode: PresentMode::AutoVsync,
            ..Default::default()
        })
        .id();

    // second window camera
    commands.spawn(Camera3dBundle {
        camera: Camera {
            target: RenderTarget::Window(WindowRef::Entity(second_window_id)),
            ..Default::default()
        },
        transform: Transform::from_xyz(6.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
}

fn load_assets_system(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Images {
        bevy_icon: assets.load("icon.png"),
    });
}

#[derive(Default)]
struct UiState {
    input: String,
}

#[derive(Default, Resource)]
struct SharedUiState {
    shared_input: String,
}

fn ui_first_window_system(
    mut egui_user_textures: ResMut<EguiUserTextures>,
    mut ui_state: Local<UiState>,
    mut shared_ui_state: ResMut<SharedUiState>,
    images: Res<Images>,
    mut egui_ctx: Query<&mut EguiContext, With<PrimaryWindow>>,
) {
    let bevy_texture_id = egui_user_textures.add_image(images.bevy_icon.clone_weak());
    let Ok(mut ctx) = egui_ctx.get_single_mut() else { return; };
    egui::Window::new("First Window")
        .vscroll(true)
        .show(ctx.get_mut(), |ui| {
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

fn ui_second_window_system(
    mut egui_user_textures: ResMut<EguiUserTextures>,
    mut ui_state: Local<UiState>,
    mut shared_ui_state: ResMut<SharedUiState>,
    images: Res<Images>,
    mut egui_ctx: Query<&mut EguiContext, Without<PrimaryWindow>>,
) {
    let bevy_texture_id = egui_user_textures.add_image(images.bevy_icon.clone_weak());
    let Ok(mut ctx) = egui_ctx.get_single_mut() else { return; };
    egui::Window::new("Second Window")
        .vscroll(true)
        .show(ctx.get_mut(), |ui| {
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
