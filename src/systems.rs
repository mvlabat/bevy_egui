#[cfg(feature = "render")]
use crate::EguiRenderToTextureHandle;
use crate::{
    EguiContext, EguiContextQuery, EguiContextQueryItem, EguiFullOutput, EguiInput, EguiSettings,
    RenderTargetSize,
};

#[cfg(feature = "render")]
use bevy::{asset::Assets, render::texture::Image};
use bevy::{
    ecs::{
        event::EventWriter,
        query::QueryEntityError,
        system::{Local, Res, SystemParam},
    },
    input::{
        keyboard::{Key, KeyCode, KeyboardFocusLost, KeyboardInput},
        mouse::{MouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel},
        touch::TouchInput,
        ButtonState,
    },
    log::{self, error},
    prelude::{Entity, EventReader, NonSend, Query, Resource, Time},
    time::Real,
    window::{CursorMoved, RequestRedraw},
    winit::{EventLoopProxy, WakeUp},
};
use std::{marker::PhantomData, time::Duration};

#[allow(missing_docs)]
#[derive(SystemParam)]
// IMPORTANT: remember to add the logic to clear event readers to the `clear` method.
pub struct InputEvents<'w, 's> {
    pub ev_cursor: EventReader<'w, 's, CursorMoved>,
    pub ev_mouse_button_input: EventReader<'w, 's, MouseButtonInput>,
    pub ev_mouse_wheel: EventReader<'w, 's, MouseWheel>,
    pub ev_keyboard_input: EventReader<'w, 's, KeyboardInput>,
    pub ev_touch: EventReader<'w, 's, TouchInput>,
    pub ev_focus: EventReader<'w, 's, KeyboardFocusLost>,
}

impl<'w, 's> InputEvents<'w, 's> {
    /// Consumes all the events.
    pub fn clear(&mut self) {
        self.ev_cursor.clear();
        self.ev_mouse_button_input.clear();
        self.ev_mouse_wheel.clear();
        self.ev_keyboard_input.clear();
        self.ev_touch.clear();
        self.ev_focus.clear();
    }
}

/// Stores "pressed" state of modifier keys.
/// Will be removed if Bevy adds support for `ButtonInput<Key>` (logical keys).
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct ModifierKeysState {
    shift: bool,
    ctrl: bool,
    alt: bool,
    win: bool,
}

#[allow(missing_docs)]
#[derive(SystemParam)]
pub struct InputResources<'w, 's> {
    #[cfg(all(
        feature = "manage_clipboard",
        not(target_os = "android"),
        not(all(target_arch = "wasm32", not(web_sys_unstable_apis)))
    ))]
    pub egui_clipboard: bevy::ecs::system::ResMut<'w, crate::EguiClipboard>,
    pub modifier_keys_state: Local<'s, ModifierKeysState>,
    #[system_param(ignore)]
    _marker: PhantomData<&'w ()>,
}

#[allow(missing_docs)]
#[derive(SystemParam)]
pub struct ContextSystemParams<'w, 's> {
    pub contexts: Query<'w, 's, EguiContextQuery>,
    pub is_macos: Local<'s, bool>,
    #[system_param(ignore)]
    _marker: PhantomData<&'s ()>,
}

impl<'w, 's> ContextSystemParams<'w, 's> {
    fn window_context(&mut self, window: Entity) -> Option<EguiContextQueryItem> {
        match self.contexts.get_mut(window) {
            Ok(context) => Some(context),
            Err(err @ QueryEntityError::AliasedMutability(_)) => {
                panic!("Failed to get an Egui context for a window ({window:?}): {err:?}");
            }
            Err(
                err @ QueryEntityError::NoSuchEntity(_)
                | err @ QueryEntityError::QueryDoesNotMatch(_),
            ) => {
                log::error!("Failed to get an Egui context for a window ({window:?}): {err:?}",);
                None
            }
        }
    }
}

/// Processes Bevy input and feeds it to Egui.
pub fn process_input_system(
    mut input_events: InputEvents,
    mut input_resources: InputResources,
    mut context_params: ContextSystemParams,
    egui_settings: Res<EguiSettings>,
    time: Res<Time<Real>>,
) {
    // Test whether it's macOS or OS X.
    use std::sync::Once;
    static START: Once = Once::new();
    START.call_once(|| {
        // The default for WASM is `false` since the `target_os` is `unknown`.
        *context_params.is_macos = cfg!(target_os = "macos");

        #[cfg(target_arch = "wasm32")]
        if let Some(window) = web_sys::window() {
            let nav = window.navigator();
            if let Ok(user_agent) = nav.user_agent() {
                if user_agent.to_ascii_lowercase().contains("mac") {
                    *context_params.is_macos = true;
                }
            }
        }
    });

    let mut keyboard_input_events = Vec::new();
    for event in input_events.ev_keyboard_input.read() {
        // Copy the events as we might want to pass them to an Egui context later.
        keyboard_input_events.push(event.clone());

        let KeyboardInput {
            logical_key, state, ..
        } = event;
        match logical_key {
            Key::Shift => {
                input_resources.modifier_keys_state.shift = state.is_pressed();
            }
            Key::Control => {
                input_resources.modifier_keys_state.ctrl = state.is_pressed();
            }
            Key::Alt => {
                input_resources.modifier_keys_state.alt = state.is_pressed();
            }
            Key::Super | Key::Meta => {
                input_resources.modifier_keys_state.win = state.is_pressed();
            }
            _ => {}
        };
    }

    // If window focus is lost, clear all modifiers to avoid stuck keys.
    if !input_events.ev_focus.is_empty() {
        input_events.ev_focus.clear();
        *input_resources.modifier_keys_state = Default::default();
    }

    let ModifierKeysState {
        shift,
        ctrl,
        alt,
        win,
    } = *input_resources.modifier_keys_state;
    let mac_cmd = if *context_params.is_macos { win } else { false };
    let command = if *context_params.is_macos { win } else { ctrl };

    let modifiers = egui::Modifiers {
        alt,
        ctrl,
        shift,
        mac_cmd,
        command,
    };

    for event in input_events.ev_cursor.read() {
        let Some(mut window_context) = context_params.window_context(event.window) else {
            continue;
        };

        let scale_factor = egui_settings.scale_factor;
        let (x, y): (f32, f32) = (event.position / scale_factor).into();
        let mouse_position = egui::pos2(x, y);
        window_context.ctx.mouse_position = mouse_position;
        window_context
            .egui_input
            .events
            .push(egui::Event::PointerMoved(mouse_position));
    }

    for event in input_events.ev_mouse_button_input.read() {
        let Some(mut window_context) = context_params.window_context(event.window) else {
            continue;
        };

        let button = match event.button {
            MouseButton::Left => Some(egui::PointerButton::Primary),
            MouseButton::Right => Some(egui::PointerButton::Secondary),
            MouseButton::Middle => Some(egui::PointerButton::Middle),
            _ => None,
        };
        let pressed = match event.state {
            ButtonState::Pressed => true,
            ButtonState::Released => false,
        };
        if let Some(button) = button {
            window_context
                .egui_input
                .events
                .push(egui::Event::PointerButton {
                    pos: window_context.ctx.mouse_position,
                    button,
                    pressed,
                    modifiers,
                });
        }
    }

    for event in input_events.ev_mouse_wheel.read() {
        let Some(mut window_context) = context_params.window_context(event.window) else {
            continue;
        };

        let delta = egui::vec2(event.x, event.y);

        let unit = match event.unit {
            MouseScrollUnit::Line => egui::MouseWheelUnit::Line,
            MouseScrollUnit::Pixel => egui::MouseWheelUnit::Point,
        };

        window_context
            .egui_input
            .events
            .push(egui::Event::MouseWheel {
                unit,
                delta,
                modifiers,
            });
    }

    for event in keyboard_input_events {
        let text_event_allowed = !command && !win || !*context_params.is_macos && ctrl && alt;
        let Some(mut window_context) = context_params.window_context(event.window) else {
            continue;
        };

        if text_event_allowed && event.state.is_pressed() {
            match &event.logical_key {
                Key::Character(char) if char.matches(char::is_control).count() == 0 => {
                    (window_context.egui_input.events).push(egui::Event::Text(char.to_string()));
                }
                Key::Space => {
                    (window_context.egui_input.events).push(egui::Event::Text(" ".into()));
                }
                _ => (),
            }
        }

        let (Some(key), physical_key) = (
            bevy_to_egui_key(&event.logical_key),
            bevy_to_egui_physical_key(&event.key_code),
        ) else {
            continue;
        };

        let egui_event = egui::Event::Key {
            key,
            pressed: event.state.is_pressed(),
            repeat: false,
            modifiers,
            physical_key,
        };
        window_context.egui_input.events.push(egui_event);

        // We also check that it's an `ButtonState::Pressed` event, as we don't want to
        // copy, cut or paste on the key release.
        #[cfg(all(
            feature = "manage_clipboard",
            not(target_os = "android"),
            not(target_arch = "wasm32")
        ))]
        if command && event.state.is_pressed() {
            match key {
                egui::Key::C => {
                    window_context.egui_input.events.push(egui::Event::Copy);
                }
                egui::Key::X => {
                    window_context.egui_input.events.push(egui::Event::Cut);
                }
                egui::Key::V => {
                    if let Some(contents) = input_resources.egui_clipboard.get_contents() {
                        window_context
                            .egui_input
                            .events
                            .push(egui::Event::Text(contents))
                    }
                }
                _ => {}
            }
        }
    }

    #[cfg(all(
        feature = "manage_clipboard",
        target_arch = "wasm32",
        web_sys_unstable_apis
    ))]
    while let Some(event) = input_resources.egui_clipboard.try_receive_clipboard_event() {
        // In web, we assume that we have only 1 window per app.
        let mut window_context = context_params.contexts.single_mut();

        match event {
            crate::web_clipboard::WebClipboardEvent::Copy => {
                window_context.egui_input.events.push(egui::Event::Copy);
            }
            crate::web_clipboard::WebClipboardEvent::Cut => {
                window_context.egui_input.events.push(egui::Event::Cut);
            }
            crate::web_clipboard::WebClipboardEvent::Paste(contents) => {
                input_resources
                    .egui_clipboard
                    .set_contents_internal(&contents);
                window_context
                    .egui_input
                    .events
                    .push(egui::Event::Text(contents))
            }
        }
    }

    for event in input_events.ev_touch.read() {
        let Some(mut window_context) = context_params.window_context(event.window) else {
            continue;
        };

        let touch_id = egui::TouchId::from(event.id);
        let scale_factor = egui_settings.scale_factor;
        let touch_position: (f32, f32) = (event.position / scale_factor).into();

        // Emit touch event
        window_context.egui_input.events.push(egui::Event::Touch {
            device_id: egui::TouchDeviceId(event.window.to_bits()),
            id: touch_id,
            phase: match event.phase {
                bevy::input::touch::TouchPhase::Started => egui::TouchPhase::Start,
                bevy::input::touch::TouchPhase::Moved => egui::TouchPhase::Move,
                bevy::input::touch::TouchPhase::Ended => egui::TouchPhase::End,
                bevy::input::touch::TouchPhase::Canceled => egui::TouchPhase::Cancel,
            },
            pos: egui::pos2(touch_position.0, touch_position.1),
            force: match event.force {
                Some(bevy::input::touch::ForceTouch::Normalized(force)) => Some(force as f32),
                Some(bevy::input::touch::ForceTouch::Calibrated {
                    force,
                    max_possible_force,
                    ..
                }) => Some((force / max_possible_force) as f32),
                None => None,
            },
        });

        // If we're not yet translating a touch, or we're translating this very
        // touch, …
        if window_context.ctx.pointer_touch_id.is_none()
            || window_context.ctx.pointer_touch_id.unwrap() == event.id
        {
            // … emit PointerButton resp. PointerMoved events to emulate mouse.
            match event.phase {
                bevy::input::touch::TouchPhase::Started => {
                    window_context.ctx.pointer_touch_id = Some(event.id);
                    // First move the pointer to the right location.
                    window_context
                        .egui_input
                        .events
                        .push(egui::Event::PointerMoved(egui::pos2(
                            touch_position.0,
                            touch_position.1,
                        )));
                    // Then do mouse button input.
                    window_context
                        .egui_input
                        .events
                        .push(egui::Event::PointerButton {
                            pos: egui::pos2(touch_position.0, touch_position.1),
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers,
                        });
                }
                bevy::input::touch::TouchPhase::Moved => {
                    window_context
                        .egui_input
                        .events
                        .push(egui::Event::PointerMoved(egui::pos2(
                            touch_position.0,
                            touch_position.1,
                        )));
                }
                bevy::input::touch::TouchPhase::Ended => {
                    window_context.ctx.pointer_touch_id = None;
                    window_context
                        .egui_input
                        .events
                        .push(egui::Event::PointerButton {
                            pos: egui::pos2(touch_position.0, touch_position.1),
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers,
                        });
                    window_context
                        .egui_input
                        .events
                        .push(egui::Event::PointerGone);
                }
                bevy::input::touch::TouchPhase::Canceled => {
                    window_context.ctx.pointer_touch_id = None;
                    window_context
                        .egui_input
                        .events
                        .push(egui::Event::PointerGone);
                }
            }
        }
    }

    for mut context in context_params.contexts.iter_mut() {
        context.egui_input.modifiers = modifiers;
        context.egui_input.time = Some(time.elapsed_seconds_f64());
    }

    // In some cases, we may skip certain events. For example, we ignore `ReceivedCharacter` events
    // when alt or ctrl button is pressed. We still want to clear event buffer.
    input_events.clear();
}

/// Initialises Egui contexts (for multiple windows).
pub fn update_contexts_system(
    mut context_params: ContextSystemParams,
    egui_settings: Res<EguiSettings>,
    #[cfg(feature = "render")] images: Res<Assets<Image>>,
) {
    for mut context in context_params.contexts.iter_mut() {
        let mut render_target_size = None;
        if let Some(window) = context.window {
            render_target_size = Some(RenderTargetSize::new(
                window.physical_width() as f32,
                window.physical_height() as f32,
                window.scale_factor(),
            ));
        }
        #[cfg(feature = "render")]
        if let Some(EguiRenderToTextureHandle(handle)) = context.render_to_texture.as_deref() {
            let image = images.get(handle).expect("rtt handle should be valid");
            let size = image.size_f32();
            render_target_size = Some(RenderTargetSize {
                physical_width: size.x,
                physical_height: size.y,
                scale_factor: 1.0,
            })
        }

        let Some(new_render_target_size) = render_target_size else {
            error!("bevy_egui context without window or render to texture!");
            continue;
        };
        let width = new_render_target_size.physical_width
            / new_render_target_size.scale_factor
            / egui_settings.scale_factor;
        let height = new_render_target_size.physical_height
            / new_render_target_size.scale_factor
            / egui_settings.scale_factor;

        if width < 1.0 || height < 1.0 {
            continue;
        }

        context.egui_input.screen_rect = Some(egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(width, height),
        ));

        context
            .ctx
            .get_mut()
            .set_pixels_per_point(new_render_target_size.scale_factor * egui_settings.scale_factor);

        *context.render_target_size = new_render_target_size;
    }
}

/// Marks frame start for Egui.
pub fn begin_pass_system(mut contexts: Query<(&mut EguiContext, &mut EguiInput)>) {
    for (mut ctx, mut egui_input) in contexts.iter_mut() {
        ctx.get_mut().begin_pass(egui_input.take());
    }
}

/// Marks frame end for Egui.
pub fn end_pass_system(mut contexts: Query<(&mut EguiContext, &mut EguiFullOutput)>) {
    for (mut ctx, mut full_output) in contexts.iter_mut() {
        **full_output = Some(ctx.get_mut().end_pass());
    }
}

/// Reads Egui output.
pub fn process_output_system(
    #[cfg_attr(not(feature = "open_url"), allow(unused_variables))] egui_settings: Res<
        EguiSettings,
    >,
    mut contexts: Query<EguiContextQuery>,
    #[cfg(all(feature = "manage_clipboard", not(target_os = "android")))]
    mut egui_clipboard: bevy::ecs::system::ResMut<crate::EguiClipboard>,
    mut event: EventWriter<RequestRedraw>,
    #[cfg(windows)] mut last_cursor_icon: Local<bevy::utils::HashMap<Entity, egui::CursorIcon>>,
    event_loop_proxy: Option<NonSend<EventLoopProxy<WakeUp>>>,
) {
    let mut should_request_redraw = false;

    for mut context in contexts.iter_mut() {
        let ctx = context.ctx.get_mut();
        let Some(full_output) = context.egui_full_output.0.take() else {
            log::error!("bevy_egui frame output has not been prepared!");
            continue;
        };
        let egui::FullOutput {
            platform_output,
            shapes,
            textures_delta,
            pixels_per_point,
            viewport_output: _,
        } = full_output;
        let paint_jobs = ctx.tessellate(shapes, pixels_per_point);

        context.render_output.paint_jobs = paint_jobs;
        context.render_output.textures_delta.append(textures_delta);

        context.egui_output.platform_output = platform_output.clone();

        #[cfg(all(
            feature = "manage_clipboard",
            not(target_os = "android"),
            not(all(target_arch = "wasm32", not(web_sys_unstable_apis)))
        ))]
        if !platform_output.copied_text.is_empty() {
            egui_clipboard.set_contents(&platform_output.copied_text);
        }

        if let Some(mut window) = context.window {
            let mut set_icon = || {
                window.cursor.icon = egui_to_winit_cursor_icon(platform_output.cursor_icon)
                    .unwrap_or(bevy::window::CursorIcon::Default);
            };

            #[cfg(windows)]
            {
                let last_cursor_icon = last_cursor_icon.entry(context.render_target).or_default();
                if *last_cursor_icon != platform_output.cursor_icon {
                    set_icon();
                    *last_cursor_icon = platform_output.cursor_icon;
                }
            }
            #[cfg(not(windows))]
            set_icon();
        }

        let needs_repaint = !context.render_output.is_empty();
        should_request_redraw |= ctx.has_requested_repaint() && needs_repaint;

        // The resource doesn't exist in the headless mode.
        if let Some(event_loop_proxy) = &event_loop_proxy {
            // A zero duration indicates that it's an outstanding redraw request, which gives Egui an
            // opportunity to settle the effects of interactions with widgets. Such repaint requests
            // are processed not immediately but on a next frame. In this case, we need to indicate to
            // winit, that it needs to wake up next frame as well even if there are no inputs.
            //
            // TLDR: this solves repaint corner cases of `WinitSettings::desktop_app()`.
            if let Some(Duration::ZERO) =
                ctx.viewport(|viewport| viewport.input.wants_repaint_after())
            {
                let _ = event_loop_proxy.send_event(WakeUp);
            }
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

    if should_request_redraw {
        event.send(RequestRedraw);
    }
}

fn egui_to_winit_cursor_icon(cursor_icon: egui::CursorIcon) -> Option<bevy::window::CursorIcon> {
    match cursor_icon {
        egui::CursorIcon::Default => Some(bevy::window::CursorIcon::Default),
        egui::CursorIcon::PointingHand => Some(bevy::window::CursorIcon::Pointer),
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

/// Matches the implementation of <https://github.com/emilk/egui/blob/68b3ef7f6badfe893d3bbb1f791b481069d807d9/crates/egui-winit/src/lib.rs#L1005>.
pub fn bevy_to_egui_key(key: &Key) -> Option<egui::Key> {
    let key = match key {
        Key::Character(str) => return egui::Key::from_name(str.as_str()),
        Key::Unidentified(_) | Key::Dead(_) => return None,

        Key::Enter => egui::Key::Enter,
        Key::Tab => egui::Key::Tab,
        Key::Space => egui::Key::Space,
        Key::ArrowDown => egui::Key::ArrowDown,
        Key::ArrowLeft => egui::Key::ArrowLeft,
        Key::ArrowRight => egui::Key::ArrowRight,
        Key::ArrowUp => egui::Key::ArrowUp,
        Key::End => egui::Key::End,
        Key::Home => egui::Key::Home,
        Key::PageDown => egui::Key::PageDown,
        Key::PageUp => egui::Key::PageUp,
        Key::Backspace => egui::Key::Backspace,
        Key::Delete => egui::Key::Delete,
        Key::Insert => egui::Key::Insert,
        Key::Escape => egui::Key::Escape,
        Key::F1 => egui::Key::F1,
        Key::F2 => egui::Key::F2,
        Key::F3 => egui::Key::F3,
        Key::F4 => egui::Key::F4,
        Key::F5 => egui::Key::F5,
        Key::F6 => egui::Key::F6,
        Key::F7 => egui::Key::F7,
        Key::F8 => egui::Key::F8,
        Key::F9 => egui::Key::F9,
        Key::F10 => egui::Key::F10,
        Key::F11 => egui::Key::F11,
        Key::F12 => egui::Key::F12,
        Key::F13 => egui::Key::F13,
        Key::F14 => egui::Key::F14,
        Key::F15 => egui::Key::F15,
        Key::F16 => egui::Key::F16,
        Key::F17 => egui::Key::F17,
        Key::F18 => egui::Key::F18,
        Key::F19 => egui::Key::F19,
        Key::F20 => egui::Key::F20,

        _ => return None,
    };
    Some(key)
}

/// Matches the implementation of <https://github.com/emilk/egui/blob/68b3ef7f6badfe893d3bbb1f791b481069d807d9/crates/egui-winit/src/lib.rs#L1080>.
pub fn bevy_to_egui_physical_key(key: &KeyCode) -> Option<egui::Key> {
    let key = match key {
        KeyCode::ArrowDown => egui::Key::ArrowDown,
        KeyCode::ArrowLeft => egui::Key::ArrowLeft,
        KeyCode::ArrowRight => egui::Key::ArrowRight,
        KeyCode::ArrowUp => egui::Key::ArrowUp,

        KeyCode::Escape => egui::Key::Escape,
        KeyCode::Tab => egui::Key::Tab,
        KeyCode::Backspace => egui::Key::Backspace,
        KeyCode::Enter | KeyCode::NumpadEnter => egui::Key::Enter,

        KeyCode::Insert => egui::Key::Insert,
        KeyCode::Delete => egui::Key::Delete,
        KeyCode::Home => egui::Key::Home,
        KeyCode::End => egui::Key::End,
        KeyCode::PageUp => egui::Key::PageUp,
        KeyCode::PageDown => egui::Key::PageDown,

        // Punctuation
        KeyCode::Space => egui::Key::Space,
        KeyCode::Comma => egui::Key::Comma,
        KeyCode::Period => egui::Key::Period,
        // KeyCode::Colon => egui::Key::Colon, // NOTE: there is no physical colon key on an american keyboard
        KeyCode::Semicolon => egui::Key::Semicolon,
        KeyCode::Backslash => egui::Key::Backslash,
        KeyCode::Slash | KeyCode::NumpadDivide => egui::Key::Slash,
        KeyCode::BracketLeft => egui::Key::OpenBracket,
        KeyCode::BracketRight => egui::Key::CloseBracket,
        KeyCode::Backquote => egui::Key::Backtick,

        KeyCode::Cut => egui::Key::Cut,
        KeyCode::Copy => egui::Key::Copy,
        KeyCode::Paste => egui::Key::Paste,
        KeyCode::Minus | KeyCode::NumpadSubtract => egui::Key::Minus,
        KeyCode::NumpadAdd => egui::Key::Plus,
        KeyCode::Equal => egui::Key::Equals,

        KeyCode::Digit0 | KeyCode::Numpad0 => egui::Key::Num0,
        KeyCode::Digit1 | KeyCode::Numpad1 => egui::Key::Num1,
        KeyCode::Digit2 | KeyCode::Numpad2 => egui::Key::Num2,
        KeyCode::Digit3 | KeyCode::Numpad3 => egui::Key::Num3,
        KeyCode::Digit4 | KeyCode::Numpad4 => egui::Key::Num4,
        KeyCode::Digit5 | KeyCode::Numpad5 => egui::Key::Num5,
        KeyCode::Digit6 | KeyCode::Numpad6 => egui::Key::Num6,
        KeyCode::Digit7 | KeyCode::Numpad7 => egui::Key::Num7,
        KeyCode::Digit8 | KeyCode::Numpad8 => egui::Key::Num8,
        KeyCode::Digit9 | KeyCode::Numpad9 => egui::Key::Num9,

        KeyCode::KeyA => egui::Key::A,
        KeyCode::KeyB => egui::Key::B,
        KeyCode::KeyC => egui::Key::C,
        KeyCode::KeyD => egui::Key::D,
        KeyCode::KeyE => egui::Key::E,
        KeyCode::KeyF => egui::Key::F,
        KeyCode::KeyG => egui::Key::G,
        KeyCode::KeyH => egui::Key::H,
        KeyCode::KeyI => egui::Key::I,
        KeyCode::KeyJ => egui::Key::J,
        KeyCode::KeyK => egui::Key::K,
        KeyCode::KeyL => egui::Key::L,
        KeyCode::KeyM => egui::Key::M,
        KeyCode::KeyN => egui::Key::N,
        KeyCode::KeyO => egui::Key::O,
        KeyCode::KeyP => egui::Key::P,
        KeyCode::KeyQ => egui::Key::Q,
        KeyCode::KeyR => egui::Key::R,
        KeyCode::KeyS => egui::Key::S,
        KeyCode::KeyT => egui::Key::T,
        KeyCode::KeyU => egui::Key::U,
        KeyCode::KeyV => egui::Key::V,
        KeyCode::KeyW => egui::Key::W,
        KeyCode::KeyX => egui::Key::X,
        KeyCode::KeyY => egui::Key::Y,
        KeyCode::KeyZ => egui::Key::Z,

        KeyCode::F1 => egui::Key::F1,
        KeyCode::F2 => egui::Key::F2,
        KeyCode::F3 => egui::Key::F3,
        KeyCode::F4 => egui::Key::F4,
        KeyCode::F5 => egui::Key::F5,
        KeyCode::F6 => egui::Key::F6,
        KeyCode::F7 => egui::Key::F7,
        KeyCode::F8 => egui::Key::F8,
        KeyCode::F9 => egui::Key::F9,
        KeyCode::F10 => egui::Key::F10,
        KeyCode::F11 => egui::Key::F11,
        KeyCode::F12 => egui::Key::F12,
        KeyCode::F13 => egui::Key::F13,
        KeyCode::F14 => egui::Key::F14,
        KeyCode::F15 => egui::Key::F15,
        KeyCode::F16 => egui::Key::F16,
        KeyCode::F17 => egui::Key::F17,
        KeyCode::F18 => egui::Key::F18,
        KeyCode::F19 => egui::Key::F19,
        KeyCode::F20 => egui::Key::F20,
        _ => return None,
    };
    Some(key)
}
