use std::marker::PhantomData;

use crate::{EguiContext, EguiInput, EguiOutput, EguiSettings, EguiShapes, WindowSize};
#[cfg(feature = "open_url")]
use bevy::log;
use bevy::{
    app::EventReader,
    core::Time,
    ecs::system::{Local, Res, ResMut, SystemParam},
    input::{
        keyboard::{KeyCode, KeyboardInput},
        mouse::{MouseButton, MouseScrollUnit, MouseWheel},
        ElementState, Input,
    },
    utils::HashMap,
    window::{
        CursorLeft, CursorMoved, ReceivedCharacter, WindowCreated, WindowFocused, WindowId, Windows,
    },
    winit::WinitWindows,
};

#[derive(SystemParam)]
pub struct InputEvents<'w, 's> {
    ev_cursor_left: EventReader<'w, 's, CursorLeft>,
    ev_cursor: EventReader<'w, 's, CursorMoved>,
    ev_mouse_wheel: EventReader<'w, 's, MouseWheel>,
    ev_received_character: EventReader<'w, 's, ReceivedCharacter>,
    ev_keyboard_input: EventReader<'w, 's, KeyboardInput>,
    ev_window_focused: EventReader<'w, 's, WindowFocused>,
    ev_window_created: EventReader<'w, 's, WindowCreated>,
}

#[derive(SystemParam)]
pub struct InputResources<'w, 's> {
    #[cfg(feature = "manage_clipboard")]
    egui_clipboard: Res<'w, crate::EguiClipboard>,
    mouse_button_input: Res<'w, Input<MouseButton>>,
    keyboard_input: Res<'w, Input<KeyCode>>,
    egui_input: ResMut<'w, HashMap<WindowId, EguiInput>>,
    #[system_param(ignore)]
    marker: PhantomData<&'s usize>,
}

#[derive(SystemParam)]
pub struct WindowResources<'w, 's> {
    focused_window: Local<'s, WindowId>,
    windows: ResMut<'w, Windows>,
    window_sizes: ResMut<'w, HashMap<WindowId, WindowSize>>,
    #[system_param(ignore)]
    _marker: PhantomData<&'s usize>,
}

pub fn init_contexts_on_startup(
    mut egui_context: ResMut<EguiContext>,
    mut egui_input: ResMut<HashMap<WindowId, EguiInput>>,
    mut window_resources: WindowResources,
    egui_settings: Res<EguiSettings>,
) {
    update_window_contexts(
        &mut egui_context,
        &mut egui_input,
        &mut window_resources,
        &egui_settings,
    );
}

pub fn process_input(
    mut egui_context: ResMut<EguiContext>,
    mut input_events: InputEvents,
    mut input_resources: InputResources,
    mut window_resources: WindowResources,
    egui_settings: Res<EguiSettings>,
    time: Res<Time>,
) {
    // This is a workaround for Windows. For some reason, `WindowFocused` event isn't fired.
    // when a window is created.
    for event in input_events.ev_window_created.iter().rev() {
        *window_resources.focused_window = event.id;
    }

    for event in input_events.ev_window_focused.iter().rev() {
        if event.focused {
            *window_resources.focused_window = event.id;
        }
    }

    update_window_contexts(
        &mut egui_context,
        &mut input_resources.egui_input,
        &mut window_resources,
        &egui_settings,
    );

    for event in input_events.ev_mouse_wheel.iter() {
        let mut delta = egui::vec2(event.x, event.y);
        if let MouseScrollUnit::Line = event.unit {
            // TODO: https://github.com/emilk/egui/blob/b869db728b6bbefa098ac987a796b2b0b836c7cd/egui_glium/src/lib.rs#L141
            delta *= 24.0;
        }

        for egui_input in input_resources.egui_input.values_mut() {
            egui_input.raw_input.scroll_delta += delta;
        }
    }

    let shift = input_resources.keyboard_input.pressed(KeyCode::LShift)
        || input_resources.keyboard_input.pressed(KeyCode::RShift);
    let ctrl = input_resources.keyboard_input.pressed(KeyCode::LControl)
        || input_resources.keyboard_input.pressed(KeyCode::RControl);
    let alt = input_resources.keyboard_input.pressed(KeyCode::LAlt)
        || input_resources.keyboard_input.pressed(KeyCode::RAlt);
    let win = input_resources.keyboard_input.pressed(KeyCode::LWin)
        || input_resources.keyboard_input.pressed(KeyCode::RWin);

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

    for cursor_entered in input_events.ev_cursor_left.iter() {
        input_resources
            .egui_input
            .get_mut(&cursor_entered.id)
            .unwrap()
            .raw_input
            .events
            .push(egui::Event::PointerGone);
        egui_context.mouse_position = None;
    }
    if let Some(cursor_moved) = input_events.ev_cursor.iter().next_back() {
        let scale_factor = egui_settings.scale_factor as f32;
        let mut mouse_position: (f32, f32) = (cursor_moved.position / scale_factor).into();
        mouse_position.1 = window_resources.window_sizes[&cursor_moved.id].height() / scale_factor
            - mouse_position.1;
        egui_context.mouse_position = Some(mouse_position);
        input_resources
            .egui_input
            .get_mut(&cursor_moved.id)
            .unwrap()
            .raw_input
            .events
            .push(egui::Event::PointerMoved(egui::pos2(
                mouse_position.0,
                mouse_position.1,
            )));
    }

    if let Some((x, y)) = egui_context.mouse_position {
        let focused_egui_input = input_resources
            .egui_input
            .get_mut(&*window_resources.focused_window)
            .unwrap();
        let events = &mut focused_egui_input.raw_input.events;

        let pos = egui::pos2(x, y);
        process_mouse_button_event(
            events,
            pos,
            modifiers,
            &input_resources.mouse_button_input,
            MouseButton::Left,
        );
        process_mouse_button_event(
            events,
            pos,
            modifiers,
            &input_resources.mouse_button_input,
            MouseButton::Right,
        );
        process_mouse_button_event(
            events,
            pos,
            modifiers,
            &input_resources.mouse_button_input,
            MouseButton::Middle,
        );
    }

    if !ctrl && !win {
        for event in input_events.ev_received_character.iter() {
            if !event.char.is_control() {
                input_resources
                    .egui_input
                    .get_mut(&event.id)
                    .unwrap()
                    .raw_input
                    .events
                    .push(egui::Event::Text(event.char.to_string()));
            }
        }
    }

    let focused_input = input_resources
        .egui_input
        .get_mut(&*window_resources.focused_window)
        .unwrap();

    for ev in input_events.ev_keyboard_input.iter() {
        if let Some(key) = ev.key_code.and_then(bevy_to_egui_key) {
            let egui_event = egui::Event::Key {
                key,
                pressed: match ev.state {
                    ElementState::Pressed => true,
                    ElementState::Released => false,
                },
                modifiers,
            };
            focused_input.raw_input.events.push(egui_event);

            #[cfg(feature = "manage_clipboard")]
            if command {
                match key {
                    egui::Key::C => {
                        focused_input.raw_input.events.push(egui::Event::Copy);
                    }
                    egui::Key::X => {
                        focused_input.raw_input.events.push(egui::Event::Cut);
                    }
                    egui::Key::V => {
                        if let Some(contents) = input_resources.egui_clipboard.get_contents() {
                            focused_input
                                .raw_input
                                .events
                                .push(egui::Event::Text(contents))
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    focused_input.raw_input.modifiers = modifiers;

    for egui_input in input_resources.egui_input.values_mut() {
        egui_input.raw_input.predicted_dt = time.delta_seconds();
    }
}

fn update_window_contexts(
    egui_context: &mut EguiContext,
    egui_input: &mut HashMap<WindowId, EguiInput>,
    window_resources: &mut WindowResources,
    egui_settings: &EguiSettings,
) {
    for window in window_resources.windows.iter() {
        let egui_input = egui_input.entry(window.id()).or_default();

        let window_size = WindowSize::new(
            window.physical_width() as f32,
            window.physical_height() as f32,
            window.scale_factor() as f32,
        );
        let width = window_size.physical_width
            / window_size.scale_factor
            / egui_settings.scale_factor as f32;
        let height = window_size.physical_height
            / window_size.scale_factor
            / egui_settings.scale_factor as f32;

        if width < 1.0 || height < 1.0 {
            continue;
        }

        egui_input.raw_input.screen_rect = Some(egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(width, height),
        ));

        egui_input.raw_input.pixels_per_point =
            Some(window_size.scale_factor * egui_settings.scale_factor as f32);

        window_resources
            .window_sizes
            .insert(window.id(), window_size);
        egui_context.ctx.entry(window.id()).or_default();
    }
}

pub fn begin_frame(
    mut egui_context: ResMut<EguiContext>,
    mut egui_input: ResMut<HashMap<WindowId, EguiInput>>,
) {
    let ids: Vec<_> = egui_context.ctx.keys().copied().collect();
    for id in ids {
        let raw_input = egui_input.get_mut(&id).unwrap().raw_input.take();
        egui_context
            .ctx
            .get_mut(&id)
            .unwrap()
            .begin_frame(raw_input);
    }
}

pub fn process_output(
    egui_context: Res<EguiContext>,
    mut egui_output: ResMut<HashMap<WindowId, EguiOutput>>,
    mut egui_shapes: ResMut<HashMap<WindowId, EguiShapes>>,
    #[cfg(feature = "manage_clipboard")] mut egui_clipboard: ResMut<crate::EguiClipboard>,
    winit_windows: Res<WinitWindows>,
) {
    for id in egui_context.ctx.keys().copied() {
        let (output, shapes) = egui_context.ctx_for_window(id).end_frame();
        egui_shapes.entry(id).or_default().shapes = shapes;
        egui_output.entry(id).or_default().output = output.clone();

        #[cfg(feature = "manage_clipboard")]
        if !output.copied_text.is_empty() {
            egui_clipboard.set_contents(&output.copied_text);
        }

        if let Some(winit_window) = winit_windows.get_window(id) {
            winit_window.set_cursor_icon(
                egui_to_winit_cursor_icon(output.cursor_icon)
                    .unwrap_or(winit::window::CursorIcon::Default),
            );
        }

        // TODO: see if we can support `new_tab`.
        #[cfg(feature = "open_url")]
        if let Some(egui::output::OpenUrl {
            url,
            new_tab: _new_tab,
        }) = output.open_url
        {
            if let Err(err) = webbrowser::open(&url) {
                log::error!("Failed to open '{}': {:?}", url, err);
            }
        }
    }
}

fn egui_to_winit_cursor_icon(cursor_icon: egui::CursorIcon) -> Option<winit::window::CursorIcon> {
    match cursor_icon {
        egui::CursorIcon::Default => Some(winit::window::CursorIcon::Default),
        egui::CursorIcon::PointingHand => Some(winit::window::CursorIcon::Hand),
        egui::CursorIcon::ResizeHorizontal => Some(winit::window::CursorIcon::EwResize),
        egui::CursorIcon::ResizeNeSw => Some(winit::window::CursorIcon::NeswResize),
        egui::CursorIcon::ResizeNwSe => Some(winit::window::CursorIcon::NwseResize),
        egui::CursorIcon::ResizeVertical => Some(winit::window::CursorIcon::NsResize),
        egui::CursorIcon::Text => Some(winit::window::CursorIcon::Text),
        egui::CursorIcon::Grab => Some(winit::window::CursorIcon::Grab),
        egui::CursorIcon::Grabbing => Some(winit::window::CursorIcon::Grabbing),
        egui::CursorIcon::ContextMenu => Some(winit::window::CursorIcon::ContextMenu),
        egui::CursorIcon::Help => Some(winit::window::CursorIcon::Help),
        egui::CursorIcon::Progress => Some(winit::window::CursorIcon::Progress),
        egui::CursorIcon::Wait => Some(winit::window::CursorIcon::Wait),
        egui::CursorIcon::Cell => Some(winit::window::CursorIcon::Cell),
        egui::CursorIcon::Crosshair => Some(winit::window::CursorIcon::Crosshair),
        egui::CursorIcon::VerticalText => Some(winit::window::CursorIcon::VerticalText),
        egui::CursorIcon::Alias => Some(winit::window::CursorIcon::Alias),
        egui::CursorIcon::Copy => Some(winit::window::CursorIcon::Copy),
        egui::CursorIcon::Move => Some(winit::window::CursorIcon::Move),
        egui::CursorIcon::NoDrop => Some(winit::window::CursorIcon::NoDrop),
        egui::CursorIcon::NotAllowed => Some(winit::window::CursorIcon::NotAllowed),
        egui::CursorIcon::AllScroll => Some(winit::window::CursorIcon::AllScroll),
        egui::CursorIcon::ZoomIn => Some(winit::window::CursorIcon::ZoomIn),
        egui::CursorIcon::ZoomOut => Some(winit::window::CursorIcon::ZoomOut),
        egui::CursorIcon::None => None,
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
