use bevy::{
    prelude::*,
    render2::{
        render_asset::RenderAssets, render_resource::BindGroup, renderer::RenderDevice,
        texture::Image,
    },
    utils::HashMap,
    window::WindowId,
};
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindingResource};

#[repr(C)]
#[derive(bytemuck::Zeroable, bytemuck::Pod, Clone, Copy)]
pub(crate) struct EguiTransform {
    scale: Vec2,
    translation: Vec2,
}
impl EguiTransform {
    fn new(window_size: WindowSize, egui_settings: &EguiSettings) -> Self {
        EguiTransform {
            scale: Vec2::new(
                2.0 / (window_size.width() / egui_settings.scale_factor as f32),
                -2.0 / (window_size.height() / egui_settings.scale_factor as f32),
            ),
            translation: Vec2::new(-1.0, 1.0),
        }
    }
}

use crate::{
    egui_node::EguiPipeline, EguiContext, EguiMainTextures, EguiSettings, EguiShapes, WindowSize,
};

pub(crate) struct ExtractedShapes(pub HashMap<WindowId, EguiShapes>);
pub(crate) struct ExtractedWindowSizes(pub HashMap<WindowId, (WindowSize, EguiTransform)>);
pub(crate) struct ExtractedEguiSettings(pub EguiSettings);
pub(crate) struct ExtractedEguiContext(pub HashMap<WindowId, egui::CtxRef>);

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) enum EguiTexture {
    Font(WindowId),
    User(u64),
}

pub(crate) struct ExtractedEguiTextures {
    pub(crate) main_textures: HashMap<WindowId, Handle<Image>>,
    pub(crate) user_textures: HashMap<u64, Handle<Image>>,
}
impl ExtractedEguiTextures {
    pub(crate) fn handles(&self) -> impl Iterator<Item = (EguiTexture, &Handle<Image>)> {
        self.main_textures
            .iter()
            .map(|(&window, handle)| (EguiTexture::Font(window), handle))
            .chain(
                self.user_textures
                    .iter()
                    .map(|(&id, handle)| (EguiTexture::User(id), handle)),
            )
    }
}

pub(crate) fn extract_egui_render_data(
    mut commands: Commands,
    mut shapes: ResMut<HashMap<WindowId, EguiShapes>>,
    window_sizes: ResMut<HashMap<WindowId, WindowSize>>,
    egui_settings: Res<EguiSettings>,
    egui_context: Res<EguiContext>,
) {
    let shapes = std::mem::take(&mut *shapes);
    commands.insert_resource(ExtractedShapes(shapes));
    commands.insert_resource(ExtractedEguiSettings(egui_settings.clone()));
    commands.insert_resource(ExtractedEguiContext(egui_context.ctx.clone()));
    commands.insert_resource(ExtractedWindowSizes(
        window_sizes
            .iter()
            .map(|(&id, &window_size)| {
                let transform = EguiTransform::new(window_size, &*egui_settings);
                (id, (window_size, transform))
            })
            .collect(),
    ));
}

pub(crate) fn extract_egui_textures(
    mut commands: Commands,
    egui_context: Res<EguiContext>,
    egui_main_textures: ResMut<EguiMainTextures>,
    _image_assets: ResMut<Assets<Image>>,
) {
    commands.insert_resource(ExtractedEguiTextures {
        main_textures: egui_main_textures
            .0
            .iter()
            .map(|(&window_id, (handle, _))| (window_id, handle.clone()))
            .collect(),
        user_textures: egui_context.egui_textures.clone(),
    });
}

pub(crate) struct EguiTextureBindGroups {
    pub(crate) bind_groups: HashMap<EguiTexture, BindGroup>,
}

pub(crate) fn queue_bind_groups(
    mut commands: Commands,
    egui_textures: Res<ExtractedEguiTextures>,
    render_device: Res<RenderDevice>,
    gpu_images: Res<RenderAssets<Image>>,
    egui_pipeline: Res<EguiPipeline>,
) {
    let bind_groups = egui_textures
        .handles()
        .filter_map(|(texture, handle)| {
            let gpu_image = gpu_images.get(handle)?;
            let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &egui_pipeline.texture_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&gpu_image.texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&gpu_image.sampler),
                    },
                ],
            });
            Some((texture, bind_group))
        })
        .collect();

    commands.insert_resource(EguiTextureBindGroups { bind_groups })
}
