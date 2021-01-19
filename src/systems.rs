use crate::{EguiContext, EguiInput, EguiOutput, EguiSettings, EguiShapes, WindowSize};
use bevy::{
    app::EventReader,
    core::Time,
    ecs::{Res, ResMut},
    input::{
        keyboard::KeyCode,
        mouse::{MouseButton, MouseScrollUnit, MouseWheel},
        Input,
    },
    log,
    window::{CursorLeft, CursorMoved, ReceivedCharacter, Windows},
};
use bevy_winit::WinitWindows;

#[allow(clippy::too_many_arguments)]
pub fn process_input(
    mut egui_context: ResMut<EguiContext>,
    mut egui_input: ResMut<EguiInput>,
    #[cfg(feature = "manage_clipboard")] egui_clipboard: Res<crate::EguiClipboard>,
    mut ev_cursor_left: EventReader<CursorLeft>,
    mut ev_cursor_moved: EventReader<CursorMoved>,
    mut ev_mouse_wheel: EventReader<MouseWheel>,
    mut ev_received_character: EventReader<ReceivedCharacter>,
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

    egui_input.raw_input.screen_rect = Some(egui::Rect::from_min_max(
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
    egui_input.raw_input.pixels_per_point =
        Some(window_size.scale_factor * egui_settings.scale_factor as f32);

    for event in ev_mouse_wheel.iter() {
        let mut delta = egui::vec2(event.x, event.y);
        if let MouseScrollUnit::Line = event.unit {
            // TODO: https://github.com/emilk/egui/blob/b869db728b6bbefa098ac987a796b2b0b836c7cd/egui_glium/src/lib.rs#L141
            delta *= 24.0;
        }
        egui_input.raw_input.scroll_delta += delta;
    }

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

    for cursor_entered in ev_cursor_left.iter() {
        if cursor_entered.id.is_primary() {
            egui_input.raw_input.events.push(egui::Event::PointerGone);
            egui_context.mouse_position = None;
        }
    }
    if let Some(cursor_moved) = &ev_cursor_moved.iter().last() {
        if cursor_moved.id.is_primary() {
            let scale_factor = egui_settings.scale_factor as f32;
            let mut mouse_position: (f32, f32) = (cursor_moved.position / scale_factor).into();
            mouse_position.1 = window_size.height() / scale_factor - mouse_position.1;
            egui_context.mouse_position = Some(mouse_position);
            egui_input
                .raw_input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(
                    mouse_position.0,
                    mouse_position.1,
                )));
        }
    }

    if let Some((x, y)) = egui_context.mouse_position {
        let pos = egui::pos2(x, y);
        process_mouse_button_event(
            &mut egui_input.raw_input.events,
            pos,
            modifiers,
            &mouse_button_input,
            MouseButton::Left,
        );
        process_mouse_button_event(
            &mut egui_input.raw_input.events,
            pos,
            modifiers,
            &mouse_button_input,
            MouseButton::Right,
        );
        process_mouse_button_event(
            &mut egui_input.raw_input.events,
            pos,
            modifiers,
            &mouse_button_input,
            MouseButton::Middle,
        );
    }

    if !ctrl && !win {
        for event in ev_received_character.iter() {
            if event.id.is_primary() && !event.char.is_control() {
                egui_input
                    .raw_input
                    .events
                    .push(egui::Event::Text(event.char.to_string()));
            }
        }
    }

    for pressed_key in keyboard_input.get_just_pressed() {
        if let Some(key) = bevy_to_egui_key(*pressed_key) {
            egui_input.raw_input.events.push(egui::Event::Key {
                key,
                pressed: true,
                modifiers,
            })
        }
    }
    for pressed_key in keyboard_input.get_just_released() {
        if let Some(key) = bevy_to_egui_key(*pressed_key) {
            egui_input.raw_input.events.push(egui::Event::Key {
                key,
                pressed: false,
                modifiers,
            })
        }
    }

    #[cfg(feature = "manage_clipboard")]
    {
        if command && keyboard_input.just_pressed(KeyCode::C) {
            egui_input.raw_input.events.push(egui::Event::Copy);
        }
        if command && keyboard_input.just_pressed(KeyCode::X) {
            egui_input.raw_input.events.push(egui::Event::Cut);
        }
        if command && keyboard_input.just_pressed(KeyCode::V) {
            if let Some(contents) = egui_clipboard.get_contents() {
                egui_input
                    .raw_input
                    .events
                    .push(egui::Event::Text(contents))
            }
        }
    };

    egui_input.raw_input.predicted_dt = time.delta_seconds();
    egui_input.raw_input.modifiers = modifiers;
}

pub fn begin_frame(mut egui_context: ResMut<EguiContext>, mut egui_input: ResMut<EguiInput>) {
    let raw_input = egui_input.raw_input.take();
    egui_context.ctx.begin_frame(raw_input);
}

pub fn process_output(
    egui_context: Res<EguiContext>,
    mut egui_output: ResMut<EguiOutput>,
    mut egui_shapes: ResMut<EguiShapes>,
    #[cfg(feature = "manage_clipboard")] mut egui_clipboard: ResMut<crate::EguiClipboard>,
    windows: Res<Windows>,
    winit_windows: Res<WinitWindows>,
) {
    let (output, shapes) = egui_context.ctx.end_frame();
    egui_shapes.shapes = shapes;
    egui_output.output = output.clone();

    #[cfg(feature = "manage_clipboard")]
    if !output.copied_text.is_empty() {
        egui_clipboard.set_contents(&output.copied_text);
    }

    if let Some(window) = windows.get_primary() {
        if let Some(winit_window) = winit_windows.get_window(window.id()) {
            winit_window.set_cursor_icon(egui_to_winit_cursor_icon(output.cursor_icon));
        } else {
            log::error!("No winit window found for the primary window");
        }
    } else {
        log::warn!("No primary window detected");
    }

    #[cfg(feature = "open_url")]
    if let Some(url) = output.open_url {
        if let Err(err) = webbrowser::open(&url) {
            log::error!("Failed to open '{}': {:?}", url, err);
        }
    }
}

fn egui_to_winit_cursor_icon(cursor_icon: egui::CursorIcon) -> winit::window::CursorIcon {
    match cursor_icon {
        egui::CursorIcon::Default => winit::window::CursorIcon::Default,
        egui::CursorIcon::PointingHand => winit::window::CursorIcon::Hand,
        egui::CursorIcon::ResizeHorizontal => winit::window::CursorIcon::EwResize,
        egui::CursorIcon::ResizeNeSw => winit::window::CursorIcon::NeswResize,
        egui::CursorIcon::ResizeNwSe => winit::window::CursorIcon::NwseResize,
        egui::CursorIcon::ResizeVertical => winit::window::CursorIcon::NsResize,
        egui::CursorIcon::Text => winit::window::CursorIcon::Text,
        egui::CursorIcon::Grab => winit::window::CursorIcon::Grab,
        egui::CursorIcon::Grabbing => winit::window::CursorIcon::Grabbing,
    }
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

fn process_mouse_button_event(
    egui_events: &mut Vec<egui::Event>,
    pos: egui::Pos2,
    modifiers: egui::Modifiers,
    mouse_button_input: &Input<MouseButton>,
    mouse_button: MouseButton,
) {
    let button = match mouse_button {
        MouseButton::Left => egui::PointerButton::Primary,
        MouseButton::Right => egui::PointerButton::Secondary,
        MouseButton::Middle => egui::PointerButton::Middle,
        _ => panic!("Unsupported mouse button"),
    };

    let pressed = if mouse_button_input.just_pressed(mouse_button) {
        true
    } else if mouse_button_input.just_released(mouse_button) {
        false
    } else {
        return;
    };
    egui_events.push(egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    });
}
