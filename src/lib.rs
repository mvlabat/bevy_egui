#![deny(missing_docs)]

//! This crate provides a [Egui](https://github.com/emilk/egui) integration for the [Bevy](https://github.com/bevyengine/bevy) game engine.
//!
//! **Trying out:**
//!
//! An example WASM project is live at [mvlabat.github.io/bevy_egui_web_showcase](https://mvlabat.github.io/bevy_egui_web_showcase/index.html) [[source](https://github.com/mvlabat/bevy_egui_web_showcase)].
//!
//! **Features:**
//! - Desktop and web ([bevy_webgl2](https://github.com/mrk-its/bevy_webgl2)) platforms support
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
use bevy::{
    app::{App, CoreStage, Plugin, StartupStage},
    asset::Handle,
    ecs::schedule::{ParallelSystemDescriptorCoercion, SystemLabel},
    input::InputSystem,
    log,
    prelude::{AssetEvent, Assets, Commands, EventReader, ResMut},
    render::{render_graph::RenderGraph, texture::Image, RenderApp, RenderStage},
    utils::HashMap,
    window::WindowId,
};
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use clipboard::{ClipboardContext, ClipboardProvider};
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use std::cell::{RefCell, RefMut};
use std::collections::hash_map::Entry;
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
    clipboard: ThreadLocal<Option<RefCell<ClipboardContext>>>,
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
            if let Err(err) = clipboard.set_contents(contents.to_owned()) {
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
            match clipboard.get_contents() {
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
    fn get(&self) -> Option<RefMut<ClipboardContext>> {
        self.clipboard
            .get_or(|| {
                ClipboardContext::new()
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
pub struct EguiShapes {
    /// Pairs of rectangles and paint commands.
    ///
    /// The field gets populated during the [`EguiSystem::ProcessOutput`] system in the [`CoreStage::PostUpdate`] and reset during `EguiNode::update`.
    pub shapes: Vec<egui::epaint::ClippedShape>,
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
    egui_textures: HashMap<u64, Handle<Image>>,
    mouse_position: Option<(WindowId, egui::Vec2)>,
}

impl EguiContext {
    fn new() -> Self {
        Self {
            ctx: HashMap::default(),
            egui_textures: Default::default(),
            mouse_position: None,
        }
    }

    /// Egui context of the primary window.
    ///
    /// This function is only available when the `multi_threaded` feature is enabled.
    /// The preferable way is to use `ctx_mut` to avoid unpredictable blocking inside UI systems.
    #[cfg(feature = "multi_threaded")]
    #[track_caller]
    pub fn ctx(&self) -> &egui::CtxRef {
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
    pub fn ctx_for_window(&self, window: WindowId) -> &egui::CtxRef {
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
        let mut unique_ids = std::collections::HashSet::new();
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
        let mut unique_ids = std::collections::HashSet::new();
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
    /// application manually and you don't want to bother with cleaning up textures in egui.
    ///
    /// You'll want to pass a strong handle if a texture is used only in egui and there's no
    /// handle copies stored anywhere else.
    pub fn set_egui_texture(&mut self, id: u64, texture: Handle<Image>) {
        log::debug!("Set egui texture: {:?}", texture);
        self.egui_textures.insert(id, texture);
    }

    /// Removes a texture handle associated with the id.
    pub fn remove_egui_texture(&mut self, id: u64) {
        let texture_handle = self.egui_textures.remove(&id);
        log::debug!("Remove egui texture: {:?}", texture_handle);
    }

    // Is called when we get an event that a texture asset is removed.
    fn remove_texture(&mut self, texture_handle: &Handle<Image>) {
        log::debug!("Removing egui handles: {:?}", texture_handle);
        self.egui_textures = self
            .egui_textures
            .iter()
            .map(|(id, texture)| (*id, texture.clone()))
            .filter(|(_, texture)| texture != texture_handle)
            .collect();
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
        app.add_system_to_stage(CoreStage::PostUpdate, update_egui_textures);

        let world = &mut app.world;
        world.get_resource_or_insert_with(EguiSettings::default);
        world.get_resource_or_insert_with(HashMap::<WindowId, EguiInput>::default);
        world.get_resource_or_insert_with(HashMap::<WindowId, EguiOutput>::default);
        world.get_resource_or_insert_with(HashMap::<WindowId, WindowSize>::default);
        world.get_resource_or_insert_with(HashMap::<WindowId, EguiShapes>::default);
        world.get_resource_or_insert_with(EguiFontTextures::default);
        #[cfg(feature = "manage_clipboard")]
        world.get_resource_or_insert_with(EguiClipboard::default);
        world.insert_resource(EguiContext::new());

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
pub(crate) struct EguiFontTextures(HashMap<WindowId, (Handle<Image>, u64)>);

fn update_egui_textures(
    _commands: Commands,
    mut egui_context: ResMut<EguiContext>,
    mut egui_font_textures: ResMut<EguiFontTextures>,
    mut image_assets: ResMut<Assets<Image>>,
    mut image_events: EventReader<AssetEvent<Image>>,
) {
    egui_context.ctx.iter().for_each(|(&window_id, ctx)| {
        let texture = ctx.font_image();

        match egui_font_textures.0.entry(window_id) {
            Entry::Occupied(entry) if entry.get().1 == texture.version => {}
            Entry::Occupied(mut entry) => {
                let image = image_assets.add(egui_node::as_wgpu_image(&texture));
                entry.insert((image, texture.version));
            }
            Entry::Vacant(entry) => {
                let image = image_assets.add(egui_node::as_wgpu_image(&texture));
                entry.insert((image, texture.version));
            }
        };
    });

    for image_event in image_events.iter() {
        if let AssetEvent::Removed { handle } = image_event {
            egui_context.remove_texture(handle);
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
pub fn setup_pipeline(
    render_graph: &mut RenderGraph,
    config: RenderGraphConfig,
    //msaa: &Msaa,
) {
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
    use bevy::{render::options::WgpuOptions, winit::WinitPlugin, DefaultPlugins};

    #[test]
    fn test_readme_deps() {
        version_sync::assert_markdown_deps_updated!("README.md");
    }

    #[test]
    fn test_headless_mode() {
        App::new()
            .insert_resource(WgpuOptions {
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
