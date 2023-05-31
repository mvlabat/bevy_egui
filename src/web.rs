use crossbeam_channel::{Receiver, Sender};

use bevy::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::EguiClipboard;

/// startup system for bevy to initialize web events.
pub fn startup_setup_web_events(mut clipboard_channel: ResMut<EguiClipboard>) {
    setup_clipboard_paste(&mut clipboard_channel.clipboard)
}

fn setup_clipboard_paste(clipboard_channel: &mut WebClipboardPaste) {
    let (tx, rx): (Sender<String>, Receiver<String>) = crossbeam_channel::bounded(1);

    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::*;

    let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::ClipboardEvent| {
        // TODO: maybe we should check if current canvas is selected ? not sure it's possible,
        // but reacting to event at the document level will lead to problems if multiple games are on the same page.
        match event
            .clipboard_data()
            .expect("could not get clipboard data.")
            .get_data("text/plain")
        {
            Ok(data) => {
                tx.send(data);
            }
            _ => {
                info!("Not implemented.");
            }
        }
        info!("{:?}", event.clipboard_data())
    });

    // TODO: a lot of unwraps ; it's using documents because paste event set on a canvas do not trigger (also tested on firefox in vanilla javascript)
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .add_event_listener_with_callback("paste", closure.as_ref().unchecked_ref())
        .expect("Could not edd paste event listener.");
    closure.forget();
    *clipboard_channel = WebClipboardPaste { rx: Some(rx) };

    info!("setup_clipboard_paste OK");
}

/// To get data from web paste events
#[derive(Default)]
pub struct WebClipboardPaste {
    rx: Option<Receiver<String>>,
}

impl WebClipboardPaste {
    /// Only returns Some if user explicitly triggered a paste event.
    /// We are not querying the clipboard data without user input here (it would require permissions).
    pub fn try_read_clipboard_event(&mut self) -> Option<String> {
        match &mut self.rx {
            Some(rx) => {
                if let Ok(clipboard_string) = rx.try_recv() {
                    info!("received: {}", clipboard_string);
                    return Some(clipboard_string);
                }
                None
            }
            None => {
                info!("no arc");
                None
            }
        }
    }
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
                info!("copy to clipboard worked");
            }
            None => {
                warn!("failed to write clipboard data");
            }
        };
    });
}
