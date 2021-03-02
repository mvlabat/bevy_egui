# `bevy_egui`

[![Crates.io](https://img.shields.io/crates/v/bevy_egui.svg)](https://crates.io/crates/bevy_egui)
[![Documentation](https://docs.rs/bevy_egui/badge.svg)](https://docs.rs/bevy_egui)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/bevyengine/bevy/blob/master/LICENSE)
[![Downloads](https://img.shields.io/crates/d/bevy_egui.svg)](https://crates.io/crates/bevy_egui)
[![Rust](https://github.com/mvlabat/bevy_egui/workflows/CI/badge.svg)](https://github.com/mvlabat/bevy_egui/actions)

This crate provides a [Egui](https://github.com/emilk/egui) integration for the [Bevy](https://github.com/bevyengine/bevy) game engine.

**Features:**
- Desktop and web ([bevy_webgl2](https://github.com/mrk-its/bevy_webgl2)) platforms support
- Clipboard (web support is limited to the same window, see [rust-windowing/winit#1829](https://github.com/rust-windowing/winit/issues/1829))
- Opening URLs

`bevy_egui` can be compiled with using only `bevy` and `egui` as dependencies: `manage_clipboard` and `open_url` features,
that require additional crates, can be disabled.

![bevy_egui](bevy_egui.png)

## Trying out

An example WASM project is live at [mvlabat.github.io/bevy_egui_web_showcase](https://mvlabat.github.io/bevy_egui_web_showcase/index.html) [[source](https://github.com/mvlabat/bevy_egui_web_showcase)].

**Note** that in order to use `bevy_egui`in WASM you need [bevy_webgl2](https://github.com/mrk-its/bevy_webgl2) of at least `0.4.1` version.

## Usage

Here's a minimal usage example:
```toml
# Cargo.toml
[dependencies]
bevy = "0.4"
bevy_egui = "0.3"
```

```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContext, EguiPlugin};

fn main() {
    App::build()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_system(ui_example.system())
        .run();
}

fn ui_example(mut egui_context: ResMut<EguiContext>) {
    let ctx = &mut egui_context.ctx;
    egui::Window::new("Hello").show(ctx, |ui| {
        ui.label("world");
    });
}
```

For a more advanced example, see [examples/ui.rs](examples/ui.rs).

```bash
cargo run --example ui --features="bevy/x11 bevy/png bevy/bevy_wgpu"
```

## See also

- [`jakobhellermann/bevy-inspector-egui`](https://github.com/jakobhellermann/bevy-inspector-egui)
- [`mvlabat/bevy_megaui`](https://github.com/mvlabat/bevy_megaui)
