use bevy::prelude::*;
use bevy_egui::{egui, EguiContext, EguiPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        // Systems that create Egui widgets should be run during the `CoreStage::Update` stage,
        // or after the `EguiSystem::BeginFrame` system (which belongs to the `CoreStage::PreUpdate` stage).
        .add_system(ui_example_system)
        .run();
}

fn ui_example_system(mut egui_context: ResMut<EguiContext>, windows: Query<Entity, With<Window>>) {
    egui::Window::new("Hello").show(
        egui_context.ctx_for_window_mut(windows.iter().next().unwrap()),
        |ui| {
            ui.label("world");
        },
    );
}
