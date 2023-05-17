use std::sync::mpsc::{self, Receiver, Sender};

use bevy::{input::common_conditions::input_just_pressed, prelude::*};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use egui::{text_edit::CursorRange, TextEdit};
use wasm_bindgen_futures::spawn_local;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugin(EguiPlugin)
        .init_resource::<CustomText>()
        .init_non_send_resource::<ClipboardChannel>()
        // Systems that create Egui widgets should be run during the `CoreSet::Update` set,
        // or after the `EguiSet::BeginFrame` system (which belongs to the `CoreSet::PreUpdate` set).
        .add_startup_system(setup_clipboard_paste)
        .add_system(ui_edit)
        .add_system(clipboard_copy.run_if(input_just_pressed(KeyCode::C)))
        .add_system(read_clipboard_channel)
        .run();
}

#[derive(Resource, Default)]
struct CustomText(pub String, pub Option<CursorRange>);

fn ui_edit(mut contexts: EguiContexts, mut text: ResMut<CustomText>) {
    egui::Window::new("Hello").show(contexts.ctx_mut(), |ui| {
        let edit = TextEdit::multiline(&mut text.0).show(ui);
        text.1 = edit.cursor_range;
    });
}

fn clipboard_copy(mut text: ResMut<CustomText>) {
    //text.0 = "copy".into();

    let text = if let Some(selected) = text.1 {
        text.0[selected.as_sorted_char_range()].to_string()
    } else {
        "".into()
    };
    let _task = spawn_local(async move {
        let window = web_sys::window().expect("window"); // { obj: val };

        let nav = window.navigator();

        let clipboard = nav.clipboard();
        match clipboard {
            Some(a) => {
                let p = a.write_text(&text);
                let result = wasm_bindgen_futures::JsFuture::from(p)
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

#[derive(Default)]
struct ClipboardChannel {
    pub rx: Option<Receiver<String>>,
}

fn setup_clipboard_paste(mut clipboard_channel: NonSendMut<ClipboardChannel>) {
    let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();

    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::*;

    let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::ClipboardEvent| {
        // TODO: maybe we should check if current canvas is selected ? not sure it's possible,
        // but reacting to event at the document level will lead to problems if multiple games are on the samge page.
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
    *clipboard_channel = ClipboardChannel { rx: Some(rx) };

    info!("setup_clipboard_paste OK");
}

fn read_clipboard_channel(
    mut clipboard_channel: NonSendMut<ClipboardChannel>,
    mut text: ResMut<CustomText>,
) {
    match &mut clipboard_channel.rx {
        Some(rx) => {
            if let Ok(clipboard_string) = rx.try_recv() {
                // TODO: a global res is not the way to go, we should detect which element is focused when the paste is triggered.
                info!("received: {}", clipboard_string);
                // TODO: sometime I receive a single string, and it's printed twice in the edit field,
                // I guess some concurrency problem with ui_edit but I'm not sure.
                if let Some(selected) = text.1 {
                    text.0
                        .replace_range(selected.as_sorted_char_range(), &clipboard_string);
                } else {
                    text.0 = clipboard_string;
                };
                // TODO: set cursor to the end of the copied text.
            }
        }
        None => {}
    }
}
