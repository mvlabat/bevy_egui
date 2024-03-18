[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/banner2-direct.svg)](https://stand-with-ukraine.pp.ua)

**Hey!** I'm the author of the crate, and I was born in Mariupol, Ukraine. When russians started the war in 2014, I moved to Kyiv. My parents, who had been staying in Mariupol till the start of the full-scale invasion, barely escaped the city alive. By the moment of writing (November 5th, 2023), we had [874 air raid alerts in Kyiv, and russians managed to bomb the city 132 times](https://air-alarms.in.ua/en/region/kyiv).

**If you are using this crate, please consider donating to any of the listed funds (see the banner above), that will mean a lot to me.**

# `bevy_egui`

[![Crates.io](https://img.shields.io/crates/v/bevy_egui.svg)](https://crates.io/crates/bevy_egui)
[![Documentation](https://docs.rs/bevy_egui/badge.svg)](https://docs.rs/bevy_egui)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/bevyengine/bevy/blob/master/LICENSE)
[![Downloads](https://img.shields.io/crates/d/bevy_egui.svg)](https://crates.io/crates/bevy_egui)
[![Rust](https://github.com/mvlabat/bevy_egui/workflows/CI/badge.svg)](https://github.com/mvlabat/bevy_egui/actions)

This crate provides an [Egui](https://github.com/emilk/egui) integration for the [Bevy](https://github.com/bevyengine/bevy) game engine.

**Trying out:**

An example WASM project is live at [mvlabat.github.io/bevy_egui_web_showcase](https://mvlabat.github.io/bevy_egui_web_showcase/index.html) [[source](https://github.com/mvlabat/bevy_egui_web_showcase)].

**Features:**
- Desktop and web platforms support
- Clipboard (web support is limited to the same window, see [rust-windowing/winit#1829](https://github.com/rust-windowing/winit/issues/1829))
- Opening URLs
- Multiple windows support (see [./examples/two_windows.rs](https://github.com/mvlabat/bevy_egui/blob/v0.20.1/examples/two_windows.rs))

`bevy_egui` can be compiled with using only `bevy` and `egui` as dependencies: `manage_clipboard` and `open_url` features,
that require additional crates, can be disabled.

![bevy_egui](bevy_egui.png)

## Dependencies

On Linux, this crate requires certain parts of [XCB](https://xcb.freedesktop.org/) to be installed on your system. On Debian-based systems, these can be installed with the following command:

```
$ sudo apt install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

## Usage

Here's a minimal usage example:
```toml
# Cargo.toml
[dependencies]
bevy = "0.13"
bevy_egui = "0.26"
```

```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        // Systems that create Egui widgets should be run during the `CoreSet::Update` set,
        // or after the `EguiSet::BeginFrame` system (which belongs to the `CoreSet::PreUpdate` set).
        .add_systems(Update, ui_example_system)
        .run();
}

fn ui_example_system(mut contexts: EguiContexts) {
    egui::Window::new("Hello").show(contexts.ctx_mut(), |ui| {
        ui.label("world");
    });
}

```

For a more advanced example, see [examples/ui.rs](https://github.com/mvlabat/bevy_egui/blob/v0.20.1/examples/ui.rs).

```bash
cargo run --example ui
```

## See also

- [`jakobhellermann/bevy-inspector-egui`](https://github.com/jakobhellermann/bevy-inspector-egui)

## Bevy support table

**Note:** if you're looking for a `bevy_egui` version that supports `main` branch of Bevy, check out [open PRs](https://github.com/mvlabat/bevy_egui/pulls), there's a great chance we've already started working on the future Bevy release support.

| bevy | bevy_egui |
|------|-----------|
| 0.13 | 0.25-0.26 |
| 0.12 | 0.23-0.24 |
| 0.11 | 0.21-0.22 |
| 0.10 | 0.20      |
| 0.9  | 0.17-0.19 |
| 0.8  | 0.15-0.16 |
| 0.7  | 0.13-0.14 |
| 0.6  | 0.10-0.12 |
| 0.5  | 0.4-0.9   |
| 0.4  | 0.1-0.3   |
