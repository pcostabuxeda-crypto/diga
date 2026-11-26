mod audio;
mod sortida;
mod transcriure;

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

// shortcut: drecera i idioma fixos; quan B tingui config.rs, es llegeixen de la config.
// Ctrl+Alt+Space no: sovint ja la fa servir una altra app (p. ex. Claude Desktop).
const DRECERA: &str = "Ctrl+Alt+D";
const IDIOMA: &str = "ca";

#[derive(Default)]
struct Gravant(Mutex<Option<audio::Gravacio>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, ev| match ev.state() {
                    ShortcutState::Pressed => premuda(app),
                    ShortcutState::Released => deixada(app),
                })
                .build(),
        )
        .manage(Gravant::default())
        .setup(|app| {
            // Si una altra app ja té la drecera, Diga segueix viva perquè es pugui canviar.
            if let Err(e) = app.global_shortcut().register(DRECERA) {
                eprintln!("diga: no es pot registrar {DRECERA}: {e}");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error en arrencar Diga");
}

fn premuda(app: &AppHandle) {
    let gravant = app.state::<Gravant>();
    let mut g = gravant.0.lock().unwrap();
    if g.is_some() {
        return; // repetició de tecla mentre es manté premuda
    }
    match audio::comencar() {
        Ok(r) => {
            *g = Some(r);
            sortida::estat(app, "escoltant");
        }
        Err(e) => sortida::error(app, &e),
    }
}

fn deixada(app: &AppHandle) {
    let Some(gravacio) = app.state::<Gravant>().0.lock().unwrap().take() else {
        return;
    };
    sortida::estat(app, "transcrivint");
    let app = app.clone();
    // Fora del fil de la drecera: Whisper triga uns segons.
    std::thread::spawn(move || {
        let res = gravacio.aturar().and_then(|audio| {
            let text = transcriure::transcriure(&transcriure::cami_model(&app)?, &audio, IDIOMA)?;
            if text.is_empty() {
                return Err("No s'ha entès res".into());
            }
            sortida::copiar(&app, &text)
        });
        if let Err(e) = res {
            sortida::error(&app, &e);
        }
    });
}
