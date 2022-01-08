use bevy::prelude::*;
use bevy_egui::{egui, EguiContext, EguiPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_system(ui_example.system())
        .run();
}

fn ui_example(egui_context: Res<EguiContext>) {
    egui::Window::new("Hello").show(egui_context.ctx(), |ui| {
        ui.label("world");
    });
}
