mod audio;
mod config;
mod models;
mod net;
mod safata;
mod sortida;
mod transcriure;

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Default)]
struct Gravant(Mutex<Option<audio::Gravacio>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, drecera, ev| match ev.state() {
                    // Només la drecera de dictat: el mode codi està aparcat.
                    ShortcutState::Pressed if es_dictat(app, drecera) => premuda(app),
                    ShortcutState::Pressed => {}
                    ShortcutState::Released => deixada(app), // no fa res si no s'està gravant
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            config::get_config,
            config::set_config,
            config::te_clau_api,
            config::set_clau_api,
            models::model_present,
            models::descarregar_model,
            audio::llista_micros,
        ])
        .manage(Gravant::default())
        .setup(|app| {
            safata::crear(app)?;
            // Si una altra app ja té la drecera, Diga segueix viva perquè es pugui canviar.
            let drecera = config::carregar(app.handle()).drecera;
            if let Err(e) = app.global_shortcut().register(drecera.as_str()) {
                eprintln!("diga: no es pot registrar {drecera}: {e}");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error en arrencar Diga");
}

fn es_dictat(app: &AppHandle, drecera: &Shortcut) -> bool {
    config::carregar(app).drecera.parse::<Shortcut>().is_ok_and(|d| &d == drecera)
}

fn premuda(app: &AppHandle) {
    let gravant = app.state::<Gravant>();
    let mut g = gravant.0.lock().unwrap();
    if g.is_some() {
        return; // repetició de tecla mentre es manté premuda
    }
    match audio::comencar(config::carregar(app).micro) {
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
    let config::Config { idioma, vocabulari, mode_net, .. } = config::carregar(app);
    let app = app.clone();
    // Fora del fil de la drecera: Whisper triga uns segons.
    std::thread::spawn(move || {
        let res = gravacio.aturar().and_then(|audio| {
            let text = transcriure::transcriure(&transcriure::cami_model(&app)?, &audio, &idioma, &vocabulari)?;
            if text.is_empty() {
                return Err("No s'ha entès res".into());
            }
            if !mode_net {
                return sortida::copiar(&app, &text, None);
            }
            // Mode net: si Claude falla, es copia igualment el text del Whisper i s'avisa.
            sortida::estat(&app, "endreçant");
            match config::clau_api() {
                None => sortida::copiar(&app, &text, Some("Mode net sense clau d'API: copiat tal qual")),
                Some(clau) => match net::endrecar(&clau, &text, &vocabulari) {
                    Ok(net) => sortida::copiar(&app, &net, None),
                    Err(e) => sortida::copiar(&app, &text, Some(&format!("{e}: copiat tal qual"))),
                },
            }
        });
        if let Err(e) = res {
            sortida::error(&app, &e);
        }
    });
}
