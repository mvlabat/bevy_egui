use crate::{
    egui_node::{EguiNode, EguiPipeline, EguiPipelineKey},
    EguiManagedTextures, EguiSettings, EguiUserTextures, WindowSize,
};
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_asset::RenderAssets,
        render_graph::{RenderGraph, RenderLabel},
        render_resource::{
            BindGroup, BindGroupEntry, BindingResource, BufferId, CachedRenderPipelineId,
            DynamicUniformBuffer, PipelineCache, ShaderType, SpecializedRenderPipelines,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
        view::ExtractedWindows,
        Extract,
    },
    utils::HashMap,
};

/// Extracted Egui settings.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct ExtractedEguiSettings(pub EguiSettings);

/// The extracted version of [`EguiManagedTextures`].
#[derive(Debug, Resource)]
pub struct ExtractedEguiManagedTextures(pub HashMap<(Entity, u64), Handle<Image>>);
impl ExtractResource for ExtractedEguiManagedTextures {
    type Source = EguiManagedTextures;

    fn extract_resource(source: &Self::Source) -> Self {
        Self(source.iter().map(|(k, v)| (*k, v.handle.clone())).collect())
    }
}

/// Corresponds to Egui's [`egui::TextureId`].
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum EguiTextureId {
    /// Textures allocated via Egui.
    Managed(Entity, u64),
    /// Textures allocated via Bevy.
    User(u64),
}

/// Extracted Egui textures.
#[derive(SystemParam)]
pub struct ExtractedEguiTextures<'w> {
    /// Maps Egui managed texture ids to Bevy image handles.
    pub egui_textures: Res<'w, ExtractedEguiManagedTextures>,
    /// Maps Bevy managed texture handles to Egui user texture ids.
    pub user_textures: Res<'w, EguiUserTextures>,
}

/// [`RenderLabel`] type for the Egui pass.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct EguiPass {
    /// Index of the window entity.
    pub window_index: u32,
    /// Generation of the window entity.
    pub window_generation: u32,
}

impl ExtractedEguiTextures<'_> {
    /// Returns an iterator over all textures (both Egui and Bevy managed).
    pub fn handles(&self) -> impl Iterator<Item = (EguiTextureId, AssetId<Image>)> + '_ {
        self.egui_textures
            .0
            .iter()
            .map(|(&(window, texture_id), managed_tex)| {
                (EguiTextureId::Managed(window, texture_id), managed_tex.id())
            })
            .chain(
                self.user_textures
                    .textures
                    .iter()
                    .map(|(handle, id)| (EguiTextureId::User(*id), handle.id())),
            )
    }
}

/// Sets up the pipeline for newly created windows.
pub fn setup_new_windows_render_system(
    windows: Extract<Query<Entity, Added<Window>>>,
    mut render_graph: ResMut<RenderGraph>,
) {
    for window in windows.iter() {
        let egui_pass = EguiPass {
            window_index: window.index(),
            window_generation: window.generation(),
        };

        let new_node = EguiNode::new(window);

        render_graph.add_node(egui_pass.clone(), new_node);

        render_graph.add_node_edge(bevy::render::graph::CameraDriverLabel, egui_pass);
    }
}

/// Describes the transform buffer.
#[derive(Resource, Default)]
pub struct EguiTransforms {
    /// Uniform buffer.
    pub buffer: DynamicUniformBuffer<EguiTransform>,
    /// Offsets for each window.
    pub offsets: HashMap<Entity, u32>,
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
    window_sizes: Query<(Entity, &WindowSize)>,
    egui_settings: Res<EguiSettings>,

    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,

    egui_pipeline: Res<EguiPipeline>,
) {
    egui_transforms.buffer.clear();
    egui_transforms.offsets.clear();

    for (window, size) in window_sizes.iter() {
        let offset = egui_transforms
            .buffer
            .push(&EguiTransform::from_window_size(
                *size,
                egui_settings.scale_factor,
            ));
        egui_transforms.offsets.insert(window, offset);
    }

    egui_transforms
        .buffer
        .write_buffer(&render_device, &render_queue);

    if let Some(buffer) = egui_transforms.buffer.buffer() {
        match egui_transforms.bind_group {
            Some((id, _)) if buffer.id() == id => {}
            _ => {
                let transform_bind_group = render_device.create_bind_group(
                    Some("egui transform bind group"),
                    &egui_pipeline.transform_bind_group_layout,
                    &[BindGroupEntry {
                        binding: 0,
                        resource: egui_transforms.buffer.binding().unwrap(),
                    }],
                );
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
    egui_textures: ExtractedEguiTextures,
    render_device: Res<RenderDevice>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    egui_pipeline: Res<EguiPipeline>,
) {
    let bind_groups = egui_textures
        .handles()
        .filter_map(|(texture, handle_id)| {
            let gpu_image = gpu_images.get(&Handle::Weak(handle_id))?;
            let bind_group = render_device.create_bind_group(
                None,
                &egui_pipeline.texture_bind_group_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&gpu_image.texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&gpu_image.sampler),
                    },
                ],
            );
            Some((texture, bind_group))
        })
        .collect();

    commands.insert_resource(EguiTextureBindGroups(bind_groups))
}

/// Cached Pipeline IDs for the specialized `EguiPipeline`s
#[derive(Resource)]
pub struct EguiPipelines(pub HashMap<Entity, CachedRenderPipelineId>);

/// Queue [`EguiPipeline`]s specialized on each window's swap chain texture format.
pub fn queue_pipelines_system(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<EguiPipeline>>,
    egui_pipeline: Res<EguiPipeline>,
    windows: Res<ExtractedWindows>,
) {
    let pipelines = windows
        .iter()
        .filter_map(|(window_id, window)| {
            let key = EguiPipelineKey {
                texture_format: window.swap_chain_texture_format?.add_srgb_suffix(),
            };
            let pipeline_id = pipelines.specialize(&pipeline_cache, &egui_pipeline, key);

            Some((*window_id, pipeline_id))
        })
        .collect();

    commands.insert_resource(EguiPipelines(pipelines));
}
