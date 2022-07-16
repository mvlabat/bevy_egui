use crate::{
    egui_node::EguiPipeline, EguiContext, EguiManagedTextures, EguiRenderOutput, EguiSettings,
    WindowSize,
};
use bevy::{
    asset::HandleId,
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, BufferId,
            DynamicUniformBuffer, ShaderType,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::Image,
        Extract,
    },
    utils::HashMap,
    window::WindowId,
};

pub(crate) struct ExtractedRenderOutput(pub HashMap<WindowId, EguiRenderOutput>);
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
    pub(crate) user_textures: HashMap<HandleId, u64>,
}

impl ExtractedEguiTextures {
    pub(crate) fn handles(&self) -> impl Iterator<Item = (EguiTexture, HandleId)> + '_ {
        self.egui_textures
            .iter()
            .map(|(&(window, texture_id), handle)| {
                (EguiTexture::Managed(window, texture_id), handle.id)
            })
            .chain(
                self.user_textures
                    .iter()
                    .map(|(handle, id)| (EguiTexture::User(*id), *handle)),
            )
    }
}

pub(crate) fn extract_egui_render_data(
    mut commands: Commands,
    egui_render_output: Extract<Res<HashMap<WindowId, EguiRenderOutput>>>,
    window_sizes: Extract<Res<HashMap<WindowId, WindowSize>>>,
    egui_settings: Extract<Res<EguiSettings>>,
    egui_context: Extract<Res<EguiContext>>,
) {
    commands.insert_resource(ExtractedRenderOutput(egui_render_output.clone()));
    commands.insert_resource(ExtractedEguiSettings(egui_settings.clone()));
    commands.insert_resource(ExtractedEguiContext(egui_context.ctx.clone()));
    commands.insert_resource(ExtractedWindowSizes(window_sizes.clone()));
}

pub(crate) fn extract_egui_textures(
    mut commands: Commands,
    egui_context: Extract<Res<EguiContext>>,
    egui_managed_textures: Extract<Res<EguiManagedTextures>>,
) {
    commands.insert_resource(ExtractedEguiTextures {
        egui_textures: egui_managed_textures
            .0
            .iter()
            .map(|(&(window_id, texture_id), managed_texture)| {
                ((window_id, texture_id), managed_texture.handle.clone())
            })
            .collect(),
        user_textures: egui_context.user_textures.clone(),
    });
}

#[derive(Default)]
pub(crate) struct EguiTransforms {
    pub buffer: DynamicUniformBuffer<EguiTransform>,
    pub offsets: HashMap<WindowId, u32>,
    pub bind_group: Option<(BufferId, BindGroup)>,
}

#[derive(ShaderType, Default)]
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

    if let Some(buffer) = egui_transforms.buffer.buffer() {
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
        .filter_map(|(texture, handle_id)| {
            let gpu_image = gpu_images.get(&Handle::weak(handle_id))?;
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
