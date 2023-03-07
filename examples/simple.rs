use bevy::{prelude::*, window::PrimaryWindow};
use bevy_egui::{egui, EguiContext, EguiPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        // Systems that create Egui widgets should be run during the `CoreSet::Update` set,
        // or after the `EguiSet::BeginFrame` system (which belongs to the `CoreSet::PreUpdate` set).
        .add_system(ui_example_system)
        .run();
}

fn ui_example_system(mut egui_ctx: Query<&mut EguiContext, With<PrimaryWindow>>) {
    egui::Window::new("Hello").show(egui_ctx.single_mut().get_mut(), |ui| {
        ui.label("world");
    });
}
