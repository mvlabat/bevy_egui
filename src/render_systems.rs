use crate::{
    egui_node::{EguiPipeline, EguiPipelineKey},
    EguiContext, EguiManagedTextures, EguiRenderOutput, EguiRenderOutputContainer, EguiSettings,
    EguiWindowSizeContainer, WindowSize,
};
use bevy::{
    asset::HandleId,
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, BufferId,
            CachedRenderPipelineId, DynamicUniformBuffer, PipelineCache, ShaderType,
            SpecializedRenderPipelines,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::Image,
        view::ExtractedWindows,
        Extract,
    },
    utils::HashMap,
    window::WindowId,
};

/// Extracted Egui render output.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct ExtractedRenderOutput(pub HashMap<WindowId, EguiRenderOutput>);

/// Extracted window sizes.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct ExtractedWindowSizes(pub HashMap<WindowId, WindowSize>);

/// Extracted Egui settings.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct ExtractedEguiSettings(pub EguiSettings);

/// Extracted Egui contexts.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct ExtractedEguiContext(pub HashMap<WindowId, egui::Context>);

/// Corresponds to Egui's [`egui::TextureId`].
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum EguiTextureId {
    /// Textures allocated via Egui.
    Managed(WindowId, u64),
    /// Textures allocated via Bevy.
    User(u64),
}

/// Extracted Egui textures.
#[derive(Resource, Default)]
pub struct ExtractedEguiTextures {
    /// Maps Egui managed texture ids to Bevy image handles.
    pub egui_textures: HashMap<(WindowId, u64), Handle<Image>>,
    /// Maps Bevy managed texture handles to Egui user texture ids.
    pub user_textures: HashMap<Handle<Image>, u64>,
}

impl ExtractedEguiTextures {
    /// Returns an iterator over all textures (both Egui and Bevy managed).
    pub fn handles(&self) -> impl Iterator<Item = (EguiTextureId, HandleId)> + '_ {
        self.egui_textures
            .iter()
            .map(|(&(window, texture_id), handle)| {
                (EguiTextureId::Managed(window, texture_id), handle.id())
            })
            .chain(
                self.user_textures
                    .iter()
                    .map(|(handle, id)| (EguiTextureId::User(*id), handle.id())),
            )
    }
}

/// Extracts Egui context, render output, settings and application window sizes.
pub fn extract_egui_render_data_system(
    mut commands: Commands,
    egui_render_output: Extract<Res<EguiRenderOutputContainer>>,
    window_sizes: Extract<Res<EguiWindowSizeContainer>>,
    egui_settings: Extract<Res<EguiSettings>>,
    egui_context: Extract<Res<EguiContext>>,
) {
    commands.insert_resource(ExtractedRenderOutput(egui_render_output.clone()));
    commands.insert_resource(ExtractedEguiSettings(egui_settings.clone()));
    commands.insert_resource(ExtractedEguiContext(egui_context.ctx.clone()));
    commands.insert_resource(ExtractedWindowSizes(window_sizes.clone()));
}

/// Extracts Egui textures.
pub fn extract_egui_textures_system(
    mut commands: Commands,
    egui_context: Extract<Res<EguiContext>>,
    egui_managed_textures: Extract<Res<EguiManagedTextures>>,
) {
    commands.insert_resource(ExtractedEguiTextures {
        egui_textures: egui_managed_textures
            .iter()
            .map(|(&(window_id, texture_id), managed_texture)| {
                ((window_id, texture_id), managed_texture.handle.clone())
            })
            .collect(),
        user_textures: egui_context.user_textures.clone(),
    });
}

/// Describes the transform buffer.
#[derive(Resource, Default)]
pub struct EguiTransforms {
    /// Uniform buffer.
    pub buffer: DynamicUniformBuffer<EguiTransform>,
    /// Offsets for each window.
    pub offsets: HashMap<WindowId, u32>,
    /// Bind group.
    pub bind_group: Option<(BufferId, BindGroup)>,
}

/// Scale and translation for rendering Egui shapes. Is needed to transform Egui coordinates from
/// the screen space with the center at (0, 0) to the normalised viewport space.
#[derive(ShaderType, Default)]
pub struct EguiTransform {
    /// Is affected by window size and [`EguiSettings::scale_factor`].
    pub scale: Vec2,
    /// Normally equals `Vec2::new(-1.0, 1.0)`.
    pub translation: Vec2,
}

impl EguiTransform {
    /// Calculates the transform from window size and scale factor.
    pub fn from_window_size(window_size: WindowSize, scale_factor: f32) -> Self {
        EguiTransform {
            scale: Vec2::new(
                2.0 / (window_size.width() / scale_factor),
                -2.0 / (window_size.height() / scale_factor),
            ),
            translation: Vec2::new(-1.0, 1.0),
        }
    }
}

/// Prepares Egui transforms.
pub fn prepare_egui_transforms_system(
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
        let offset = egui_transforms.buffer.push(EguiTransform::from_window_size(
            *size,
            egui_settings.scale_factor as f32,
        ));
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

/// Maps Egui textures to bind groups.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct EguiTextureBindGroups(pub HashMap<EguiTextureId, BindGroup>);

/// Queues bind groups.
pub fn queue_bind_groups_system(
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

    commands.insert_resource(EguiTextureBindGroups(bind_groups))
}

/// Cached Pipeline IDs for the specialized `EguiPipeline`s
#[derive(Resource)]
pub struct EguiPipelines(pub HashMap<WindowId, CachedRenderPipelineId>);

/// Queue [`EguiPipeline`]s specialized on each window's swap chain texture format.
pub fn queue_pipelines_system(
    mut commands: Commands,
    mut pipeline_cache: ResMut<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<EguiPipeline>>,
    egui_pipeline: Res<EguiPipeline>,
    windows: Res<ExtractedWindows>,
) {
    let pipelines = windows
        .iter()
        .filter_map(|(window_id, window)| {
            let key = EguiPipelineKey {
                texture_format: window.swap_chain_texture_format?,
            };
            let pipeline_id = pipelines.specialize(&mut pipeline_cache, &egui_pipeline, key);

            Some((*window_id, pipeline_id))
        })
        .collect();

    commands.insert_resource(EguiPipelines(pipelines));
}
