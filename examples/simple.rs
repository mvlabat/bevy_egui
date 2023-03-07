use bevy::{prelude::*, window::PrimaryWindow};
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

fn ui_example_system(mut egui_ctx: Query<&mut EguiContext, With<PrimaryWindow>>) {
    egui::Window::new("Hello").show(egui_ctx.single_mut().get_mut(), |ui| {
        ui.label("world");
    });
}
