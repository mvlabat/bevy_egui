use bevy::prelude::*;
use bevy_egui::{egui, EguiContext, EguiPlugin, EguiSettings};

struct Images {
    bevy_icon: Handle<Image>,
    bevy_icon_inverted: Handle<Image>,
}

impl FromWorld for Images {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource_mut::<AssetServer>().unwrap();
        Self {
            bevy_icon: asset_server.load("icon.png"),
            bevy_icon_inverted: asset_server.load("icon_inverted.png"),
        }
    }
}

/// This example demonstrates the following functionality and use-cases of bevy_egui:
/// - rendering loaded assets;
/// - toggling hidpi scaling (by pressing '/' button);
/// - configuring egui contexts during the startup.
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
        .insert_resource(Msaa { samples: 4 })
        .init_resource::<UiState>()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_startup_system(configure_visuals)
        .add_system(update_ui_scale_factor)
        .add_system(ui_example)
        .run();
}

#[derive(Default)]
struct UiState {
    label: String,
    value: f32,
    painting: Painting,
    inverted: bool,
    egui_texture_handle: Option<egui::TextureHandle>,
}

fn configure_visuals(mut egui_ctx: ResMut<EguiContext>) {
    egui_ctx.ctx_mut().set_visuals(egui::Visuals {
        window_rounding: 0.0.into(),
        ..Default::default()
    });
}

fn update_ui_scale_factor(
    keyboard_input: Res<Input<KeyCode>>,
    mut toggle_scale_factor: Local<Option<bool>>,
    mut egui_settings: ResMut<EguiSettings>,
    windows: Res<Windows>,
) {
    if keyboard_input.just_pressed(KeyCode::Slash) || toggle_scale_factor.is_none() {
        *toggle_scale_factor = Some(!toggle_scale_factor.unwrap_or(true));

        if let Some(window) = windows.get_primary() {
            let scale_factor = if toggle_scale_factor.unwrap() {
                1.0
            } else {
                1.0 / window.scale_factor()
            };
            egui_settings.scale_factor = scale_factor;
        }
    }
}

fn ui_example(
    mut egui_ctx: ResMut<EguiContext>,
    mut ui_state: ResMut<UiState>,
    // You are not required to store Egui texture ids in systems. We store this one here just to
    // demonstrate that rendering by using a texture id of a removed image is handled without
    // making bevy_egui panic.
    mut rendered_texture_id: Local<egui::TextureId>,
    mut is_initialized: Local<bool>,
    images: Local<Images>,
) {
    let egui_texture_handle = ui_state
        .egui_texture_handle
        .get_or_insert_with(|| {
            egui_ctx
                .ctx_mut()
                .load_texture("example-image", egui::ColorImage::example())
        })
        .clone();

    let mut load = false;
    let mut remove = false;
    let mut invert = false;

    if !*is_initialized {
        *is_initialized = true;
        *rendered_texture_id = egui_ctx.add_image(images.bevy_icon.clone_weak());
    }

    egui::SidePanel::left("side_panel")
        .default_width(200.0)
        .show(egui_ctx.ctx_mut(), |ui| {
            ui.heading("Side Panel");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(&mut ui_state.label);
            });

            ui.add(egui::widgets::Image::new(
                egui_texture_handle.id(),
                egui_texture_handle.size_vec2(),
            ));

            ui.add(egui::Slider::new(&mut ui_state.value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                ui_state.value += 1.0;
            }

            ui.allocate_space(egui::Vec2::new(1.0, 100.0));
            ui.horizontal(|ui| {
                load = ui.button("Load").clicked();
                invert = ui.button("Invert").clicked();
                remove = ui.button("Remove").clicked();
            });

            ui.add(egui::widgets::Image::new(
                *rendered_texture_id,
                [256.0, 256.0],
            ));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add(egui::Hyperlink::from_label_and_url(
                    "powered by egui",
                    "https://github.com/emilk/egui/",
                ));
            });
        });

    egui::TopBottomPanel::top("top_panel").show(egui_ctx.ctx_mut(), |ui| {
        // The top panel is often a good place for a menu bar:
        egui::menu::bar(ui, |ui| {
            egui::menu::menu_button(ui, "File", |ui| {
                if ui.button("Quit").clicked() {
                    std::process::exit(0);
                }
            });
        });
    });

    egui::CentralPanel::default().show(egui_ctx.ctx_mut(), |ui| {
        ui.heading("Egui Template");
        ui.hyperlink("https://github.com/emilk/egui_template");
        ui.add(egui::github_link_file_line!(
            "https://github.com/mvlabat/bevy_egui/blob/main/",
            "Direct link to source code."
        ));
        egui::warn_if_debug_build(ui);

        ui.separator();

        ui.heading("Central Panel");
        ui.label("The central panel the region left after adding TopPanel's and SidePanel's");
        ui.label("It is often a great place for big things, like drawings:");

        ui.heading("Draw with your mouse to paint:");
        ui_state.painting.ui_control(ui);
        egui::Frame::dark_canvas(ui.style()).show(ui, |ui| {
            ui_state.painting.ui_content(ui);
        });
    });

    egui::Window::new("Window")
        .vscroll(true)
        .show(egui_ctx.ctx_mut(), |ui| {
            ui.label("Windows can be moved by dragging them.");
            ui.label("They are automatically sized based on contents.");
            ui.label("You can turn on resizing and scrolling if you like.");
            ui.label("You would normally chose either panels OR windows.");
        });

    if invert {
        ui_state.inverted = !ui_state.inverted;
    }
    if load || invert {
        // If an image is already added to the context, it'll return an existing texture id.
        if ui_state.inverted {
            *rendered_texture_id = egui_ctx.add_image(images.bevy_icon_inverted.clone_weak());
        } else {
            *rendered_texture_id = egui_ctx.add_image(images.bevy_icon.clone_weak());
        };
    }
    if remove {
        egui_ctx.remove_image(&images.bevy_icon);
        egui_ctx.remove_image(&images.bevy_icon_inverted);
    }
}

struct Painting {
    lines: Vec<Vec<egui::Vec2>>,
    stroke: egui::Stroke,
}

impl Default for Painting {
    fn default() -> Self {
        Self {
            lines: Default::default(),
            stroke: egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE),
        }
    }
}

impl Painting {
    pub fn ui_control(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            egui::stroke_ui(ui, &mut self.stroke, "Stroke");
            ui.separator();
            if ui.button("Clear Painting").clicked() {
                self.lines.clear();
            }
        })
        .response
    }

    pub fn ui_content(&mut self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());
        let rect = response.rect;

        if self.lines.is_empty() {
            self.lines.push(vec![]);
        }

        let current_line = self.lines.last_mut().unwrap();

        if let Some(pointer_pos) = response.interact_pointer_pos() {
            let canvas_pos = pointer_pos - rect.min;
            if current_line.last() != Some(&canvas_pos) {
                current_line.push(canvas_pos);
            }
        } else if !current_line.is_empty() {
            self.lines.push(vec![]);
        }

        for line in &self.lines {
            if line.len() >= 2 {
                let points: Vec<egui::Pos2> = line.iter().map(|p| rect.min + *p).collect();
                painter.add(egui::Shape::line(points, self.stroke));
            }
        }
    }
}
