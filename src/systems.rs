use crate::{EguiContext, EguiSettings, WindowSize};
use bevy::{
    app::Events,
    core::Time,
    ecs::{Res, ResMut},
    input::{keyboard::KeyCode, mouse::MouseButton, Input},
    window::{CursorMoved, ReceivedCharacter, Windows},
};

#[allow(clippy::too_many_arguments)]
pub fn process_input(
    mut egui_context: ResMut<EguiContext>,
    ev_cursor: Res<Events<CursorMoved>>,
    ev_received_character: Res<Events<ReceivedCharacter>>,
    mouse_button_input: Res<Input<MouseButton>>,
    keyboard_input: Res<Input<KeyCode>>,
    mut window_size: ResMut<WindowSize>,
    windows: ResMut<Windows>,
    egui_settings: ResMut<EguiSettings>,
    time: Res<Time>,
) {
    if let Some(window) = windows.get_primary() {
        *window_size = WindowSize::new(
            window.physical_width() as f32,
            window.physical_height() as f32,
            window.scale_factor() as f32,
        );
    }

    if let Some(cursor_moved) = egui_context.cursor.latest(&ev_cursor) {
        if cursor_moved.id.is_primary() {
            let scale_factor = egui_settings.scale_factor as f32;
            let mut mouse_position: (f32, f32) = (cursor_moved.position / scale_factor).into();
            mouse_position.1 = window_size.height() / scale_factor - mouse_position.1;
            egui_context.mouse_position = mouse_position;
            egui_context.raw_input.mouse_pos = Some(egui::pos2(mouse_position.0, mouse_position.1));
        }
    }

    egui_context.raw_input.mouse_down = mouse_button_input.pressed(MouseButton::Left);
    egui_context.raw_input.screen_rect = Some(egui::Rect::from_min_max(
        egui::pos2(0.0, 0.0),
        egui::pos2(
            window_size.physical_width
                / window_size.scale_factor
                / egui_settings.scale_factor as f32,
            window_size.physical_height
                / window_size.scale_factor
                / egui_settings.scale_factor as f32,
        ),
    ));
    egui_context.raw_input.pixels_per_point =
        Some(window_size.scale_factor * egui_settings.scale_factor as f32);

    let shift = keyboard_input.pressed(KeyCode::LShift) || keyboard_input.pressed(KeyCode::RShift);
    let ctrl =
        keyboard_input.pressed(KeyCode::LControl) || keyboard_input.pressed(KeyCode::RControl);
    let alt = keyboard_input.pressed(KeyCode::LAlt) || keyboard_input.pressed(KeyCode::RAlt);

    for event in egui_context.received_character.iter(&ev_received_character) {
        if event.id.is_primary() && !event.char.is_control() {
            egui_context
                .raw_input
                .events
                .push(egui::Event::Text(event.char.to_string()));
        }
    }

    let modifiers = egui::Modifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: false, // TODO: figure out how to detect this.
        command: ctrl,
    };

    if keyboard_input.pressed(KeyCode::Up) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowUp,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::Down) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowDown,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::Right) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowRight,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::Left) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowLeft,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::Home) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::Home,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::End) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::End,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::Delete) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::Delete,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::Back) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::Backspace,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::Return) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::Tab) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::Tab,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::A) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::A,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::K) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::K,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::K) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::K,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::U) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::U,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::W) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::W,
            pressed: true,
            modifiers,
        });
    }
    if keyboard_input.pressed(KeyCode::Z) {
        egui_context.raw_input.events.push(egui::Event::Key {
            key: egui::Key::Z,
            pressed: true,
            modifiers,
        });
    }
    egui_context.raw_input.predicted_dt = time.delta_seconds();
    egui_context.raw_input.modifiers = modifiers;
}

pub fn begin_frame(mut egui_context: ResMut<EguiContext>) {
    let raw_input = egui_context.raw_input.take();
    egui_context.ctx.begin_frame(raw_input);
}
