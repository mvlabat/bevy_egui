use crate::{
    EguiContext, EguiInput, EguiOutputContainer, EguiRenderInputContainer,
    EguiRenderOutputContainer, EguiSettings, EguiWindowSizeContainer, WindowSize,
};
#[cfg(feature = "open_url")]
use bevy::log;
use bevy::{
    ecs::{
        event::EventWriter,
        system::{Local, Res, ResMut, SystemParam},
    },
    input::{
        keyboard::{KeyCode, KeyboardInput},
        mouse::{MouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel},
        ButtonState, Input,
    },
    prelude::{Entity, EventReader, Query, Time},
    utils::HashMap,
    window::{
        CursorEntered, CursorLeft, CursorMoved, ReceivedCharacter, RequestRedraw, Window,
        WindowCreated, WindowFocused,
    },
};
use std::marker::PhantomData;

#[allow(missing_docs)]
#[derive(SystemParam)]
pub struct InputEvents<'w, 's> {
    pub ev_cursor_entered: EventReader<'w, 's, CursorEntered>,
    pub ev_cursor_left: EventReader<'w, 's, CursorLeft>,
    pub ev_cursor: EventReader<'w, 's, CursorMoved>,
    pub ev_mouse_button_input: EventReader<'w, 's, MouseButtonInput>,
    pub ev_mouse_wheel: EventReader<'w, 's, MouseWheel>,
    pub ev_received_character: EventReader<'w, 's, ReceivedCharacter>,
    pub ev_keyboard_input: EventReader<'w, 's, KeyboardInput>,
    pub ev_window_focused: EventReader<'w, 's, WindowFocused>,
    pub ev_window_created: EventReader<'w, 's, WindowCreated>,
}

impl<'w, 's> InputEvents<'w, 's> {
    /// Consumes all the events.
    pub fn clear(&mut self) {
        self.ev_cursor_entered.iter().last();
        self.ev_cursor_left.iter().last();
        self.ev_cursor.iter().last();
        self.ev_mouse_button_input.iter().last();
        self.ev_mouse_wheel.iter().last();
        self.ev_received_character.iter().last();
        self.ev_keyboard_input.iter().last();
        self.ev_window_focused.iter().last();
        self.ev_window_created.iter().last();
    }
}

#[allow(missing_docs)]
#[derive(SystemParam)]
pub struct InputResources<'w, 's> {
    #[cfg(feature = "manage_clipboard")]
    pub egui_clipboard: Res<'w, crate::EguiClipboard>,
    pub keyboard_input: Res<'w, Input<KeyCode>>,
    pub egui_input: ResMut<'w, EguiRenderInputContainer>,
    #[system_param(ignore)]
    _marker: PhantomData<&'s ()>,
}

#[allow(missing_docs)]
#[derive(SystemParam)]
pub struct WindowResources<'w, 's> {
    pub focused_window: Local<'s, Option<Entity>>,
    pub windows: Query<'w, 's, (Entity, &'static mut Window)>,
    pub window_sizes: ResMut<'w, EguiWindowSizeContainer>,
    #[system_param(ignore)]
    _marker: PhantomData<&'s ()>,
}

/// Initialises Egui contexts (for multiple windows) on startup.
pub fn init_contexts_startup_system(
    mut egui_context: ResMut<EguiContext>,
    mut egui_input: ResMut<EguiRenderInputContainer>,
    mut window_resources: WindowResources,
    egui_settings: Res<EguiSettings>,
) {
    update_window_contexts(
        &mut egui_context,
        &mut egui_input.0,
        &mut window_resources,
        &egui_settings,
    );
}

/// Processes Bevy input and feeds it to Egui.
pub fn process_input_system(
    mut egui_context: ResMut<EguiContext>,
    mut input_events: InputEvents,
    mut input_resources: InputResources,
    mut window_resources: WindowResources,
    egui_settings: Res<EguiSettings>,
    time: Res<Time>,
) {
    // This is a workaround for Windows. For some reason, `WindowFocused` event isn't fired
    // when a window is created.
    if let Some(event) = input_events.ev_window_created.iter().last() {
        *window_resources.focused_window = Some(event.window);
    }

    for event in input_events.ev_window_focused.iter() {
        *window_resources.focused_window = if event.focused {
            Some(event.window)
        } else {
            None
        };
    }

    update_window_contexts(
        &mut egui_context,
        &mut input_resources.egui_input.0,
        &mut window_resources,
        &egui_settings,
    );

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

    let mut cursor_left_window = None;
    if let Some(cursor_left) = input_events.ev_cursor_left.iter().last() {
        cursor_left_window = Some(cursor_left.window);
    }
    let cursor_entered_window = input_events
        .ev_cursor_entered
        .iter()
        .last()
        .map(|event| event.window);

    // When a user releases a mouse button, Safari emits both `CursorLeft` and `CursorEntered`
    // events during the same frame. We don't want to reset mouse position in such a case, otherwise
    // we won't be able to process the mouse button event.
    let prev_mouse_position =
        if cursor_left_window.is_some() && cursor_left_window != cursor_entered_window {
            // If it's not the Safari edge case, reset the mouse position.
            egui_context.mouse_position.take()
        } else {
            None
        };

    if let Some(cursor_moved) = input_events.ev_cursor.iter().last() {
        // If we've left the window, it's unlikely that we've moved the cursor back to the same
        // window this exact frame, so we are safe to ignore all `CursorMoved` events for the window
        // that has been left.
        if cursor_left_window != Some(cursor_moved.window) {
            let scale_factor = egui_settings.scale_factor as f32;
            let mut mouse_position: (f32, f32) = (cursor_moved.position / scale_factor).into();
            mouse_position.1 = window_resources.window_sizes[&cursor_moved.window].height()
                / scale_factor
                - mouse_position.1;
            egui_context.mouse_position = Some((cursor_moved.window, mouse_position.into()));
            input_resources
                .egui_input
                .get_mut(&cursor_moved.window)
                .unwrap()
                .events
                .push(egui::Event::PointerMoved(egui::pos2(
                    mouse_position.0,
                    mouse_position.1,
                )));
        }
    }

    // If we pressed a button, started dragging a cursor inside a window and released
    // the button when being outside, some platforms will fire `CursorLeft` again together
    // with `MouseButtonInput` - this is why we also take `prev_mouse_position` into account.
    if let Some((window_id, position)) = egui_context
        .mouse_position
        .as_ref()
        .or(prev_mouse_position.as_ref())
    {
        if let Some(egui_input) = input_resources.egui_input.get_mut(window_id) {
            let events = &mut egui_input.events;

            for mouse_button_event in input_events.ev_mouse_button_input.iter() {
                let button = match mouse_button_event.button {
                    MouseButton::Left => Some(egui::PointerButton::Primary),
                    MouseButton::Right => Some(egui::PointerButton::Secondary),
                    MouseButton::Middle => Some(egui::PointerButton::Middle),
                    _ => None,
                };
                let pressed = match mouse_button_event.state {
                    ButtonState::Pressed => true,
                    ButtonState::Released => false,
                };
                if let Some(button) = button {
                    events.push(egui::Event::PointerButton {
                        pos: position.to_pos2(),
                        button,
                        pressed,
                        modifiers,
                    });
                }
            }

            for event in input_events.ev_mouse_wheel.iter() {
                let mut delta = egui::vec2(event.x, event.y);
                if let MouseScrollUnit::Line = event.unit {
                    // https://github.com/emilk/egui/blob/a689b623a669d54ea85708a8c748eb07e23754b0/egui-winit/src/lib.rs#L449
                    delta *= 50.0;
                }

                if ctrl || mac_cmd {
                    // Treat as zoom instead.
                    let factor = (delta.y / 200.0).exp();
                    events.push(egui::Event::Zoom(factor));
                } else if shift {
                    // Treat as horizontal scrolling.
                    // Note: Mac already fires horizontal scroll events when shift is down.
                    events.push(egui::Event::Scroll(egui::vec2(delta.x + delta.y, 0.0)));
                } else {
                    events.push(egui::Event::Scroll(delta));
                }
            }
        }
    }

    if !ctrl && !win {
        for event in input_events.ev_received_character.iter() {
            if !event.char.is_control() {
                input_resources
                    .egui_input
                    .get_mut(&event.window)
                    .unwrap()
                    .events
                    .push(egui::Event::Text(event.char.to_string()));
            }
        }
    }

    if let Some(focused_input) = window_resources
        .focused_window
        .as_ref()
        .and_then(|window_id| input_resources.egui_input.get_mut(window_id))
    {
        for ev in input_events.ev_keyboard_input.iter() {
            if let Some(key) = ev.key_code.and_then(bevy_to_egui_key) {
                let pressed = match ev.state {
                    ButtonState::Pressed => true,
                    ButtonState::Released => false,
                };
                let egui_event = egui::Event::Key {
                    key,
                    pressed,
                    repeat: false,
                    modifiers,
                };
                focused_input.events.push(egui_event);

                // We also check that it's an `ButtonState::Pressed` event, as we don't want to
                // copy, cut or paste on the key release.
                #[cfg(feature = "manage_clipboard")]
                if command && pressed {
                    match key {
                        egui::Key::C => {
                            focused_input.events.push(egui::Event::Copy);
                        }
                        egui::Key::X => {
                            focused_input.events.push(egui::Event::Cut);
                        }
                        egui::Key::V => {
                            if let Some(contents) = input_resources.egui_clipboard.get_contents() {
                                focused_input.events.push(egui::Event::Text(contents))
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        focused_input.modifiers = modifiers;
    }

    for egui_input in input_resources.egui_input.values_mut() {
        egui_input.predicted_dt = time.delta_seconds();
    }

    // In some cases, we may skip certain events. For example, we ignore `ReceivedCharacter` events
    // when alt or ctrl button is pressed. We still want to clear event buffer.
    input_events.clear();
}

fn update_window_contexts(
    egui_context: &mut EguiContext,
    egui_input: &mut HashMap<Entity, EguiInput>,
    window_resources: &mut WindowResources,
    egui_settings: &EguiSettings,
) {
    for (window_entity, window) in window_resources.windows.iter() {
        let egui_input = egui_input.entry(window_entity).or_default();

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

        egui_input.screen_rect = Some(egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(width, height),
        ));

        egui_input.pixels_per_point =
            Some(window_size.scale_factor * egui_settings.scale_factor as f32);

        window_resources
            .window_sizes
            .insert(window_entity, window_size);
        egui_context.ctx.entry(window_entity).or_default();
    }
}

/// Marks frame start for Egui.
pub fn begin_frame_system(
    mut egui_context: ResMut<EguiContext>,
    mut egui_input: ResMut<EguiRenderInputContainer>,
) {
    for (id, ctx) in egui_context.ctx.iter_mut() {
        let raw_input = egui_input.get_mut(id).unwrap().take();
        ctx.begin_frame(raw_input);
    }
}

#[allow(missing_docs)]
#[derive(SystemParam)]
pub struct OutputResources<'w, 's> {
    pub egui: ResMut<'w, EguiOutputContainer>,
    pub egui_render: ResMut<'w, EguiRenderOutputContainer>,
    #[system_param(ignore)]
    _marker: PhantomData<&'s ()>,
}

/// Reads Egui output.
pub fn process_output_system(
    #[cfg_attr(not(feature = "open_url"), allow(unused_variables))] egui_settings: Res<
        EguiSettings,
    >,
    mut egui_context: ResMut<EguiContext>,
    mut output: OutputResources,
    #[cfg(feature = "manage_clipboard")] mut egui_clipboard: ResMut<crate::EguiClipboard>,
    mut windows: Query<&mut Window>,
    mut event: EventWriter<RequestRedraw>,
    #[cfg(windows)] mut last_cursor_icon: Local<HashMap<Entity, egui::CursorIcon>>,
) {
    for (window_id, ctx) in egui_context.ctx.iter_mut() {
        let full_output = ctx.end_frame();
        let egui::FullOutput {
            platform_output,
            shapes,
            textures_delta,
            repaint_after,
        } = full_output;

        let egui_render_output = output.egui_render.entry(*window_id).or_default();
        egui_render_output.shapes = shapes;
        egui_render_output.textures_delta.append(textures_delta);

        output.egui.entry(*window_id).or_default().platform_output = platform_output.clone();

        #[cfg(feature = "manage_clipboard")]
        if !platform_output.copied_text.is_empty() {
            egui_clipboard.set_contents(&platform_output.copied_text);
        }

        if let Ok(mut window) = windows.get_mut(*window_id) {
            let mut set_icon = || {
                window.cursor.icon = egui_to_winit_cursor_icon(platform_output.cursor_icon)
                    .unwrap_or(bevy::window::CursorIcon::Default);
            };

            #[cfg(windows)]
            {
                let last_cursor_icon = last_cursor_icon.entry(*window_id).or_default();
                if *last_cursor_icon != platform_output.cursor_icon {
                    set_icon();
                    *last_cursor_icon = platform_output.cursor_icon;
                }
            }
            #[cfg(not(windows))]
            set_icon();
        }

        if repaint_after.is_zero() {
            event.send(RequestRedraw)
        }

        #[cfg(feature = "open_url")]
        if let Some(egui::output::OpenUrl { url, new_tab }) = platform_output.open_url {
            let target = if new_tab {
                "_blank"
            } else {
                egui_settings
                    .default_open_url_target
                    .as_deref()
                    .unwrap_or("_self")
            };
            if let Err(err) = webbrowser::open_browser_with_options(
                webbrowser::Browser::Default,
                &url,
                webbrowser::BrowserOptions::new().with_target_hint(target),
            ) {
                log::error!("Failed to open '{}': {:?}", url, err);
            }
        }
    }
}

fn egui_to_winit_cursor_icon(cursor_icon: egui::CursorIcon) -> Option<bevy::window::CursorIcon> {
    match cursor_icon {
        egui::CursorIcon::Default => Some(bevy::window::CursorIcon::Default),
        egui::CursorIcon::PointingHand => Some(bevy::window::CursorIcon::Hand),
        egui::CursorIcon::ResizeHorizontal => Some(bevy::window::CursorIcon::EwResize),
        egui::CursorIcon::ResizeNeSw => Some(bevy::window::CursorIcon::NeswResize),
        egui::CursorIcon::ResizeNwSe => Some(bevy::window::CursorIcon::NwseResize),
        egui::CursorIcon::ResizeVertical => Some(bevy::window::CursorIcon::NsResize),
        egui::CursorIcon::Text => Some(bevy::window::CursorIcon::Text),
        egui::CursorIcon::Grab => Some(bevy::window::CursorIcon::Grab),
        egui::CursorIcon::Grabbing => Some(bevy::window::CursorIcon::Grabbing),
        egui::CursorIcon::ContextMenu => Some(bevy::window::CursorIcon::ContextMenu),
        egui::CursorIcon::Help => Some(bevy::window::CursorIcon::Help),
        egui::CursorIcon::Progress => Some(bevy::window::CursorIcon::Progress),
        egui::CursorIcon::Wait => Some(bevy::window::CursorIcon::Wait),
        egui::CursorIcon::Cell => Some(bevy::window::CursorIcon::Cell),
        egui::CursorIcon::Crosshair => Some(bevy::window::CursorIcon::Crosshair),
        egui::CursorIcon::VerticalText => Some(bevy::window::CursorIcon::VerticalText),
        egui::CursorIcon::Alias => Some(bevy::window::CursorIcon::Alias),
        egui::CursorIcon::Copy => Some(bevy::window::CursorIcon::Copy),
        egui::CursorIcon::Move => Some(bevy::window::CursorIcon::Move),
        egui::CursorIcon::NoDrop => Some(bevy::window::CursorIcon::NoDrop),
        egui::CursorIcon::NotAllowed => Some(bevy::window::CursorIcon::NotAllowed),
        egui::CursorIcon::AllScroll => Some(bevy::window::CursorIcon::AllScroll),
        egui::CursorIcon::ZoomIn => Some(bevy::window::CursorIcon::ZoomIn),
        egui::CursorIcon::ZoomOut => Some(bevy::window::CursorIcon::ZoomOut),
        egui::CursorIcon::ResizeEast => Some(bevy::window::CursorIcon::EResize),
        egui::CursorIcon::ResizeSouthEast => Some(bevy::window::CursorIcon::SeResize),
        egui::CursorIcon::ResizeSouth => Some(bevy::window::CursorIcon::SResize),
        egui::CursorIcon::ResizeSouthWest => Some(bevy::window::CursorIcon::SwResize),
        egui::CursorIcon::ResizeWest => Some(bevy::window::CursorIcon::WResize),
        egui::CursorIcon::ResizeNorthWest => Some(bevy::window::CursorIcon::NwResize),
        egui::CursorIcon::ResizeNorth => Some(bevy::window::CursorIcon::NResize),
        egui::CursorIcon::ResizeNorthEast => Some(bevy::window::CursorIcon::NeResize),
        egui::CursorIcon::ResizeColumn => Some(bevy::window::CursorIcon::ColResize),
        egui::CursorIcon::ResizeRow => Some(bevy::window::CursorIcon::RowResize),
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
