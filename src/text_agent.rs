//! The text agent is an `<input>` element used to trigger
//! mobile keyboard and IME input.

use std::sync::Mutex;

use bevy::{
    prelude::{EventWriter, Res, Resource},
    window::RequestRedraw,
};
use crossbeam_channel::Sender;

use once_cell::sync::Lazy;
use wasm_bindgen::prelude::*;

use crate::{systems::ContextSystemParams, EventClosure, SubscribedEvents};

static AGENT_ID: &str = "egui_text_agent";

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VirtualTouchInfo {
    pub editing_text: bool,
}

pub static VIRTUAL_KEYBOARD_GLOBAL: Lazy<Mutex<VirtualTouchInfo>> =
    Lazy::new(|| Mutex::new(VirtualTouchInfo::default()));

#[derive(Resource)]
pub struct TextAgentChannel {
    pub sender: crossbeam_channel::Sender<egui::Event>,
    pub receiver: crossbeam_channel::Receiver<egui::Event>,
}

impl Default for TextAgentChannel {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }
}

pub fn propagate_text(
    channel: Res<TextAgentChannel>,
    mut context_params: ContextSystemParams,
    mut redraw_event: EventWriter<RequestRedraw>,
) {
    for mut contexts in context_params.contexts.iter_mut() {
        if contexts.egui_input.focused {
            let mut redraw = false;
            while let Ok(r) = channel.receiver.try_recv() {
                redraw = true;
                contexts.egui_input.events.push(r);
            }
            if redraw {
                redraw_event.send(RequestRedraw);
            }
            break;
        }
    }
}

// stolen from https://github.com/emilk/egui/pull/4855
pub fn is_mobile_safari() -> bool {
    (|| {
        let user_agent = web_sys::window()?.navigator().user_agent().ok()?;
        let is_ios = user_agent.contains("iPhone")
            || user_agent.contains("iPad")
            || user_agent.contains("iPod");
        let is_safari = user_agent.contains("Safari");
        Some(is_ios && is_safari)
    })()
    .unwrap_or(false)
}

fn is_mobile() -> Option<bool> {
    const MOBILE_DEVICE: [&str; 6] = ["Android", "iPhone", "iPad", "iPod", "webOS", "BlackBerry"];

    let user_agent = web_sys::window()?.navigator().user_agent().ok()?;
    let is_mobile = MOBILE_DEVICE.iter().any(|&name| user_agent.contains(name));
    Some(is_mobile)
}

/// Text event handler,
pub fn install_text_agent(
    subscribed_keyboard_events: &mut SubscribedEvents<web_sys::KeyboardEvent>,
    subscribed_input_events: &mut SubscribedEvents<web_sys::InputEvent>,
    subscribed_touch_events: &mut SubscribedEvents<web_sys::TouchEvent>,
    sender: Sender<egui::Event>,
) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().expect("document should have a body");
    let input = document
        .create_element("input")?
        .dyn_into::<web_sys::HtmlInputElement>()?;
    let input = std::rc::Rc::new(input);
    input.set_type("text");
    input.set_autofocus(true);
    input.set_attribute("autocapitalize", "off")?;
    input.set_id(AGENT_ID);
    {
        let style = input.style();
        // Transparent
        style.set_property("background-color", "transparent")?;
        style.set_property("border", "none")?;
        style.set_property("outline", "none")?;
        style.set_property("width", "1px")?;
        style.set_property("height", "1px")?;
        style.set_property("caret-color", "transparent")?;
        style.set_property("position", "absolute")?;
        style.set_property("top", "0")?;
        style.set_property("left", "0")?;
    }
    // Set size as small as possible, in case user may click on it.
    input.set_size(1);
    input.set_autofocus(true);
    input.set_hidden(true);

    if let Some(true) = is_mobile() {
        let input_clone = input.clone();
        let sender_clone = sender.clone();
        let closure = Closure::wrap(Box::new(move |_event: web_sys::InputEvent| {
            let text = input_clone.value();

            if !text.is_empty() {
                input_clone.set_value("");
                if text.len() == 1 {
                    input_clone.blur().ok();
                    input_clone.focus().ok();
                    let _ = sender_clone.send(egui::Event::Text(text.clone()));
                }
            }
        }) as Box<dyn FnMut(_)>);
        input.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref())?;
        subscribed_input_events.event_closures.push(EventClosure {
            target: <web_sys::Document as std::convert::AsRef<web_sys::EventTarget>>::as_ref(
                &document,
            )
            .clone(),
            event_name: "virtual_keyboard_input".to_owned(),
            closure,
        });

        // mobile safari doesn't let you set input focus outside of an event handler
        if is_mobile_safari() {
            let closure = Closure::wrap(Box::new(move |_event: web_sys::TouchEvent| {
                match VIRTUAL_KEYBOARD_GLOBAL.lock() {
                    Ok(touch_info) => {
                        update_text_agent(touch_info.editing_text);
                    }
                    Err(poisoned) => {
                        let _unused = poisoned.into_inner();
                    }
                };
            }) as Box<dyn FnMut(_)>);
            document
                .add_event_listener_with_callback("touchend", closure.as_ref().unchecked_ref())
                .unwrap();
            subscribed_touch_events.event_closures.push(EventClosure {
                target: <web_sys::Document as std::convert::AsRef<web_sys::EventTarget>>::as_ref(
                    &document,
                )
                .clone(),
                event_name: "virtual_keyboard_touchend".to_owned(),
                closure,
            });
        }

        // keydown
        let sender_clone = sender.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            if event.is_composing() || event.key_code() == 229 {
                // https://www.fxsitecompat.dev/en-CA/docs/2018/keydown-and-keyup-events-are-now-fired-during-ime-composition/
                return;
            }
            if "Backspace" == event.key() {
                let _ = sender_clone.send(egui::Event::Key {
                    key: egui::Key::Backspace,
                    physical_key: None,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                    repeat: false,
                });
            }
        }) as Box<dyn FnMut(_)>);
        document.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        subscribed_keyboard_events
            .event_closures
            .push(EventClosure {
                target: <web_sys::Document as std::convert::AsRef<web_sys::EventTarget>>::as_ref(
                    &document,
                )
                .clone(),
                event_name: "virtual_keyboard_keydown".to_owned(),
                closure,
            });

        // keyup
        let input_clone = input.clone();
        let sender_clone = sender.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            input_clone.focus().ok();
            if "Backspace" == event.key() {
                let _ = sender_clone.send(egui::Event::Key {
                    key: egui::Key::Backspace,
                    physical_key: None,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                    repeat: false,
                });
            }
        }) as Box<dyn FnMut(_)>);
        document.add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())?;
        subscribed_keyboard_events
            .event_closures
            .push(EventClosure {
                target: <web_sys::Document as std::convert::AsRef<web_sys::EventTarget>>::as_ref(
                    &document,
                )
                .clone(),
                event_name: "virtual_keyboard_keyup".to_owned(),
                closure,
            });
    }

    body.append_child(&input)?;

    Ok(())
}

/// Focus or blur text agent to toggle mobile keyboard.
pub fn update_text_agent(editing_text: bool) {
    use web_sys::HtmlInputElement;

    let window = match web_sys::window() {
        Some(window) => window,
        None => {
            bevy::log::error!("No window found");
            return;
        }
    };
    let document = match window.document() {
        Some(doc) => doc,
        None => {
            bevy::log::error!("No document found");
            return;
        }
    };
    let input: HtmlInputElement = match document.get_element_by_id(AGENT_ID) {
        Some(ele) => ele,
        None => {
            bevy::log::error!("Agent element not found");
            return;
        }
    }
    .dyn_into()
    .unwrap();

    let keyboard_closed = input.hidden();

    if editing_text && keyboard_closed {
        // open keyboard
        input.set_hidden(false);
        match input.focus().ok() {
            Some(_) => {}
            None => {
                bevy::log::error!("Unable to set focus");
            }
        }
    } else {
        // close keyboard
        if input.blur().is_err() {
            bevy::log::error!("Agent element not found");
            return;
        }

        input.set_hidden(true);
    }
}
