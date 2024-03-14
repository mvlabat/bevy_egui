use crossbeam_channel::{Receiver, Sender};

use bevy::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::EguiClipboard;
use wasm_bindgen::{closure::Closure, prelude::*};

/// startup system for bevy to initialize web events.
pub fn startup_setup_web_events(mut clipboard_channel: ResMut<EguiClipboard>) {
    setup_clipboard_copy(&mut clipboard_channel.web_copy);
    setup_clipboard_cut(&mut clipboard_channel.web_cut);
    setup_clipboard_paste(&mut clipboard_channel.web_paste);
}

/// To get data from web events
#[derive(Default)]
pub struct WebChannel<T> {
    rx: Option<Receiver<T>>,
}

impl<T> WebChannel<T> {
    /// Only returns Some if user explicitly triggered an event. Should be called each frame to react as soon as the event is fired.
    pub fn try_read_clipboard_event(&mut self) -> Option<T> {
        match &mut self.rx {
            Some(rx) => {
                if let Ok(data) = rx.try_recv() {
                    return Some(data);
                }
                None
            }
            None => None,
        }
    }
}

/// User provided a string to paste
#[derive(Debug, Default)]
pub struct WebEventPaste(pub String);
/// User asked to cut
#[derive(Default)]
pub struct WebEventCut;
/// Used asked to copy
#[derive(Default)]
pub struct WebEventCopy;

fn setup_clipboard_copy(clipboard_channel: &mut WebChannel<WebEventCopy>) {
    let (tx, rx): (Sender<WebEventCopy>, Receiver<WebEventCopy>) = crossbeam_channel::bounded(1);

    let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::ClipboardEvent| {
        let _ = tx.try_send(WebEventCopy);
    });

    let listener = closure.as_ref().unchecked_ref();
    web_sys::window()
        .expect("Could not retrieve web_sys::window()")
        .document()
        .expect("Could not retrieve web_sys window's document")
        .add_event_listener_with_callback("copy", listener)
        .expect("Could not add copy event listener.");
    closure.forget();
    *clipboard_channel = WebChannel::<WebEventCopy> { rx: Some(rx) };
}

fn setup_clipboard_cut(clipboard_channel: &mut WebChannel<WebEventCut>) {
    let (tx, rx): (Sender<WebEventCut>, Receiver<WebEventCut>) = crossbeam_channel::bounded(1);

    let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::ClipboardEvent| {
        let _ = tx.try_send(WebEventCut);
    });

    let listener = closure.as_ref().unchecked_ref();
    web_sys::window()
        .expect("Could not retrieve web_sys::window()")
        .document()
        .expect("Could not retrieve web_sys window's document")
        .add_event_listener_with_callback("cut", listener)
        .expect("Could not add cut event listener.");
    closure.forget();
    *clipboard_channel = WebChannel::<WebEventCut> { rx: Some(rx) };
}

fn setup_clipboard_paste(clipboard_channel: &mut WebChannel<WebEventPaste>) {
    let (tx, rx): (Sender<WebEventPaste>, Receiver<WebEventPaste>) = crossbeam_channel::bounded(1);

    let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::ClipboardEvent| {
        match event
            .clipboard_data()
            .expect("could not get clipboard data.")
            .get_data("text/plain")
        {
            Ok(data) => {
                let _ = tx.try_send(WebEventPaste(data));
            }
            _ => {
                error!("Not implemented.");
            }
        }
    });

    let listener = closure.as_ref().unchecked_ref();
    web_sys::window()
        .expect("Could not retrieve web_sys::window()")
        .document()
        .expect("Could not retrieve web_sys window's document")
        .add_event_listener_with_callback("paste", listener)
        .expect("Could not add paste event listener.");
    closure.forget();
    *clipboard_channel = WebChannel::<WebEventPaste> { rx: Some(rx) };
}

/// Puts argument string to the web clipboard
pub fn clipboard_copy(text: String) {
    spawn_local(async move {
        let window = web_sys::window().expect("window");

        let nav = window.navigator();

        let clipboard = nav.clipboard();
        match clipboard {
            Some(a) => {
                let p = a.write_text(&text);
                let _result = wasm_bindgen_futures::JsFuture::from(p)
                    .await
                    .expect("clipboard populated");
            }
            None => {
                warn!("failed to write clipboard data");
            }
        };
    });
}
