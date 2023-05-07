use std::sync::mpsc::{self, Receiver, Sender};

use bevy::{input::common_conditions::input_just_pressed, prelude::*, winit::WinitWindows};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use egui::{text_edit::CursorRange, TextEdit, Widget};
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
        text.0[selected.primary.ccursor.index..selected.secondary.ccursor.index].to_string()
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
                info!("clippyboy worked");
            }
            None => {
                warn!("failed to write clippyboy");
            }
        };
    });
}

#[derive(Default)]
struct ClipboardChannel {
    pub rx: Option<Receiver<String>>,
}

fn setup_clipboard_paste(
    mut commands: Commands,
    windows: Query<Entity, With<Window>>,
    winit_windows: NonSendMut<WinitWindows>,
    mut clipboardChannel: NonSendMut<ClipboardChannel>,
) {
    let Some(first_window) = windows.iter().next() else {
        return;
    };
    let Some(winit_window_instance) = winit_windows.get_window(first_window) else {
        return;
    };
    let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();

    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::*;
    use winit::platform::web::WindowExtWebSys;
    let canvas = winit_window_instance.canvas();
    info!("canvas found");
    let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::ClipboardEvent| {
        tx.send("test".to_string());
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

    canvas
        .add_event_listener_with_callback("paste", closure.as_ref().unchecked_ref())
        .expect("Could not edd paste event listener.");
    closure.forget();
    *clipboardChannel = ClipboardChannel { rx: Some(rx) };

    //        winit_window_instance.can
    info!("setup_clipboard_paste OK");
}

fn read_clipboard_channel(mut clipboardChannel: NonSendMut<ClipboardChannel>) {
    match &mut clipboardChannel.rx {
        Some(rx) => {
            if let Ok(clipboard_string) = rx.try_recv() {
                info!("received: {}", clipboard_string);
            }
        }
        None => {}
    }
}
