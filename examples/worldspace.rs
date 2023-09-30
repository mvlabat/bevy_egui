use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureUsages},
};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiRenderToTexture};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1.,
        })
        .add_systems(Startup, setup_worldspace)
        // Systems that create Egui widgets should be run during the `CoreSet::Update` set,
        // or after the `EguiSet::BeginFrame` system (which belongs to the `CoreSet::PreUpdate` set).
        .add_systems(Update, (update_screenspace, update_worldspace))
        .run();
}

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
        mesh: meshes.add(shape::Cube::default().into()),
        material: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(Handle::clone(&output_texture)),
            // Remove this if you want it to use the world's lighting.
            unlit: true,
            ..default()
        }),
        ..default()
    });
    commands.spawn(EguiRenderToTexture(output_texture));
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(1.5, 1.5, 1.5).looking_at(Vec3::new(0., 0., 0.), Vec3::Y),
        ..default()
    });
}

fn update_screenspace(mut contexts: EguiContexts) {
    egui::Window::new("Screenspace UI").show(contexts.ctx_mut(), |ui| {
        ui.label("I'm rendering to screenspace!");
    });
}

fn update_worldspace(mut contexts: Query<&mut bevy_egui::EguiContext, With<EguiRenderToTexture>>) {
    for mut ctx in contexts.iter_mut() {
        egui::Window::new("Worldspace UI").show(ctx.get_mut(), |ui| {
            ui.label("I'm rendering to a texture in worldspace!");
        });
    }
}
