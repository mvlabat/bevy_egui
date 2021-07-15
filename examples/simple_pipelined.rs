use bevy_egui::{EguiContext, EguiPlugin};

use bevy::{
    self,
    ecs::prelude::*,
    math::Vec3,
    prelude::{App, Transform},
    render2::camera::PerspectiveCameraBundle,
    PipelinedDefaultPlugins,
};

fn main() {
    App::new()
        .add_plugins(PipelinedDefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_system(ui_example.system())
        .add_startup_system(setup.system())
        .run();
}

fn ui_example(mut color: Local<[f32; 3]>, egui_context: Res<EguiContext>) {
    egui::Window::new("Hello").show(egui_context.ctx(), |ui| {
        ui.label("world");
        ui.color_edit_button_rgb(&mut *color);
    });
}

fn setup(mut commands: Commands) {
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        ..Default::default()
    });
}
