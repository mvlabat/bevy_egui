#![deny(missing_docs)]

//! This crate provides a [egui](https://crates.io/crates/egui) integration for the [Bevy](https://github.com/bevyengine/bevy) game engine.
//!
//! `bevy_egui` depends solely on `egui` and `bevy` with only `render` feature required.
//!
//! ## Trying out
//!
//! An example WASM project is live at [mvlabat.github.io/bevy_egui_web_showcase](https://mvlabat.github.io/bevy_egui_web_showcase/index.html) [[source](https://github.com/mvlabat/bevy_egui_web_showcase)].
//!
//! **Note** that in order to use `bevy_egui`in WASM you need [bevy_webgl2](https://github.com/mrk-its/bevy_webgl2) of at least `0.4.1` version.
//!
//! ## Usage
//!
//! Here's a minimal usage example:
//!
//! ```rust
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
//! fn ui_example(mut egui_context: ResMut<EguiContext>) {
//!     let ctx = &mut egui_context.ctx;
//!     egui::Window::new("Hello").show(ctx, |ui| {
//!         ui.label("world");
//!     });
//! }
//! ```
//!
//! For a more advanced example, see [examples/ui.rs](examples/ui.rs).
//!
//! ```bash
//! cargo run --example ui --features="bevy/x11 bevy/png bevy/bevy_wgpu"
//! ```

pub use egui;

mod egui_node;
mod systems;
mod transform_node;

use crate::{egui_node::EguiNode, systems::*, transform_node::EguiTransformNode};
use bevy::{
    app::{stage as bevy_stage, AppBuilder, EventReader, Plugin},
    asset::{Assets, Handle, HandleUntyped},
    ecs::{IntoSystem, SystemStage},
    input::mouse::MouseWheel,
    log,
    reflect::TypeUuid,
    render::{
        pipeline::{
            BlendDescriptor, BlendFactor, BlendOperation, ColorStateDescriptor, ColorWrite,
            CompareFunction, CullMode, DepthStencilStateDescriptor, FrontFace, IndexFormat,
            PipelineDescriptor, RasterizationStateDescriptor, StencilStateDescriptor,
            StencilStateFaceDescriptor,
        },
        render_graph::{base, base::Msaa, RenderGraph, WindowSwapChainNode, WindowTextureNode},
        shader::{Shader, ShaderStage, ShaderStages},
        stage as bevy_render_stage,
        texture::{Texture, TextureFormat},
    },
    window::{CursorMoved, ReceivedCharacter},
};
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use clipboard::{ClipboardContext, ClipboardProvider};
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
#[cfg(all(feature = "manage_clipboard", not(target_arch = "wasm32")))]
use thread_local::ThreadLocal;

/// A handle pointing to the egui [PipelineDescriptor].
pub const EGUI_PIPELINE_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(PipelineDescriptor::TYPE_UUID, 9404026720151354217);
/// Name of the transform uniform.
pub const EGUI_TRANSFORM_RESOURCE_BINDING_NAME: &str = "EguiTransform";
/// Name of the texture uniform.
pub const EGUI_TEXTURE_RESOURCE_BINDING_NAME: &str = "EguiTexture_texture";

/// Adds all egui resources and render graph nodes.
pub struct EguiPlugin;

/// A resource containing global UI settings.
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

/// A resource that stores the input passed to Egui.
/// It gets reset during the [stage::UI_FRAME] stage.
#[derive(Clone, Debug, Default)]
pub struct EguiInput {
    /// Egui's raw input.
    pub raw_input: egui::RawInput,
}

/// A resource for accessing clipboard.
/// Is available only if `manage_clipboard` feature is enabled.
#[cfg(feature = "manage_clipboard")]
#[derive(Default)]
pub struct EguiClipboard {
    #[cfg(not(target_arch = "wasm32"))]
    clipboard: ThreadLocal<Option<RefCell<ClipboardContext>>>,
    #[cfg(target_arch = "wasm32")]
    clipboard: String,
}

impl EguiClipboard {
    /// Sets clipboard contents.
    pub fn set_contents(&mut self, contents: &str) {
        self.set_contents_impl(contents);
    }

    /// Gets clipboard contents. Returns [None] if clipboard provider is unavailable or returns an error.
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

#[derive(Clone, Default)]
/// A resource for storing Egui shapes.
pub struct EguiShapes {
    /// Pairs of rectangles and paint commands.
    ///
    /// The field gets populated during the [stage::UI_FRAME_END] stage and reset during `EguiNode::update`.
    pub shapes: Vec<egui::paint::ClippedShape>,
}

/// A resource for storing Egui output.
#[derive(Clone, Default)]
pub struct EguiOutput {
    /// The field gets updated during the [stage::UI_FRAME_END] stage.
    pub output: egui::Output,
}

/// A resource that is used to store `bevy_egui` context.
pub struct EguiContext {
    /// Egui context.
    pub ctx: egui::CtxRef,
    egui_textures: HashMap<egui::TextureId, Handle<Texture>>,

    mouse_position: (f32, f32),
    cursor: EventReader<CursorMoved>,
    mouse_wheel: EventReader<MouseWheel>,
    received_character: EventReader<ReceivedCharacter>,
}

impl EguiContext {
    fn new() -> Self {
        Self {
            ctx: Default::default(),
            egui_textures: Default::default(),
            mouse_position: (0.0, 0.0),
            cursor: Default::default(),
            mouse_wheel: Default::default(),
            received_character: Default::default(),
        }
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

/// The names of `bevy_egui` stages.
pub mod stage {
    /// Runs after [bevy::app::stage::EVENT]. This is where `bevy_egui` translates Bevy's input events to Egui.
    pub const INPUT: &str = "input";
    /// Runs after [INPUT].
    pub const POST_INPUT: &str = "post_input";
    /// Runs after [POST_INPUT]. All Egui widgets should be added during or after this stage and before [UI_FRAME_END].
    pub const UI_FRAME: &str = "ui_frame";
    /// Runs before [bevy::render::stage::RENDER_RESOURCE]. This is where we read Egui's output.
    pub const UI_FRAME_END: &str = "ui_frame_end";
    /// Runs after [UI_FRAME_END].
    pub const POST_UI_FRAME_END: &str = "post_ui_frame_end";
}

impl Plugin for EguiPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_stage_after(bevy_stage::EVENT, stage::INPUT, SystemStage::parallel());
        app.add_stage_after(stage::INPUT, stage::POST_INPUT, SystemStage::parallel());
        app.add_stage_after(stage::POST_INPUT, stage::UI_FRAME, SystemStage::parallel());
        app.add_stage_before(
            bevy_render_stage::RENDER_RESOURCE,
            stage::UI_FRAME_END,
            SystemStage::parallel(),
        );
        app.add_stage_after(
            stage::UI_FRAME_END,
            stage::POST_UI_FRAME_END,
            SystemStage::parallel(),
        );

        #[cfg(all(
            feature = "manage_clipboard",
            target_arch = "wasm32",
            web_sys_unstable_apis
        ))]
        app.add_startup_system(setup_clipboard_event_listeners.system());
        app.add_system_to_stage(stage::INPUT, process_input.system());
        app.add_system_to_stage(stage::UI_FRAME, begin_frame.system());
        app.add_system_to_stage(stage::UI_FRAME_END, process_output.system());

        let resources = app.resources_mut();
        resources.get_or_insert_with(EguiSettings::default);
        resources.get_or_insert_with(EguiInput::default);
        resources.get_or_insert_with(EguiOutput::default);
        resources.get_or_insert_with(EguiShapes::default);
        resources.get_or_insert_with(EguiClipboard::default);
        resources.insert(EguiContext::new());
        resources.insert(WindowSize::new(0.0, 0.0, 0.0));

        let mut pipelines = resources.get_mut::<Assets<PipelineDescriptor>>().unwrap();
        let mut shaders = resources.get_mut::<Assets<Shader>>().unwrap();
        let msaa = resources.get::<Msaa>().unwrap();

        pipelines.set_untracked(
            EGUI_PIPELINE_HANDLE,
            build_egui_pipeline(&mut shaders, msaa.samples),
        );
        let mut render_graph = resources.get_mut::<RenderGraph>().unwrap();

        render_graph.add_node(node::EGUI_PASS, EguiNode::new(&msaa));
        render_graph
            .add_node_edge(base::node::MAIN_PASS, node::EGUI_PASS)
            .unwrap();

        render_graph
            .add_slot_edge(
                base::node::PRIMARY_SWAP_CHAIN,
                WindowSwapChainNode::OUT_TEXTURE,
                node::EGUI_PASS,
                if msaa.samples > 1 {
                    "color_resolve_target"
                } else {
                    "color_attachment"
                },
            )
            .unwrap();

        render_graph
            .add_slot_edge(
                base::node::MAIN_DEPTH_TEXTURE,
                WindowTextureNode::OUT_TEXTURE,
                node::EGUI_PASS,
                "depth",
            )
            .unwrap();

        if msaa.samples > 1 {
            render_graph
                .add_slot_edge(
                    base::node::MAIN_SAMPLED_COLOR_ATTACHMENT,
                    WindowSwapChainNode::OUT_TEXTURE,
                    node::EGUI_PASS,
                    "color_attachment",
                )
                .unwrap();
        }

        // Transform.
        render_graph.add_system_node(node::EGUI_TRANSFORM, EguiTransformNode::new());
        render_graph
            .add_node_edge(node::EGUI_TRANSFORM, node::EGUI_PASS)
            .unwrap();
    }
}

fn build_egui_pipeline(shaders: &mut Assets<Shader>, sample_count: u32) -> PipelineDescriptor {
    PipelineDescriptor {
        rasterization_state: Some(RasterizationStateDescriptor {
            front_face: FrontFace::Cw,
            cull_mode: CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
            clamp_depth: false,
        }),
        depth_stencil_state: Some(DepthStencilStateDescriptor {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: CompareFunction::LessEqual,
            stencil: StencilStateDescriptor {
                front: StencilStateFaceDescriptor::IGNORE,
                back: StencilStateFaceDescriptor::IGNORE,
                read_mask: 0,
                write_mask: 0,
            },
        }),
        color_states: vec![ColorStateDescriptor {
            format: TextureFormat::default(),
            color_blend: BlendDescriptor {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha_blend: BlendDescriptor {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            write_mask: ColorWrite::ALL,
        }],
        index_format: IndexFormat::Uint32,
        sample_count,
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
