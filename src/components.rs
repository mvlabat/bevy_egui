//! Custom bevy components and resources

use std::cell::RefCell;
use std::cell::RefMut;

use arboard::Clipboard;
use bevy::log;
use bevy::utils::HashMap;
use bevy::{
    prelude::*,
    render::{
        render_resource::{AddressMode, SamplerDescriptor},
        texture::ImageSampler,
    },
};
use thread_local::ThreadLocal;

use crate::EguiManagedTexture;

/// A resource for storing global UI settings.
#[derive(Clone, Debug, Resource)]
pub struct EguiSettings {
    /// Global scale factor for Egui widgets (`1.0` by default).
    ///
    /// This setting can be used to force the UI to render in physical pixels regardless of DPI as follows:
    /// ```rust
    /// use bevy::{prelude::*, window::PrimaryWindow};
    /// use bevy_egui::EguiSettings;
    ///
    /// fn update_ui_scale_factor(mut egui_settings: ResMut<EguiSettings>, windows: Query<&Window, With<PrimaryWindow>>) {
    ///     if let Ok(window) = windows.get_single() {
    ///         egui_settings.scale_factor = 1.0 / window.scale_factor();
    ///     }
    /// }
    /// ```
    pub scale_factor: f64,
    /// Will be used as a default value for hyperlink [target](https://www.w3schools.com/tags/att_a_target.asp) hints.
    /// If not specified, `_self` will be used. Only matters in a web browser.
    #[cfg(feature = "open_url")]
    pub default_open_url_target: Option<String>,
    /// Used to change sampler properties
    /// Defaults to linear and clamped to edge
    pub sampler_descriptor: ImageSampler,
}

// Just to keep the PartialEq
impl PartialEq for EguiSettings {
    fn eq(&self, other: &Self) -> bool {
        let eq = self.scale_factor == other.scale_factor;
        #[cfg(feature = "open_url")]
        let eq = eq && self.default_open_url_target == other.default_open_url_target;
        eq && compare_descriptors(&self.sampler_descriptor, &other.sampler_descriptor)
    }
}

// Since Eq is not implemented for ImageSampler
fn compare_descriptors(a: &ImageSampler, b: &ImageSampler) -> bool {
    match (a, b) {
        (ImageSampler::Default, ImageSampler::Default) => true,
        (ImageSampler::Descriptor(descriptor_a), ImageSampler::Descriptor(descriptor_b)) => {
            descriptor_a == descriptor_b
        }
        _ => false,
    }
}

impl Default for EguiSettings {
    fn default() -> Self {
        Self {
            scale_factor: 1.0,
            #[cfg(feature = "open_url")]
            default_open_url_target: None,
            sampler_descriptor: ImageSampler::Descriptor(SamplerDescriptor {
                address_mode_u: AddressMode::ClampToEdge,
                address_mode_v: AddressMode::ClampToEdge,
                ..ImageSampler::linear_descriptor()
            }),
        }
    }
}

impl EguiSettings {
    /// Use nearest descriptor instead of linear.
    pub fn use_nearest_descriptor(&mut self) {
        self.sampler_descriptor = ImageSampler::Descriptor(SamplerDescriptor {
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            ..ImageSampler::nearest_descriptor()
        })
    }
    /// Use default image sampler, derived from the [`ImagePlugin`](bevy::render::texture::ImagePlugin) setup.
    pub fn use_bevy_descriptor(&mut self) {
        self.sampler_descriptor = ImageSampler::Default;
    }
}

/// A resource for accessing clipboard.
///
/// The resource is available only if `manage_clipboard` feature is enabled.
#[cfg(feature = "manage_clipboard")]
#[derive(Default, Resource)]
pub struct EguiClipboard {
    #[cfg(not(target_arch = "wasm32"))]
    clipboard: ThreadLocal<Option<RefCell<Clipboard>>>,
    #[cfg(target_arch = "wasm32")]
    clipboard: String,
}

#[cfg(feature = "manage_clipboard")]
impl EguiClipboard {
    /// Sets clipboard contents.
    pub fn set_contents(&mut self, contents: &str) {
        self.set_contents_impl(contents);
    }

    /// Gets clipboard contents. Returns [`None`] if clipboard provider is unavailable or returns an error.
    #[must_use]
    pub fn get_contents(&self) -> Option<String> {
        self.get_contents_impl()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn set_contents_impl(&self, contents: &str) {
        if let Some(mut clipboard) = self.get() {
            if let Err(err) = clipboard.set_text(contents.to_owned()) {
                log::error!("Failed to set clipboard contents: {:?}", err);
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn set_contents_impl(&mut self, contents: &str) {
        self.clipboard = contents.to_owned();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn get_contents_impl(&self) -> Option<String> {
        if let Some(mut clipboard) = self.get() {
            match clipboard.get_text() {
                Ok(contents) => return Some(contents),
                Err(err) => log::info!("Failed to get clipboard contents: {:?}", err),
            }
        };
        None
    }

    #[cfg(target_arch = "wasm32")]
    #[allow(clippy::unnecessary_wraps)]
    fn get_contents_impl(&self) -> Option<String> {
        Some(self.clipboard.clone())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn get(&self) -> Option<RefMut<Clipboard>> {
        self.clipboard
            .get_or(|| {
                Clipboard::new()
                    .map(RefCell::new)
                    .map_err(|err| {
                        log::info!("Failed to initialize clipboard: {:?}", err);
                    })
                    .ok()
            })
            .as_ref()
            .map(|cell| cell.borrow_mut())
    }
}

/// Is used for storing Egui context input.
///
/// It gets reset during the [`EguiSet::ProcessInput`] system.
#[derive(Component, Clone, Debug, Default, Deref, DerefMut)]
pub struct EguiInput(pub egui::RawInput);

/// Is used for storing Egui shapes and textures delta.
#[derive(Component, Clone, Default, Debug, Resource)]
pub struct EguiRenderOutput {
    /// Pairs of rectangles and paint commands.
    ///
    /// The field gets populated during the [`EguiSet::ProcessOutput`] system (belonging to bevy's [`PostUpdate`]) and reset during `EguiNode::update`.
    pub paint_jobs: Vec<egui::ClippedPrimitive>,

    /// The change in egui textures since last frame.
    pub textures_delta: egui::TexturesDelta,
}

/// Is used for storing Egui output.
#[derive(Component, Clone, Default)]
pub struct EguiOutput {
    /// The field gets updated during the [`EguiSet::ProcessOutput`] system (belonging to [`PostUpdate`]).
    pub platform_output: egui::PlatformOutput,
}

/// A component for storing `bevy_egui` context.
#[derive(Clone, Component, Default)]
pub struct EguiContext(egui::Context);

impl EguiContext {
    /// Borrows the underlying Egui context immutably.
    ///
    /// Even though the mutable borrow isn't necessary, as the context is wrapped into `RwLock`,
    /// using the immutable getter is gated with the `immutable_ctx` feature. Using the immutable
    /// borrow is discouraged as it may cause unpredictable blocking in UI systems.
    ///
    /// When the context is queried with `&mut EguiContext`, the Bevy scheduler is able to make
    /// sure that the context isn't accessed concurrently and can perform other useful work
    /// instead of busy-waiting.
    #[cfg(feature = "immutable_ctx")]
    #[must_use]
    pub fn get(&self) -> &egui::Context {
        &self.0
    }

    /// Borrows the underlying Egui context mutably.
    ///
    /// Even though the mutable borrow isn't necessary, as the context is wrapped into `RwLock`,
    /// using the immutable getter is gated with the `immutable_ctx` feature. Using the immutable
    /// borrow is discouraged as it may cause unpredictable blocking in UI systems.
    ///
    /// When the context is queried with `&mut EguiContext`, the Bevy scheduler is able to make
    /// sure that the context isn't accessed concurrently and can perform other useful work
    /// instead of busy-waiting.
    #[must_use]
    pub fn get_mut(&mut self) -> &mut egui::Context {
        &mut self.0
    }
}

/// A resource for storing `bevy_egui` mouse position.
#[derive(Resource, Component, Default, Deref, DerefMut)]
pub struct EguiMousePosition(pub Option<(Entity, egui::Vec2)>);

/// A resource for storing `bevy_egui` user textures.
#[derive(Clone, Resource, Default)]
pub struct EguiUserTextures {
    pub(crate) textures: HashMap<Handle<Image>, u64>,
    pub(crate) last_texture_id: u64,
}

impl EguiUserTextures {
    /// Can accept either a strong or a weak handle.
    ///
    /// You may want to pass a weak handle if you control removing texture assets in your
    /// application manually and you don't want to bother with cleaning up textures in Egui.
    ///
    /// You'll want to pass a strong handle if a texture is used only in Egui and there are no
    /// handle copies stored anywhere else.
    pub fn add_image(&mut self, image: Handle<Image>) -> egui::TextureId {
        let id = *self.textures.entry(image.clone()).or_insert_with(|| {
            let id = self.last_texture_id;
            log::debug!("Add a new image (id: {}, handle: {:?})", id, image);
            self.last_texture_id += 1;
            id
        });
        egui::TextureId::User(id)
    }

    /// Removes the image handle and an Egui texture id associated with it.
    pub fn remove_image(&mut self, image: &Handle<Image>) -> Option<egui::TextureId> {
        let id = self.textures.remove(image);
        log::debug!("Remove image (id: {:?}, handle: {:?})", id, image);
        id.map(egui::TextureId::User)
    }

    /// Returns an associated Egui texture id.
    #[must_use]
    pub fn image_id(&self, image: &Handle<Image>) -> Option<egui::TextureId> {
        self.textures
            .get(image)
            .map(|&id| egui::TextureId::User(id))
    }
}

/// Stores physical size and scale factor, is used as a helper to calculate logical size.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct WindowSize {
    pub(crate) physical_width: f32,
    pub(crate) physical_height: f32,
    pub(crate) scale_factor: f32,
}

impl WindowSize {
    pub(crate) fn new(physical_width: f32, physical_height: f32, scale_factor: f32) -> Self {
        Self {
            physical_width,
            physical_height,
            scale_factor,
        }
    }

    #[inline]
    pub(crate) fn width(&self) -> f32 {
        self.physical_width / self.scale_factor
    }

    #[inline]
    pub(crate) fn height(&self) -> f32 {
        self.physical_height / self.scale_factor
    }
}

/// Contains textures allocated and painted by Egui.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct EguiManagedTextures(pub HashMap<(Entity, u64), EguiManagedTexture>);
