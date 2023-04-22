use bevy::{input::common_conditions::input_just_pressed, prelude::*};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use egui::{text_edit::CursorRange, TextEdit, Widget};
use wasm_bindgen_futures::spawn_local;
use web_sys::ClipboardPermissionDescriptor;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .init_resource::<CustomText>()
        // Systems that create Egui widgets should be run during the `CoreSet::Update` set,
        // or after the `EguiSet::BeginFrame` system (which belongs to the `CoreSet::PreUpdate` set).
        .add_system(ui_edit)
        .add_system(clipboard_copy.run_if(input_just_pressed(KeyCode::C)))
        .add_system(clipboard_paste.run_if(input_just_pressed(KeyCode::V)))
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

fn clipboard_paste(mut text: ResMut<CustomText>) {
    let _task = spawn_local(async move {
        let window = web_sys::window().expect("window"); // { obj: val };

        let nav = window.navigator();
        let clipboard = nav.clipboard();

        let Ok(permissions) = nav.permissions() else {
            return;
        };
        let clipboard_permission_desc = js_sys::Object::new();
        js_sys::Reflect::set(
            &clipboard_permission_desc,
            &"name".into(),
            // 'clipboard-read' fails on firefox because it's not implemented.
            // more info: https://developer.mozilla.org/en-US/docs/Web/API/Permissions_API#browser_compatibility
            &"clipboard-read".into(),
        );
        dbg!(permissions.query(&clipboard_permission_desc.into()));
        match clipboard {
            Some(a) => {
                let p = a.read();
                let result = wasm_bindgen_futures::JsFuture::from(p)
                    .await
                    .expect("clipboard populated");
                dbg!("result: ", &result);
                info!("clippyboy worked");
            }
            None => {
                warn!("failed to read clippyboy");
            }
        };
    });
}
