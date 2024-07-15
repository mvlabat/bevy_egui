use crate::EguiClipboard;
use bevy::{log, prelude::*};
use crossbeam_channel::{Receiver, Sender};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{EventClosure, SubscribedEvents};

/// Startup system to initialize web clipboard events.
pub fn startup_setup_web_events(
    mut egui_clipboard: ResMut<EguiClipboard>,
    mut subscribed_events: NonSendMut<SubscribedEvents<web_sys::ClipboardEvent>>,
) {
    let (tx, rx) = crossbeam_channel::unbounded();
    egui_clipboard.clipboard.event_receiver = Some(rx);
    setup_clipboard_copy(&mut subscribed_events, tx.clone());
    setup_clipboard_cut(&mut subscribed_events, tx.clone());
    setup_clipboard_paste(&mut subscribed_events, tx);
}

/// Internal implementation of `[crate::EguiClipboard]` for web.
#[derive(Default)]
pub struct WebClipboard {
    event_receiver: Option<Receiver<WebClipboardEvent>>,
    contents: Option<String>,
}

/// Events sent by the `cut`/`copy`/`paste` listeners.
#[derive(Debug)]
pub enum WebClipboardEvent {
    /// Is sent whenever the `cut` event listener is called.
    Cut,
    /// Is sent whenever the `copy` event listener is called.
    Copy,
    /// Is sent whenever the `paste` event listener is called, includes the plain text content.
    Paste(String),
}

impl WebClipboard {
    /// Sets clipboard contents.
    pub fn set_contents(&mut self, contents: &str) {
        self.set_contents_internal(contents);
        clipboard_copy(contents.to_owned());
    }

    /// Sets the internal buffer of clipboard contents.
    /// This buffer is used to remember the contents of the last `paste` event.
    pub fn set_contents_internal(&mut self, contents: &str) {
        self.contents = Some(contents.to_owned());
    }

    /// Gets clipboard contents. Returns [`None`] if the `copy`/`cut` operation have never been invoked yet,
    /// or the `paste` event has never been received yet.
    pub fn get_contents(&mut self) -> Option<String> {
        self.contents.clone()
    }

    /// Receives a clipboard event sent by the `copy`/`cut`/`paste` listeners.
    pub fn try_receive_clipboard_event(&self) -> Option<WebClipboardEvent> {
        let Some(rx) = &self.event_receiver else {
            log::error!("Web clipboard event receiver isn't initialized");
            return None;
        };

        match rx.try_recv() {
            Ok(event) => Some(event),
            Err(crossbeam_channel::TryRecvError::Empty) => None,
            Err(err @ crossbeam_channel::TryRecvError::Disconnected) => {
                log::error!("Failed to read a web clipboard event: {err:?}");
                None
            }
        }
    }
}

fn setup_clipboard_copy(
    subscribed_events: &mut SubscribedEvents<web_sys::ClipboardEvent>,
    tx: Sender<WebClipboardEvent>,
) {
    let Some(window) = web_sys::window() else {
        log::error!("Failed to add the \"copy\" listener: no window object");
        return;
    };
    let Some(document) = window.document() else {
        log::error!("Failed to add the \"copy\" listener: no document object");
        return;
    };

    let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::ClipboardEvent| {
        if tx.send(WebClipboardEvent::Copy).is_err() {
            log::error!("Failed to send a \"copy\" event: channel is disconnected");
        }
    });

    let listener = closure.as_ref().unchecked_ref();

    if let Err(err) = document.add_event_listener_with_callback("copy", listener) {
        log::error!(
            "Failed to add the \"copy\" event listener: {}",
            string_from_js_value(&err)
        );
        drop(closure);
        return;
    };
    subscribed_events.event_closures.push(EventClosure {
        target: <web_sys::Document as std::convert::AsRef<web_sys::EventTarget>>::as_ref(&document)
            .clone(),
        event_name: "copy".to_owned(),
        closure,
    });
}

fn setup_clipboard_cut(
    subscribed_events: &mut SubscribedEvents<web_sys::ClipboardEvent>,
    tx: Sender<WebClipboardEvent>,
) {
    let Some(window) = web_sys::window() else {
        log::error!("Failed to add the \"cut\" listener: no window object");
        return;
    };
    let Some(document) = window.document() else {
        log::error!("Failed to add the \"cut\" listener: no document object");
        return;
    };

    let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::ClipboardEvent| {
        if tx.send(WebClipboardEvent::Cut).is_err() {
            log::error!("Failed to send a \"cut\" event: channel is disconnected");
        }
    });

    let listener = closure.as_ref().unchecked_ref();

    if let Err(err) = document.add_event_listener_with_callback("cut", listener) {
        log::error!(
            "Failed to add the \"cut\" event listener: {}",
            string_from_js_value(&err)
        );
        drop(closure);
        return;
    };
    subscribed_events.event_closures.push(EventClosure {
        target: <web_sys::Document as std::convert::AsRef<web_sys::EventTarget>>::as_ref(&document)
            .clone(),
        event_name: "cut".to_owned(),
        closure,
    });
}

fn setup_clipboard_paste(
    subscribed_events: &mut SubscribedEvents<web_sys::ClipboardEvent>,
    tx: Sender<WebClipboardEvent>,
) {
    let Some(window) = web_sys::window() else {
        log::error!("Failed to add the \"paste\" listener: no window object");
        return;
    };
    let Some(document) = window.document() else {
        log::error!("Failed to add the \"paste\" listener: no document object");
        return;
    };

    let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::ClipboardEvent| {
        let Some(clipboard_data) = event.clipboard_data() else {
            log::error!("Failed to access clipboard data");
            return;
        };
        match clipboard_data.get_data("text/plain") {
            Ok(data) => {
                if tx.send(WebClipboardEvent::Paste(data)).is_err() {
                    log::error!("Failed to send the \"paste\" event: channel is disconnected");
                }
            }
            Err(err) => {
                log::error!(
                    "Failed to read clipboard data: {}",
                    string_from_js_value(&err)
                );
            }
        }
    });

    let listener = closure.as_ref().unchecked_ref();

    if let Err(err) = document.add_event_listener_with_callback("paste", listener) {
        log::error!(
            "Failed to add the \"paste\" event listener: {}",
            string_from_js_value(&err)
        );
        drop(closure);
        return;
    };
    subscribed_events.event_closures.push(EventClosure {
        target: <web_sys::Document as std::convert::AsRef<web_sys::EventTarget>>::as_ref(&document)
            .clone(),
        event_name: "paste".to_owned(),
        closure,
    });
}

/// Sets contents of the clipboard via the Web API.
fn clipboard_copy(contents: String) {
    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            log::warn!("Failed to access the window object");
            return;
        };

        let clipboard = window.navigator().clipboard();

        let promise = clipboard.write_text(&contents);
        if let Err(err) = wasm_bindgen_futures::JsFuture::from(promise).await {
            log::warn!(
                "Failed to write to clipboard: {}",
                string_from_js_value(&err)
            );
        }
    });
}

/// Helper function for outputting a String from a JsValue
pub fn string_from_js_value(value: &JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:#?}"))
}
