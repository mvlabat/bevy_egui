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
    let win = keyboard_input.pressed(KeyCode::LWin) || keyboard_input.pressed(KeyCode::RWin);

    let mac_cmd = if cfg!(target_os = "macos") {
        win
    } else {
        false
    };
    let command = if cfg!(target_os = "macos") { win } else { ctrl };

    let modifiers = egui::Modifiers {
        alt,
        ctrl,
        shift,
        mac_cmd,
        command,
    };

    if !ctrl && !win {
        for event in egui_context.received_character.iter(&ev_received_character) {
            if event.id.is_primary() && !event.char.is_control() {
                egui_context
                    .raw_input
                    .events
                    .push(egui::Event::Text(event.char.to_string()));
            }
        }
    }

    for pressed_key in keyboard_input.get_just_pressed() {
        if let Some(key) = bevy_to_egui_key(*pressed_key) {
            egui_context.raw_input.events.push(egui::Event::Key {
                key,
                pressed: true,
                modifiers,
            })
        }
    }
    for pressed_key in keyboard_input.get_just_released() {
        if let Some(key) = bevy_to_egui_key(*pressed_key) {
            egui_context.raw_input.events.push(egui::Event::Key {
                key,
                pressed: false,
                modifiers,
            })
        }
    }

    egui_context.raw_input.predicted_dt = time.delta_seconds();
    egui_context.raw_input.modifiers = modifiers;
}

pub fn begin_frame(mut egui_context: ResMut<EguiContext>) {
    let raw_input = egui_context.raw_input.take();
    egui_context.ctx.begin_frame(raw_input);
}

fn bevy_to_egui_key(key_code: KeyCode) -> Option<egui::Key> {
    let key = match key_code {
        KeyCode::Down => egui::Key::ArrowDown,
        KeyCode::Left => egui::Key::ArrowLeft,
        KeyCode::Right => egui::Key::ArrowRight,
        KeyCode::Up => egui::Key::ArrowUp,
        KeyCode::Escape => egui::Key::Escape,
        KeyCode::Tab => egui::Key::Tab,
        KeyCode::Back => egui::Key::Backspace,
        KeyCode::Return => egui::Key::Enter,
        KeyCode::Space => egui::Key::Space,
        KeyCode::Insert => egui::Key::Insert,
        KeyCode::Delete => egui::Key::Delete,
        KeyCode::Home => egui::Key::Home,
        KeyCode::End => egui::Key::End,
        KeyCode::PageUp => egui::Key::PageUp,
        KeyCode::PageDown => egui::Key::PageDown,
        KeyCode::Numpad0 | KeyCode::Key0 => egui::Key::Num0,
        KeyCode::Numpad1 | KeyCode::Key1 => egui::Key::Num1,
        KeyCode::Numpad2 | KeyCode::Key2 => egui::Key::Num2,
        KeyCode::Numpad3 | KeyCode::Key3 => egui::Key::Num3,
        KeyCode::Numpad4 | KeyCode::Key4 => egui::Key::Num4,
        KeyCode::Numpad5 | KeyCode::Key5 => egui::Key::Num5,
        KeyCode::Numpad6 | KeyCode::Key6 => egui::Key::Num6,
        KeyCode::Numpad7 | KeyCode::Key7 => egui::Key::Num7,
        KeyCode::Numpad8 | KeyCode::Key8 => egui::Key::Num8,
        KeyCode::Numpad9 | KeyCode::Key9 => egui::Key::Num9,
        KeyCode::A => egui::Key::A,
        KeyCode::B => egui::Key::B,
        KeyCode::C => egui::Key::C,
        KeyCode::D => egui::Key::D,
        KeyCode::E => egui::Key::E,
        KeyCode::F => egui::Key::F,
        KeyCode::G => egui::Key::G,
        KeyCode::H => egui::Key::H,
        KeyCode::I => egui::Key::I,
        KeyCode::J => egui::Key::J,
        KeyCode::K => egui::Key::K,
        KeyCode::L => egui::Key::L,
        KeyCode::M => egui::Key::M,
        KeyCode::N => egui::Key::N,
        KeyCode::O => egui::Key::O,
        KeyCode::P => egui::Key::P,
        KeyCode::Q => egui::Key::Q,
        KeyCode::R => egui::Key::R,
        KeyCode::S => egui::Key::S,
        KeyCode::T => egui::Key::T,
        KeyCode::U => egui::Key::U,
        KeyCode::V => egui::Key::V,
        KeyCode::W => egui::Key::W,
        KeyCode::X => egui::Key::X,
        KeyCode::Y => egui::Key::Y,
        KeyCode::Z => egui::Key::Z,
        _ => return None,
    };
    Some(key)
}
