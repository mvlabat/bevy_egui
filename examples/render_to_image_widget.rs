use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    prelude::*,
    render::{
        camera::RenderTarget,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        view::RenderLayers,
    },
};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiUserTextures};
use egui::Widget;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (rotator_system, render_to_image_example_system))
        .run();
}

// Marks the preview pass cube.
#[derive(Component)]
struct PreviewPassCube;

// Marks the main pass cube, to which the material is applied.
#[derive(Component)]
struct MainPassCube;

#[derive(Deref, Resource)]
struct CubePreviewImage(Handle<Image>);

fn setup(
    mut egui_user_textures: ResMut<EguiUserTextures>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let size = Extent3d {
        width: 512,
        height: 512,
        ..default()
    };

    // This is the texture that will be rendered to.
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };

    // fill image.data with zeroes
    image.resize(size);

    let image_handle = images.add(image);
    egui_user_textures.add_image(image_handle.clone());
    commands.insert_resource(CubePreviewImage(image_handle.clone()));

    let cube_handle = meshes.add(Mesh::from(shape::Cube { size: 4.0 }));
    let default_material = StandardMaterial {
        base_color: Color::rgb(0.8, 0.7, 0.6),
        reflectance: 0.02,
        unlit: false,
        ..default()
    };
    let preview_material_handle = materials.add(default_material.clone());

    // This specifies the layer used for the preview pass, which will be attached to the preview pass camera and cube.
    let preview_pass_layer = RenderLayers::layer(1);

    // The cube that will be rendered to the texture.
    commands
        .spawn(PbrBundle {
            mesh: cube_handle,
            material: preview_material_handle,
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)),
            ..default()
        })
        .insert(PreviewPassCube)
        .insert(preview_pass_layer);

    // Light
    // NOTE: Currently lights are shared between passes - see https://github.com/bevyengine/bevy/issues/3462
    commands.spawn(PointLightBundle {
        transform: Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)),
        ..default()
    });

    commands
        .spawn(Camera3dBundle {
            camera_3d: Camera3d {
                clear_color: ClearColorConfig::Custom(Color::rgba(1.0, 1.0, 1.0, 0.0)),
                ..default()
            },
            camera: Camera {
                // render before the "main pass" camera
                order: -1,
                target: RenderTarget::Image(image_handle),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 15.0))
                .looking_at(Vec3::default(), Vec3::Y),
            ..default()
        })
        .insert(preview_pass_layer);

    let cube_size = 4.0;
    let cube_handle = meshes.add(Mesh::from(shape::Box::new(cube_size, cube_size, cube_size)));

    let main_material_handle = materials.add(default_material);

    // Main pass cube.
    commands
        .spawn(PbrBundle {
            mesh: cube_handle,
            material: main_material_handle,
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, 1.5),
                rotation: Quat::from_rotation_x(-std::f32::consts::PI / 5.0),
                ..default()
            },
            ..default()
        })
        .insert(MainPassCube);

    // The main pass camera.
    commands.spawn(Camera3dBundle {
        transform: Transform::from_translation(Vec3::new(0.0, 0.0, 15.0))
            .looking_at(Vec3::default(), Vec3::Y),
        ..default()
    });
}

fn render_to_image_example_system(
    cube_preview_image: Res<CubePreviewImage>,
    preview_cube_query: Query<&Handle<StandardMaterial>, With<PreviewPassCube>>,
    main_cube_query: Query<&Handle<StandardMaterial>, With<MainPassCube>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut contexts: EguiContexts,
) {
    let cube_preview_texture_id = contexts.image_id(&cube_preview_image).unwrap();
    let preview_material_handle = preview_cube_query.single();
    let preview_material = materials.get_mut(preview_material_handle).unwrap();

    let ctx = contexts.ctx_mut();
    let mut apply = false;
    egui::Window::new("Cube material preview").show(ctx, |ui| {
        ui.image(cube_preview_texture_id, [300.0, 300.0]);
        egui::Grid::new("preview").show(ui, |ui| {
            ui.label("Base color:");
            color_picker_widget(ui, &mut preview_material.base_color);
            ui.end_row();

            ui.label("Emissive:");
            color_picker_widget(ui, &mut preview_material.emissive);
            ui.end_row();

            ui.label("Perceptual roughness:");
            egui::Slider::new(&mut preview_material.perceptual_roughness, 0.089..=1.0).ui(ui);
            ui.end_row();

            ui.label("Reflectance:");
            egui::Slider::new(&mut preview_material.reflectance, 0.0..=1.0).ui(ui);
            ui.end_row();

            ui.label("Unlit:");
            ui.checkbox(&mut preview_material.unlit, "");
            ui.end_row();
        });

        apply = ui.button("Apply").clicked();
    });

    if apply {
        let material_clone = preview_material.clone();

        let main_material_handle = main_cube_query.single();
        let _ = materials.set(main_material_handle, material_clone);
    }
}

fn color_picker_widget(ui: &mut egui::Ui, color: &mut Color) -> egui::Response {
    let [r, g, b, a] = color.as_rgba_f32();
    let mut egui_color: egui::Rgba = egui::Rgba::from_srgba_unmultiplied(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        (a * 255.0) as u8,
    );
    let res = egui::widgets::color_picker::color_edit_button_rgba(
        ui,
        &mut egui_color,
        egui::color_picker::Alpha::Opaque,
    );
    let [r, g, b, a] = egui_color.to_srgba_unmultiplied();
    *color = [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
    .into();
    res
}

// Rotates the cubes.
#[allow(clippy::type_complexity)]
fn rotator_system(
    time: Res<Time>,
    mut query: Query<&mut Transform, Or<(With<PreviewPassCube>, With<MainPassCube>)>>,
) {
    for mut transform in &mut query {
        transform.rotate_x(1.5 * time.delta_seconds());
        transform.rotate_z(1.3 * time.delta_seconds());
    }
}
