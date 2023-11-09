use crate::{
    EguiContext, EguiContextQuery, EguiInput, EguiMousePosition, EguiSettings, WindowSize,
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
        touch::TouchInput,
        ButtonState, Input,
    },
    prelude::{Entity, EventReader, Query, Resource, Time},
    time::Real,
    window::{
        CursorEntered, CursorLeft, CursorMoved, ReceivedCharacter, RequestRedraw, WindowCreated,
        WindowFocused,
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
    pub ev_touch: EventReader<'w, 's, TouchInput>,
}

impl<'w, 's> InputEvents<'w, 's> {
    /// Consumes all the events.
    pub fn clear(&mut self) {
        self.ev_touch.read().last();
        self.ev_cursor_entered.read().last();
        self.ev_cursor_left.read().last();
        self.ev_cursor.read().last();
        self.ev_mouse_button_input.read().last();
        self.ev_mouse_wheel.read().last();
        self.ev_received_character.read().last();
        self.ev_keyboard_input.read().last();
        self.ev_window_focused.read().last();
        self.ev_window_created.read().last();
    }
}

#[allow(missing_docs)]
#[derive(Resource, Default)]
pub struct TouchId(pub Option<u64>);

#[allow(missing_docs)]
#[derive(SystemParam)]
pub struct InputResources<'w, 's> {
    #[cfg(feature = "manage_clipboard")]
    pub egui_clipboard: Res<'w, crate::EguiClipboard>,
    pub keyboard_input: Res<'w, Input<KeyCode>>,
    #[system_param(ignore)]
    _marker: PhantomData<&'s ()>,
}

#[allow(missing_docs)]
#[derive(SystemParam)]
pub struct ContextSystemParams<'w, 's> {
    pub focused_window: Local<'s, Option<Entity>>,
    pub pointer_touch_id: Local<'s, TouchId>,
    pub contexts: Query<'w, 's, EguiContextQuery>,
    #[system_param(ignore)]
    _marker: PhantomData<&'s ()>,
}

/// Processes Bevy input and feeds it to Egui.
pub fn process_input_system(
    mut input_events: InputEvents,
    input_resources: InputResources,
    mut context_params: ContextSystemParams,
    egui_settings: Res<EguiSettings>,
    mut egui_mouse_position: ResMut<EguiMousePosition>,
    time: Res<Time<Real>>,
) {
    // This is a workaround for Windows. For some reason, `WindowFocused` event isn't fired
    // when a window is created.
    if let Some(event) = input_events.ev_window_created.read().last() {
        *context_params.focused_window = Some(event.window);
    }

    for event in input_events.ev_window_focused.read() {
        *context_params.focused_window = if event.focused {
            Some(event.window)
        } else {
            None
        };
    }

    let shift = input_resources.keyboard_input.pressed(KeyCode::ShiftLeft)
        || input_resources.keyboard_input.pressed(KeyCode::ShiftRight);
    let ctrl = input_resources.keyboard_input.pressed(KeyCode::ControlLeft)
        || input_resources
            .keyboard_input
            .pressed(KeyCode::ControlRight);
    let alt = input_resources.keyboard_input.pressed(KeyCode::AltLeft)
        || input_resources.keyboard_input.pressed(KeyCode::AltRight);
    let win = input_resources.keyboard_input.pressed(KeyCode::SuperLeft)
        || input_resources.keyboard_input.pressed(KeyCode::SuperRight);

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
    if let Some(cursor_left) = input_events.ev_cursor_left.read().last() {
        cursor_left_window = Some(cursor_left.window);
    }
    let cursor_entered_window = input_events
        .ev_cursor_entered
        .read()
        .last()
        .map(|event| event.window);

    // When a user releases a mouse button, Safari emits both `CursorLeft` and `CursorEntered`
    // events during the same frame. We don't want to reset mouse position in such a case, otherwise
    // we won't be able to process the mouse button event.
    let prev_mouse_position =
        if cursor_left_window.is_some() && cursor_left_window != cursor_entered_window {
            // If it's not the Safari edge case, reset the mouse position.
            egui_mouse_position.take()
        } else {
            None
        };

    if let Some(cursor_moved) = input_events.ev_cursor.read().last() {
        // If we've left the window, it's unlikely that we've moved the cursor back to the same
        // window this exact frame, so we are safe to ignore all `CursorMoved` events for the window
        // that has been left.
        if cursor_left_window != Some(cursor_moved.window) {
            let scale_factor = egui_settings.scale_factor as f32;
            let mouse_position: (f32, f32) = (cursor_moved.position / scale_factor).into();
            let mut context = context_params
                .contexts
                .get_mut(cursor_moved.window)
                .unwrap();
            egui_mouse_position.0 = Some((cursor_moved.window, mouse_position.into()));
            context
                .egui_input
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
    if let Some((window_id, position)) = egui_mouse_position.or(prev_mouse_position) {
        if let Ok(mut context) = context_params.contexts.get_mut(window_id) {
            let events = &mut context.egui_input.events;

            for mouse_button_event in input_events.ev_mouse_button_input.read() {
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

            for event in input_events.ev_mouse_wheel.read() {
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

    if !command || cfg!(target_os = "windows") && ctrl && alt {
        for event in input_events.ev_received_character.read() {
            if !event.char.is_control() {
                let mut context = context_params.contexts.get_mut(event.window).unwrap();
                context
                    .egui_input
                    .events
                    .push(egui::Event::Text(event.char.to_string()));
            }
        }
    }

    if let Some(mut focused_input) = context_params
        .focused_window
        .as_ref()
        .and_then(|window_id| {
            if let Ok(context) = context_params.contexts.get_mut(*window_id) {
                Some(context.egui_input)
            } else {
                None
            }
        })
    {
        for ev in input_events.ev_keyboard_input.read() {
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

        for touch in input_events.ev_touch.read() {
            let scale_factor = egui_settings.scale_factor as f32;
            let touch_position: (f32, f32) = (touch.position / scale_factor).into();

            // Emit touch event
            focused_input.events.push(egui::Event::Touch {
                device_id: egui::TouchDeviceId(egui::epaint::util::hash(touch.id)),
                id: egui::TouchId::from(touch.id),
                phase: match touch.phase {
                    bevy::input::touch::TouchPhase::Started => egui::TouchPhase::Start,
                    bevy::input::touch::TouchPhase::Moved => egui::TouchPhase::Move,
                    bevy::input::touch::TouchPhase::Ended => egui::TouchPhase::End,
                    bevy::input::touch::TouchPhase::Canceled => egui::TouchPhase::Cancel,
                },
                pos: egui::pos2(touch_position.0, touch_position.1),
                force: match touch.force {
                    Some(bevy::input::touch::ForceTouch::Normalized(force)) => Some(force as f32),
                    Some(bevy::input::touch::ForceTouch::Calibrated {
                        force,
                        max_possible_force,
                        ..
                    }) => Some((force / max_possible_force) as f32),
                    None => None,
                },
            });

            // If we're not yet tanslating a touch or we're translating this very
            // touch …
            if context_params.pointer_touch_id.0.is_none()
                || context_params.pointer_touch_id.0.unwrap() == touch.id
            {
                // … emit PointerButton resp. PointerMoved events to emulate mouse
                match touch.phase {
                    bevy::input::touch::TouchPhase::Started => {
                        context_params.pointer_touch_id.0 = Some(touch.id);
                        // First move the pointer to the right location
                        focused_input
                            .events
                            .push(egui::Event::PointerMoved(egui::pos2(
                                touch_position.0,
                                touch_position.1,
                            )));
                        // Then do mouse button input
                        focused_input.events.push(egui::Event::PointerButton {
                            pos: egui::pos2(touch_position.0, touch_position.1),
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers,
                        });
                    }
                    bevy::input::touch::TouchPhase::Moved => {
                        focused_input
                            .events
                            .push(egui::Event::PointerMoved(egui::pos2(
                                touch_position.0,
                                touch_position.1,
                            )));
                    }
                    bevy::input::touch::TouchPhase::Ended => {
                        context_params.pointer_touch_id.0 = None;
                        focused_input.events.push(egui::Event::PointerButton {
                            pos: egui::pos2(touch_position.0, touch_position.1),
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers,
                        });
                        focused_input.events.push(egui::Event::PointerGone);
                    }
                    bevy::input::touch::TouchPhase::Canceled => {
                        context_params.pointer_touch_id.0 = None;
                        focused_input.events.push(egui::Event::PointerGone);
                    }
                }
            }
        }

        focused_input.modifiers = modifiers;
    }

    for mut context in context_params.contexts.iter_mut() {
        context.egui_input.time = Some(time.elapsed_seconds_f64());
    }

    // In some cases, we may skip certain events. For example, we ignore `ReceivedCharacter` events
    // when alt or ctrl button is pressed. We still want to clear event buffer.
    input_events.clear();
}

/// Initialises Egui contexts (for multiple windows).
pub fn update_window_contexts_system(
    mut context_params: ContextSystemParams,
    egui_settings: Res<EguiSettings>,
) {
    for mut context in context_params.contexts.iter_mut() {
        let new_window_size = WindowSize::new(
            context.window.physical_width() as f32,
            context.window.physical_height() as f32,
            context.window.scale_factor() as f32,
        );
        let width = new_window_size.physical_width
            / new_window_size.scale_factor
            / egui_settings.scale_factor as f32;
        let height = new_window_size.physical_height
            / new_window_size.scale_factor
            / egui_settings.scale_factor as f32;

        if width < 1.0 || height < 1.0 {
            continue;
        }

        context.egui_input.screen_rect = Some(egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(width, height),
        ));

        context.egui_input.pixels_per_point =
            Some(new_window_size.scale_factor * egui_settings.scale_factor as f32);

        *context.window_size = new_window_size;
    }
}

/// Marks frame start for Egui.
pub fn begin_frame_system(mut contexts: Query<(&mut EguiContext, &mut EguiInput)>) {
    for (mut ctx, mut egui_input) in contexts.iter_mut() {
        ctx.get_mut().begin_frame(egui_input.take());
    }
}

/// Reads Egui output.
pub fn process_output_system(
    #[cfg_attr(not(feature = "open_url"), allow(unused_variables))] egui_settings: Res<
        EguiSettings,
    >,
    mut contexts: Query<EguiContextQuery>,
    #[cfg(feature = "manage_clipboard")] mut egui_clipboard: ResMut<crate::EguiClipboard>,
    mut event: EventWriter<RequestRedraw>,
    #[cfg(windows)] mut last_cursor_icon: Local<bevy::utils::HashMap<Entity, egui::CursorIcon>>,
) {
    for mut context in contexts.iter_mut() {
        let ctx = context.ctx.get_mut();
        let full_output = ctx.end_frame();
        let egui::FullOutput {
            platform_output,
            shapes,
            textures_delta,
            repaint_after,
        } = full_output;
        let paint_jobs = ctx.tessellate(shapes);

        context.render_output.paint_jobs = paint_jobs;
        context.render_output.textures_delta.append(textures_delta);

        context.egui_output.platform_output = platform_output.clone();

        #[cfg(feature = "manage_clipboard")]
        if !platform_output.copied_text.is_empty() {
            egui_clipboard.set_contents(&platform_output.copied_text);
        }

        let mut set_icon = || {
            context.window.cursor.icon = egui_to_winit_cursor_icon(platform_output.cursor_icon)
                .unwrap_or(bevy::window::CursorIcon::Default);
        };

        #[cfg(windows)]
        {
            let last_cursor_icon = last_cursor_icon.entry(context.window_entity).or_default();
            if *last_cursor_icon != platform_output.cursor_icon {
                set_icon();
                *last_cursor_icon = platform_output.cursor_icon;
            }
        }
        #[cfg(not(windows))]
        set_icon();

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
        KeyCode::NumpadEnter => egui::Key::Enter,
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
