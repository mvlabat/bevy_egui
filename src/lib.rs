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
//!     App::build()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugin(EguiPlugin)
//!         .add_system(ui_example.system())
//!         .run();
//! }
//!
//! fn ui_example(egui_context: Res<EguiContext>) {
//!     egui::Window::new("Hello").show(egui_context.ctx(), |ui| {
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
//! - [`bevy_megaui`](https://github.com/mvlabat/bevy_megaui)

pub use egui;

mod egui_node;
mod systems;
mod transform_node;

use crate::{egui_node::EguiNode, systems::*, transform_node::EguiTransformNode};
use bevy::{
    app::{AppBuilder, CoreStage, Plugin},
    asset::{Assets, Handle, HandleUntyped},
    ecs::{
        schedule::{ParallelSystemDescriptorCoercion, StageLabel, SystemLabel, SystemStage},
        system::IntoSystem,
    },
    input::InputSystem,
    log,
    reflect::TypeUuid,
    render::{
        pipeline::{
            BlendFactor, BlendOperation, BlendState, ColorTargetState, ColorWrite, CompareFunction,
            CullMode, DepthBiasState, DepthStencilState, FrontFace, MultisampleState,
            PipelineDescriptor, PrimitiveState, StencilFaceState, StencilState,
        },
        render_graph::{base, base::Msaa, RenderGraph, WindowSwapChainNode, WindowTextureNode},
        shader::{Shader, ShaderStage, ShaderStages},
        texture::{Texture, TextureFormat},
        RenderStage,
    },
    utils::HashMap,
    window::WindowId,
};
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use clipboard::{ClipboardContext, ClipboardProvider};
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use std::cell::{RefCell, RefMut};
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use thread_local::ThreadLocal;

/// A handle pointing to the egui [`PipelineDescriptor`].
pub const EGUI_PIPELINE_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(PipelineDescriptor::TYPE_UUID, 9404026720151354217);
/// Name of the transform uniform.
pub const EGUI_TRANSFORM_RESOURCE_BINDING_NAME: &str = "EguiTransform";
/// Name of the texture uniform.
pub const EGUI_TEXTURE_RESOURCE_BINDING_NAME: &str = "EguiTexture_texture";

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

/// Is used for storing the input passed to Egui. The actual resource is a [`HashMap<WindowId, EguiInput>`].
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

/// Is used for storing Egui shapes. The actual resource is [`HashMap<WindowId, EguiShapes>`].
#[derive(Clone, Default)]
pub struct EguiShapes {
    /// Pairs of rectangles and paint commands.
    ///
    /// The field gets populated during the [`EguiStage::UiFrameEnd`] stage and reset during `EguiNode::update`.
    pub shapes: Vec<egui::paint::ClippedShape>,
}

/// Is used for storing Egui output. The actual resource is [`HashMap<WindowId, EguiOutput>`].
#[derive(Clone, Default)]
pub struct EguiOutput {
    /// The field gets updated during the [`EguiStage::UiFrameEnd`] stage.
    pub output: egui::Output,
}

/// A resource for storing `bevy_egui` context.
pub struct EguiContext {
    ctx: HashMap<WindowId, egui::CtxRef>,
    egui_textures: HashMap<egui::TextureId, Handle<Texture>>,
    mouse_position: Option<(f32, f32)>,
}

impl EguiContext {
    fn new() -> Self {
        Self {
            ctx: HashMap::default(),
            egui_textures: Default::default(),
            mouse_position: Some((0.0, 0.0)),
        }
    }

    /// Egui context of the primary window.
    #[track_caller]
    pub fn ctx(&self) -> &egui::CtxRef {
        self.ctx.get(&WindowId::primary()).expect("`EguiContext::ctx()` called before the ctx has been initialized. Consider moving your UI system to `CoreStage::Update` or run you system after `EguiSystem::BeginFrame`.")
    }

    /// Egui context for a specific window.
    /// If you want to display UI on a non-primary window,
    /// make sure to set up the render graph by calling [`setup_pipeline`].
    #[track_caller]
    pub fn ctx_for_window(&self, window: WindowId) -> &egui::CtxRef {
        &self
            .ctx
            .get(&window)
            .ok_or_else(|| format!("window with id {} not found", window))
            .unwrap()
    }

    /// Fallible variant of [`EguiContext::ctx_for_window`]. Make sure to set up the render graph by calling [`setup_pipeline`].
    pub fn try_ctx_for_window(&self, window: WindowId) -> Option<&egui::CtxRef> {
        self.ctx.get(&window)
    }

    /// Can accept either a strong or a weak handle.
    ///
    /// You may want to pass a weak handle if you control removing texture assets in your
    /// application manually and you don't want to bother with cleaning up textures in egui.
    ///
    /// You'll want to pass a strong handle if a texture is used only in egui and there's no
    /// handle copies stored anywhere else.
    pub fn set_egui_texture(&mut self, id: u64, texture: Handle<Texture>) {
        log::debug!("Set egui texture: {:?}", texture);
        self.egui_textures
            .insert(egui::TextureId::User(id), texture);
    }

    /// Removes a texture handle associated with the id.
    pub fn remove_egui_texture(&mut self, id: u64) {
        let texture_handle = self.egui_textures.remove(&egui::TextureId::User(id));
        log::debug!("Remove egui texture: {:?}", texture_handle);
    }

    // Is called when we get an event that a texture asset is removed.
    fn remove_texture(&mut self, texture_handle: &Handle<Texture>) {
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
#[derive(Debug, Default, Clone, PartialEq)]
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
    /// Keeps the transform uniform up to date.
    pub const EGUI_TRANSFORM: &str = "egui_transform";
}

#[derive(StageLabel, Clone, Hash, Debug, Eq, PartialEq)]
/// The names of `bevy_egui` stages.
pub enum EguiStage {
    /// Runs before [`bevy::render::RenderStage::RenderResource`]. This is where we read Egui's output.
    UiFrameEnd,
}

#[derive(SystemLabel, Clone, Hash, Debug, Eq, PartialEq)]
/// The names of egui systems.
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
    fn build(&self, app: &mut AppBuilder) {
        app.add_stage_before(
            RenderStage::RenderResource,
            EguiStage::UiFrameEnd,
            SystemStage::parallel(),
        );

        app.add_system_to_stage(
            CoreStage::PreUpdate,
            process_input
                .system()
                .label(EguiSystem::ProcessInput)
                .after(InputSystem),
        );
        app.add_system_to_stage(
            CoreStage::PreUpdate,
            begin_frame
                .system()
                .label(EguiSystem::BeginFrame)
                .after(EguiSystem::ProcessInput),
        );
        app.add_system_to_stage(
            EguiStage::UiFrameEnd,
            process_output.system().label(EguiSystem::ProcessOutput),
        );

        let world = app.world_mut();
        world.get_resource_or_insert_with(EguiSettings::default);
        world.get_resource_or_insert_with(HashMap::<WindowId, EguiInput>::default);
        world.get_resource_or_insert_with(HashMap::<WindowId, EguiOutput>::default);
        world.get_resource_or_insert_with(HashMap::<WindowId, WindowSize>::default);
        world.get_resource_or_insert_with(HashMap::<WindowId, EguiShapes>::default);
        #[cfg(feature = "manage_clipboard")]
        world.get_resource_or_insert_with(EguiClipboard::default);
        world.insert_resource(EguiContext::new());

        let world = world.cell();

        let mut pipelines = world
            .get_resource_mut::<Assets<PipelineDescriptor>>()
            .unwrap();
        let msaa = world.get_resource::<Msaa>().unwrap();
        let mut shaders = world.get_resource_mut::<Assets<Shader>>().unwrap();

        pipelines.set_untracked(
            EGUI_PIPELINE_HANDLE,
            build_egui_pipeline(&mut shaders, msaa.samples),
        );

        let mut render_graph = world.get_resource_mut::<RenderGraph>().unwrap();
        setup_pipeline(&mut render_graph, &msaa, RenderGraphConfig::default());
    }
}

/// Egui's render graph config.
#[allow(missing_docs)]
pub struct RenderGraphConfig {
    pub window_id: WindowId,
    pub egui_pass: &'static str,
    pub main_pass: &'static str,
    pub swap_chain_node: &'static str,
    pub depth_texture: &'static str,
    pub sampled_color_attachment: &'static str,
    pub transform_node: &'static str,
}

impl Default for RenderGraphConfig {
    fn default() -> Self {
        RenderGraphConfig {
            window_id: WindowId::primary(),
            egui_pass: node::EGUI_PASS,
            main_pass: base::node::MAIN_PASS,
            swap_chain_node: base::node::PRIMARY_SWAP_CHAIN,
            depth_texture: base::node::MAIN_DEPTH_TEXTURE,
            sampled_color_attachment: base::node::MAIN_SAMPLED_COLOR_ATTACHMENT,
            transform_node: node::EGUI_TRANSFORM,
        }
    }
}

/// Set up egui render pipeline.
///
/// The pipeline for the primary window will already be set up by the [`EguiPlugin`],
/// so you'll only need to manually call this if you want to use multiple windows.
pub fn setup_pipeline(render_graph: &mut RenderGraph, msaa: &Msaa, config: RenderGraphConfig) {
    render_graph.add_node(config.egui_pass, EguiNode::new(&msaa, config.window_id));
    render_graph
        .add_node_edge(config.main_pass, config.egui_pass)
        .unwrap();
    if let Ok(ui_pass) = render_graph.get_node_id(bevy::ui::node::UI_PASS) {
        render_graph
            .add_node_edge(ui_pass, config.egui_pass)
            .unwrap();
    }
    render_graph
        .add_slot_edge(
            config.swap_chain_node,
            WindowSwapChainNode::OUT_TEXTURE,
            config.egui_pass,
            if msaa.samples > 1 {
                "color_resolve_target"
            } else {
                "color_attachment"
            },
        )
        .unwrap();
    render_graph
        .add_slot_edge(
            config.depth_texture,
            WindowTextureNode::OUT_TEXTURE,
            config.egui_pass,
            "depth",
        )
        .unwrap();
    if msaa.samples > 1 {
        render_graph
            .add_slot_edge(
                config.sampled_color_attachment,
                WindowSwapChainNode::OUT_TEXTURE,
                config.egui_pass,
                "color_attachment",
            )
            .unwrap();
    }
    render_graph.add_system_node(
        config.transform_node,
        EguiTransformNode::new(config.window_id),
    );
    render_graph
        .add_node_edge(config.transform_node, config.egui_pass)
        .unwrap();
}

fn build_egui_pipeline(shaders: &mut Assets<Shader>, sample_count: u32) -> PipelineDescriptor {
    PipelineDescriptor {
        primitive: PrimitiveState {
            front_face: FrontFace::Cw,
            cull_mode: CullMode::None,
            ..Default::default()
        },
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: CompareFunction::LessEqual,
            stencil: StencilState {
                front: StencilFaceState::IGNORE,
                back: StencilFaceState::IGNORE,
                read_mask: 0,
                write_mask: 0,
            },
            bias: DepthBiasState {
                constant: 0,
                slope_scale: 0.0,
                clamp: 0.0,
            },
            clamp_depth: false,
        }),
        color_target_states: vec![ColorTargetState {
            format: TextureFormat::default(),
            color_blend: BlendState {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha_blend: BlendState {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            write_mask: ColorWrite::ALL,
        }],
        multisample: MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        ..PipelineDescriptor::new(ShaderStages {
            vertex: shaders.add(Shader::from_glsl(
                ShaderStage::Vertex,
                if cfg!(target_arch = "wasm32") {
                    include_str!("egui.es.vert")
                } else {
                    include_str!("egui.vert")
                },
            )),
            fragment: Some(shaders.add(Shader::from_glsl(
                ShaderStage::Fragment,
                if cfg!(target_arch = "wasm32") {
                    include_str!("egui.es.frag")
                } else {
                    include_str!("egui.frag")
                },
            ))),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_readme_deps() {
        version_sync::assert_markdown_deps_updated!("README.md");
    }
}
