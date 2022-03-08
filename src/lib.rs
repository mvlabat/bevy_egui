#![deny(missing_docs)]

//! This crate provides a [Egui](https://github.com/emilk/egui) integration for the [Bevy](https://github.com/bevyengine/bevy) game engine.
//!
//! **Trying out:**
//!
//! An example WASM project is live at [mvlabat.github.io/bevy_egui_web_showcase](https://mvlabat.github.io/bevy_egui_web_showcase/index.html) [[source](https://github.com/mvlabat/bevy_egui_web_showcase)].
//!
//! **Features:**
//! - Desktop and web platforms support
//! - Clipboard (web support is limited to the same window, see [rust-windowing/winit#1829](https://github.com/rust-windowing/winit/issues/1829))
//! - Opening URLs
//! - Multiple windows support (see [./examples/two_windows.rs](./examples/two_windows.rs))
//!
//! `bevy_egui` can be compiled with using only `bevy` and `egui` as dependencies: `manage_clipboard` and `open_url` features,
//! that require additional crates, can be disabled.
//!
//! ## Usage
//!
//! Here's a minimal usage example:
//!
//! ```no_run,rust
//! use bevy::prelude::*;
//! use bevy_egui::{egui, EguiContext, EguiPlugin};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugin(EguiPlugin)
//!         // Systems that create Egui widgets should be run during the `CoreStage::Update` stage,
//!         // or after the `EguiSystem::BeginFrame` system (which belongs to the `CoreStage::PreUpdate` stage).
//!         .add_system(ui_example)
//!         .run();
//! }
//!
//! fn ui_example(mut egui_context: ResMut<EguiContext>) {
//!     egui::Window::new("Hello").show(egui_context.ctx_mut(), |ui| {
//!         ui.label("world");
//!     });
//! }
//! ```
//!
//! For a more advanced example, see [examples/ui.rs](examples/ui.rs).
//!
//! ```bash
//! cargo run --example ui
//! ```
//!
//! ## See also
//!
//! - [`bevy-inspector-egui`](https://github.com/jakobhellermann/bevy-inspector-egui)

pub use egui;

mod egui_node;
mod render_systems;
mod systems;

use egui_node::EguiNode;
use render_systems::EguiTransforms;

use crate::systems::*;
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use arboard::Clipboard;
use bevy::{
    app::{App, CoreStage, Plugin, StartupStage},
    asset::{AssetEvent, Assets, Handle, HandleId},
    ecs::{
        event::EventReader,
        schedule::{ParallelSystemDescriptorCoercion, SystemLabel},
        system::ResMut,
    },
    input::InputSystem,
    log,
    render::{render_graph::RenderGraph, texture::Image, RenderApp, RenderStage},
    utils::HashMap,
    window::WindowId,
};
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use std::cell::{RefCell, RefMut};
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use thread_local::ThreadLocal;

/// Adds all Egui resources and render graph nodes.
pub struct EguiPlugin;

/// A resource for storing global UI settings.
#[derive(Clone, Debug, PartialEq)]
pub struct EguiSettings {
    /// Global scale factor for egui widgets (`1.0` by default).
    ///
    /// This setting can be used to force the UI to render in physical pixels regardless of DPI as follows:
    /// ```rust
    /// use bevy::prelude::*;
    /// use bevy_egui::EguiSettings;
    ///
    /// fn update_ui_scale_factor(mut egui_settings: ResMut<EguiSettings>, windows: Res<Windows>) {
    ///     if let Some(window) = windows.get_primary() {
    ///         egui_settings.scale_factor = 1.0 / window.scale_factor();
    ///     }
    /// }
    /// ```
    pub scale_factor: f64,
}

impl Default for EguiSettings {
    fn default() -> Self {
        Self { scale_factor: 1.0 }
    }
}

/// Is used for storing the input passed to Egui. The actual resource is `HashMap<WindowId, EguiInput>`.
///
/// It gets reset during the [`EguiSystem::ProcessInput`] system.
#[derive(Clone, Debug, Default)]
pub struct EguiInput {
    /// Egui's raw input.
    pub raw_input: egui::RawInput,
}

/// A resource for accessing clipboard.
///
/// The resource is available only if `manage_clipboard` feature is enabled.
#[cfg(feature = "manage_clipboard")]
#[derive(Default)]
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

/// Is used for storing Egui shapes. The actual resource is `HashMap<WindowId, EguiShapes>`.
#[derive(Clone, Default, Debug)]
pub struct EguiRenderOutput {
    /// Pairs of rectangles and paint commands.
    ///
    /// The field gets populated during the [`EguiSystem::ProcessOutput`] system in the [`CoreStage::PostUpdate`] and reset during `EguiNode::update`.
    pub shapes: Vec<egui::epaint::ClippedShape>,

    /// The change in egui textures since last frame.
    pub textures_delta: egui::TexturesDelta,
}

/// Is used for storing Egui output. The actual resource is `HashMap<WindowId, EguiOutput>`.
#[derive(Clone, Default)]
pub struct EguiOutput {
    /// The field gets updated during the [`EguiSystem::ProcessOutput`] system in the [`CoreStage::PostUpdate`].
    pub platform_output: egui::PlatformOutput,
}

/// A resource for storing `bevy_egui` context.
#[derive(Clone)]
pub struct EguiContext {
    ctx: HashMap<WindowId, egui::Context>,
    user_textures: HashMap<HandleId, u64>,
    last_texture_id: u64,
    mouse_position: Option<(WindowId, egui::Vec2)>,
}

impl EguiContext {
    fn new() -> Self {
        Self {
            ctx: HashMap::default(),
            user_textures: Default::default(),
            last_texture_id: 0,
            mouse_position: None,
        }
    }

    /// Egui context of the primary window.
    ///
    /// This function is only available when the `multi_threaded` feature is enabled.
    /// The preferable way is to use `ctx_mut` to avoid unpredictable blocking inside UI systems.
    #[cfg(feature = "multi_threaded")]
    #[track_caller]
    pub fn ctx(&self) -> &egui::Context {
        self.ctx.get(&WindowId::primary()).expect("`EguiContext::ctx` was called for an uninitialized context (primary window), consider moving your startup system to the `StartupStage::Startup` stage or run it after the `EguiStartupSystem::InitContexts` system")
    }

    /// Egui context for a specific window.
    /// If you want to display UI on a non-primary window, make sure to set up the render graph by
    /// calling [`setup_pipeline`].
    ///
    /// This function is only available when the `multi_threaded` feature is enabled.
    /// The preferable way is to use `ctx_for_window_mut` to avoid unpredictable blocking inside UI
    /// systems.
    #[cfg(feature = "multi_threaded")]
    #[track_caller]
    pub fn ctx_for_window(&self, window: WindowId) -> &egui::Context {
        self.ctx
            .get(&window)
            .unwrap_or_else(|| panic!("`EguiContext::ctx_for_window` was called for an uninitialized context (window {}), consider moving your UI system to the `CoreStage::Update` stage or run it after the `EguiSystem::BeginFrame` system (`StartupStage::Startup` or `EguiStartupSystem::InitContexts` for startup systems respectively)", window))
    }

    /// Fallible variant of [`EguiContext::ctx_for_window`]. Make sure to set up the render graph by
    /// calling [`setup_pipeline`].
    ///
    /// This function is only available when the `multi_threaded` feature is enabled.
    /// The preferable way is to use `try_ctx_for_window_mut` to avoid unpredictable blocking inside
    /// UI systems.
    #[cfg(feature = "multi_threaded")]
    pub fn try_ctx_for_window(&self, window: WindowId) -> Option<&egui::Context> {
        self.ctx.get(&window)
    }

    /// Egui context of the primary window.
    #[track_caller]
    pub fn ctx_mut(&mut self) -> &egui::Context {
        self.ctx.get(&WindowId::primary()).expect("`EguiContext::ctx_mut` was called for an uninitialized context (primary window), consider moving your startup system to the `StartupStage::Startup` stage or run it after the `EguiStartupSystem::InitContexts` system")
    }

    /// Egui context for a specific window.
    /// If you want to display UI on a non-primary window, make sure to set up the render graph by
    /// calling [`setup_pipeline`].
    #[track_caller]
    pub fn ctx_for_window_mut(&mut self, window: WindowId) -> &egui::Context {
        self.ctx
            .get(&window)
            .unwrap_or_else(|| panic!("`EguiContext::ctx_for_window_mut` was called for an uninitialized context (window {}), consider moving your UI system to the `CoreStage::Update` stage or run it after the `EguiSystem::BeginFrame` system (`StartupStage::Startup` or `EguiStartupSystem::InitContexts` for startup systems respectively)", window))
    }

    /// Fallible variant of [`EguiContext::ctx_for_window_mut`]. Make sure to set up the render
    /// graph by calling [`setup_pipeline`].
    pub fn try_ctx_for_window_mut(&mut self, window: WindowId) -> Option<&egui::Context> {
        self.ctx.get(&window)
    }

    /// Allows to get multiple contexts at the same time. This function is useful when you want
    /// to get multiple window contexts without using the `multi_threaded` feature.
    ///
    /// # Panics
    ///
    /// Panics if the passed window ids aren't unique.
    pub fn ctx_for_windows_mut<const N: usize>(
        &mut self,
        ids: [WindowId; N],
    ) -> [&egui::Context; N] {
        let mut unique_ids = bevy::utils::HashSet::default();
        assert!(
            ids.iter().all(move |id| unique_ids.insert(id)),
            "Window ids passed to `EguiContext::ctx_for_windows_mut` must be unique: {:?}",
            ids
        );
        ids.map(|id| self.ctx.get(&id).unwrap_or_else(|| panic!("`EguiContext::ctx_for_windows_mut` was called for an uninitialized context (window {}), consider moving your UI system to the `CoreStage::Update` stage or run it after the `EguiSystem::BeginFrame` system (`StartupStage::Startup` or `EguiStartupSystem::InitContexts` for startup systems respectively)", id)))
    }

    /// Fallible variant of [`EguiContext::ctx_for_windows_mut`]. Make sure to set up the render
    /// graph by calling [`setup_pipeline`].
    ///
    /// # Panics
    ///
    /// Panics if the passed window ids aren't unique.
    pub fn try_ctx_for_windows_mut<const N: usize>(
        &mut self,
        ids: [WindowId; N],
    ) -> [Option<&egui::Context>; N] {
        let mut unique_ids = bevy::utils::HashSet::default();
        assert!(
            ids.iter().all(move |id| unique_ids.insert(id)),
            "Window ids passed to `EguiContext::ctx_for_windows_mut` must be unique: {:?}",
            ids
        );
        ids.map(|id| self.ctx.get(&id))
    }

    /// Can accept either a strong or a weak handle.
    ///
    /// You may want to pass a weak handle if you control removing texture assets in your
    /// application manually and you don't want to bother with cleaning up textures in Egui.
    ///
    /// You'll want to pass a strong handle if a texture is used only in Egui and there's no
    /// handle copies stored anywhere else.
    pub fn add_image(&mut self, image: Handle<Image>) -> egui::TextureId {
        let id = *self.user_textures.entry(image.id).or_insert_with(|| {
            let id = self.last_texture_id;
            log::debug!("Add a new image (id: {}, handle: {:?})", id, image);
            self.last_texture_id += 1;
            id
        });
        egui::TextureId::User(id)
    }

    /// Removes the image handle and an Egui texture id associated with it.
    pub fn remove_image(&mut self, image: &Handle<Image>) -> Option<u64> {
        let id = self.user_textures.remove(&image.id);
        log::debug!("Remove image (id: {:?}, handle: {:?})", id, image);
        id
    }

    /// Returns associated Egui texture id.
    #[must_use]
    pub fn image_id(&self, image: &Handle<Image>) -> Option<egui::TextureId> {
        self.user_textures
            .get(&image.id)
            .map(|&id| egui::TextureId::User(id))
    }
}

#[doc(hidden)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct WindowSize {
    physical_width: f32,
    physical_height: f32,
    scale_factor: f32,
}

impl WindowSize {
    fn new(physical_width: f32, physical_height: f32, scale_factor: f32) -> Self {
        Self {
            physical_width,
            physical_height,
            scale_factor,
        }
    }

    #[inline]
    fn width(&self) -> f32 {
        self.physical_width / self.scale_factor
    }

    #[inline]
    fn height(&self) -> f32 {
        self.physical_height / self.scale_factor
    }
}

/// The names of `bevy_egui` nodes.
pub mod node {
    /// The main egui pass.
    pub const EGUI_PASS: &str = "egui_pass";
}

#[derive(SystemLabel, Clone, Hash, Debug, Eq, PartialEq)]
/// The names of `bevy_egui` startup systems.
pub enum EguiStartupSystem {
    /// Initializes Egui contexts for available windows.
    InitContexts,
}

/// The names of egui systems.
#[derive(SystemLabel, Clone, Hash, Debug, Eq, PartialEq)]
pub enum EguiSystem {
    /// Reads Egui inputs (keyboard, mouse, etc) and writes them into the [`EguiInput`] resource.
    ///
    /// To modify the input, you can hook your system like this:
    ///
    /// `system.after(EguiSystem::ProcessInput).before(EguiSystem::BeginFrame)`.
    ProcessInput,
    /// Begins the `egui` frame
    BeginFrame,
    /// Processes the [`EguiOutput`] resource
    ProcessOutput,
}

impl Plugin for EguiPlugin {
    fn build(&self, app: &mut App) {
        let world = &mut app.world;
        world.insert_resource(EguiSettings::default());
        world.insert_resource(HashMap::<WindowId, EguiInput>::default());
        world.insert_resource(HashMap::<WindowId, EguiOutput>::default());
        world.insert_resource(HashMap::<WindowId, WindowSize>::default());
        world.insert_resource(HashMap::<WindowId, EguiRenderOutput>::default());
        world.insert_resource(EguiManagedTextures::default());
        #[cfg(feature = "manage_clipboard")]
        world.insert_resource(EguiClipboard::default());
        world.insert_resource(EguiContext::new());

        app.add_startup_system_to_stage(
            StartupStage::PreStartup,
            init_contexts_on_startup.label(EguiStartupSystem::InitContexts),
        );

        app.add_system_to_stage(
            CoreStage::PreUpdate,
            process_input
                .label(EguiSystem::ProcessInput)
                .after(InputSystem),
        );
        app.add_system_to_stage(
            CoreStage::PreUpdate,
            begin_frame
                .label(EguiSystem::BeginFrame)
                .after(EguiSystem::ProcessInput),
        );
        app.add_system_to_stage(
            CoreStage::PostUpdate,
            process_output.label(EguiSystem::ProcessOutput),
        );
        app.add_system_to_stage(
            CoreStage::PostUpdate,
            update_egui_textures.after(EguiSystem::ProcessOutput),
        );
        app.add_system_to_stage(CoreStage::Last, free_egui_textures);

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<egui_node::EguiPipeline>();
            render_app
                .init_resource::<egui_node::EguiPipeline>()
                .init_resource::<EguiTransforms>()
                .add_system_to_stage(
                    RenderStage::Extract,
                    render_systems::extract_egui_render_data,
                )
                .add_system_to_stage(RenderStage::Extract, render_systems::extract_egui_textures)
                .add_system_to_stage(
                    RenderStage::Prepare,
                    render_systems::prepare_egui_transforms,
                )
                .add_system_to_stage(RenderStage::Queue, render_systems::queue_bind_groups);

            let mut render_graph = render_app.world.get_resource_mut::<RenderGraph>().unwrap();
            setup_pipeline(&mut *render_graph, RenderGraphConfig::default());
        }
    }
}

#[derive(Default)]
pub(crate) struct EguiManagedTextures(HashMap<(WindowId, u64), EguiManagedTexture>);

pub(crate) struct EguiManagedTexture {
    handle: Handle<Image>,
    /// Stored in full so we can do partial updates (which bevy doesn't support).
    color_image: egui::ColorImage,
}

fn update_egui_textures(
    mut egui_render_output: ResMut<HashMap<WindowId, EguiRenderOutput>>,
    mut egui_managed_textures: ResMut<EguiManagedTextures>,
    mut image_assets: ResMut<Assets<Image>>,
) {
    for (&window_id, egui_render_output) in egui_render_output.iter_mut() {
        let set_textures = std::mem::take(&mut egui_render_output.textures_delta.set);

        for (texture_id, image_delta) in set_textures {
            let color_image = egui_node::as_color_image(&image_delta.image);

            let texture_id = match texture_id {
                egui::TextureId::Managed(texture_id) => texture_id,
                egui::TextureId::User(_) => continue,
            };

            if let Some(pos) = image_delta.pos {
                // Partial update.
                if let Some(managed_texture) =
                    egui_managed_textures.0.get_mut(&(window_id, texture_id))
                {
                    // TODO: when bevy supports it, only update the part of the texture that changes.
                    update_image_rect(&mut managed_texture.color_image, pos, &color_image);
                    let image = egui_node::color_image_as_bevy_image(&managed_texture.color_image);
                    managed_texture.handle = image_assets.add(image);
                } else {
                    log::warn!("Partial update of a missing texture (id: {:?})", texture_id);
                }
            } else {
                // Full update.
                let image = egui_node::color_image_as_bevy_image(&color_image);
                let handle = image_assets.add(image);
                egui_managed_textures.0.insert(
                    (window_id, texture_id),
                    EguiManagedTexture {
                        handle,
                        color_image,
                    },
                );
            }
        }
    }

    fn update_image_rect(dest: &mut egui::ColorImage, [x, y]: [usize; 2], src: &egui::ColorImage) {
        for sy in 0..src.height() {
            for sx in 0..src.width() {
                dest[(x + sx, y + sy)] = src[(sx, sy)];
            }
        }
    }
}

fn free_egui_textures(
    mut egui_context: ResMut<EguiContext>,
    mut egui_render_output: ResMut<HashMap<WindowId, EguiRenderOutput>>,
    mut egui_managed_textures: ResMut<EguiManagedTextures>,
    mut image_assets: ResMut<Assets<Image>>,
    mut image_events: EventReader<AssetEvent<Image>>,
) {
    for (&window_id, egui_render_output) in egui_render_output.iter_mut() {
        let free_textures = std::mem::take(&mut egui_render_output.textures_delta.free);
        for texture_id in free_textures {
            if let egui::TextureId::Managed(texture_id) = texture_id {
                let managed_texture = egui_managed_textures.0.remove(&(window_id, texture_id));
                if let Some(managed_texture) = managed_texture {
                    image_assets.remove(managed_texture.handle);
                }
            }
        }
    }

    for image_event in image_events.iter() {
        if let AssetEvent::Removed { handle } = image_event {
            egui_context.remove_image(handle);
        }
    }
}

/// Egui's render graph config.
pub struct RenderGraphConfig {
    /// Target window.
    pub window_id: WindowId,
    /// Render pass name.
    pub egui_pass: &'static str,
}

impl Default for RenderGraphConfig {
    fn default() -> Self {
        RenderGraphConfig {
            window_id: WindowId::primary(),
            egui_pass: node::EGUI_PASS,
        }
    }
}

/// Set up egui render pipeline.
///
/// The pipeline for the primary window will already be set up by the [`EguiPlugin`],
/// so you'll only need to manually call this if you want to use multiple windows.
pub fn setup_pipeline(render_graph: &mut RenderGraph, config: RenderGraphConfig) {
    render_graph.add_node(config.egui_pass, EguiNode::new(config.window_id));

    render_graph
        .add_node_edge(
            bevy::core_pipeline::node::MAIN_PASS_DRIVER,
            config.egui_pass,
        )
        .unwrap();

    let _ = render_graph.add_node_edge("ui_pass_driver", config.egui_pass);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{render::settings::WgpuSettings, winit::WinitPlugin, DefaultPlugins};

    #[test]
    fn test_readme_deps() {
        version_sync::assert_markdown_deps_updated!("README.md");
    }

    #[test]
    fn test_headless_mode() {
        App::new()
            .insert_resource(WgpuSettings {
                backends: None,
                ..Default::default()
            })
            .add_plugins_with(DefaultPlugins, |group| group.disable::<WinitPlugin>())
            .add_plugin(EguiPlugin)
            .update();
    }

    #[test]
    fn test_ctx_for_windows_mut_unique_check_passes() {
        let mut egui_context = EguiContext::new();
        let primary_window = WindowId::primary();
        let second_window = WindowId::new();
        egui_context.ctx.insert(primary_window, Default::default());
        egui_context.ctx.insert(second_window, Default::default());
        let [primary_ctx, second_ctx] =
            egui_context.ctx_for_windows_mut([primary_window, second_window]);
        assert!(primary_ctx != second_ctx);
    }

    #[test]
    #[should_panic(
        expected = "Window ids passed to `EguiContext::ctx_for_windows_mut` must be unique"
    )]
    fn test_ctx_for_windows_mut_unique_check_panics() {
        let mut egui_context = EguiContext::new();
        let primary_window = WindowId::primary();
        egui_context.ctx.insert(primary_window, Default::default());
        egui_context.ctx_for_windows_mut([primary_window, primary_window]);
    }

    #[test]
    fn test_try_ctx_for_windows_mut_unique_check_passes() {
        let mut egui_context = EguiContext::new();
        let primary_window = WindowId::primary();
        let second_window = WindowId::new();
        egui_context.ctx.insert(primary_window, Default::default());
        egui_context.ctx.insert(second_window, Default::default());
        let [primary_ctx, second_ctx] =
            egui_context.try_ctx_for_windows_mut([primary_window, second_window]);
        assert!(primary_ctx.is_some());
        assert!(second_ctx.is_some());
        assert!(primary_ctx != second_ctx);
    }

    #[test]
    #[should_panic(
        expected = "Window ids passed to `EguiContext::ctx_for_windows_mut` must be unique"
    )]
    fn test_try_ctx_for_windows_mut_unique_check_panics() {
        let mut egui_context = EguiContext::new();
        let primary_window = WindowId::primary();
        egui_context.ctx.insert(primary_window, Default::default());
        egui_context.try_ctx_for_windows_mut([primary_window, primary_window]);
    }
}
