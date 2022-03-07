use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{std140::AsStd140, BindGroup, BufferId, DynamicUniformVec},
        renderer::{RenderDevice, RenderQueue},
        texture::Image,
    },
    utils::HashMap,
    window::WindowId,
};
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindingResource};

use crate::{
    egui_node::EguiPipeline, EguiContext, EguiManagedTextures, EguiRenderOutput, EguiSettings,
    WindowSize,
};

pub(crate) struct ExtractedShapes(pub HashMap<WindowId, EguiRenderOutput>);
pub(crate) struct ExtractedWindowSizes(pub HashMap<WindowId, WindowSize>);
pub(crate) struct ExtractedEguiSettings(pub EguiSettings);
pub(crate) struct ExtractedEguiContext(pub HashMap<WindowId, egui::Context>);

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) enum EguiTexture {
    /// Textures allocated via egui.
    Managed(WindowId, u64),
    /// Textures allocated via bevy.
    User(u64),
}

pub(crate) struct ExtractedEguiTextures {
    pub(crate) egui_textures: HashMap<(WindowId, u64), Handle<Image>>,
    pub(crate) user_textures: HashMap<u64, Handle<Image>>,
}

impl ExtractedEguiTextures {
    pub(crate) fn handles(&self) -> impl Iterator<Item = (EguiTexture, &Handle<Image>)> {
        self.egui_textures
            .iter()
            .map(|(&(window, tex_id), handle)| (EguiTexture::Managed(window, tex_id), handle))
            .chain(
                self.user_textures
                    .iter()
                    .map(|(&id, handle)| (EguiTexture::User(id), handle)),
            )
    }
}

pub(crate) fn extract_egui_render_data(
    mut commands: Commands,
    mut shapes: ResMut<HashMap<WindowId, EguiRenderOutput>>,
    window_sizes: ResMut<HashMap<WindowId, WindowSize>>,
    egui_settings: Res<EguiSettings>,
    egui_context: Res<EguiContext>,
) {
    let shapes = std::mem::take(&mut *shapes);
    commands.insert_resource(ExtractedShapes(shapes));
    commands.insert_resource(ExtractedEguiSettings(egui_settings.clone()));
    commands.insert_resource(ExtractedEguiContext(egui_context.ctx.clone()));
    commands.insert_resource(ExtractedWindowSizes(window_sizes.clone()));
}

pub(crate) fn extract_egui_textures(
    mut commands: Commands,
    egui_context: Res<EguiContext>,
    egui_managed_textures: ResMut<EguiManagedTextures>,
    _image_assets: ResMut<Assets<Image>>,
) {
    commands.insert_resource(ExtractedEguiTextures {
        egui_textures: egui_managed_textures
            .0
            .iter()
            .map(|(&window_id, handle)| (window_id, handle.clone()))
            .collect(),
        user_textures: egui_context.user_textures.clone(),
    });
}

#[derive(Default)]
pub(crate) struct EguiTransforms {
    pub buffer: DynamicUniformVec<EguiTransform>,
    pub offsets: HashMap<WindowId, u32>,

    pub bind_group: Option<(BufferId, BindGroup)>,
}

#[derive(AsStd140)]
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

pub(crate) fn prepare_egui_transforms(
    mut egui_transforms: ResMut<EguiTransforms>,
    window_sizes: Res<ExtractedWindowSizes>,
    egui_settings: Res<ExtractedEguiSettings>,

    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,

    egui_pipeline: Res<EguiPipeline>,
) {
    egui_transforms.buffer.clear();
    egui_transforms.offsets.clear();

    for (window, size) in &window_sizes.0 {
        let offset = egui_transforms
            .buffer
            .push(EguiTransform::new(*size, &egui_settings.0));
        egui_transforms.offsets.insert(*window, offset);
    }

    egui_transforms
        .buffer
        .write_buffer(&render_device, &render_queue);

    if let Some(buffer) = egui_transforms.buffer.uniform_buffer() {
        match egui_transforms.bind_group {
            Some((id, _)) if buffer.id() == id => {}
            _ => {
                let transform_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
                    label: Some("egui transform bind group"),
                    layout: &egui_pipeline.transform_bind_group_layout,
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: egui_transforms.buffer.binding().unwrap(),
                    }],
                });
                egui_transforms.bind_group = Some((buffer.id(), transform_bind_group));
            }
        };
    }
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
