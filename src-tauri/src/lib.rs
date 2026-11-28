mod audio;
mod config;
mod models;
mod net;
mod safata;
mod sortida;
mod text;
mod transcriure;

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Default)]
struct Gravant(Mutex<Option<audio::Gravacio>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
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
            let config::Config { drecera, inici_amb_windows, .. } = config::carregar(app.handle());
            if let Err(e) = app.global_shortcut().register(drecera.as_str()) {
                eprintln!("diga: no es pot registrar {drecera}: {e}");
            }
            if let Err(e) = config::aplicar_inici(app.handle(), inici_amb_windows) {
                eprintln!("diga: {e}");
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
            en_viu(app.clone(), r.en_curs());
            *g = Some(r);
            sortida::estat(app, "escoltant");
        }
        Err(e) => sortida::error(app, &e),
    }
}

/// Mentre es parla, va transcrivint el que porta gravat i ho mostra al rètol.
/// De pas, carrega el model abans de deixar la drecera.
fn en_viu(app: AppHandle, en_curs: audio::EnCurs) {
    std::thread::spawn(move || {
        let (Ok(cami), Ok(vad)) = (transcriure::cami_model(&app), transcriure::cami_vad(&app)) else {
            return;
        };
        let config::Config { idioma, vocabulari, .. } = config::carregar(&app);
        loop {
            std::thread::sleep(Duration::from_millis(300));
            let Some(audio) = en_curs.audio() else { return };
            // En deixar la drecera s'avorta la passada: la final no ha d'esperar-la.
            let viu = en_curs.clone();
            let parcial = transcriure::transcriure(&cami, &vad, &audio, &idioma, &vocabulari, move || !viu.viu());
            // Els errors (massa curt, silenci, sense model) ja els dirà la passada final.
            if let Ok(text) = parcial.as_deref()
                && !text.is_empty()
                && en_curs.viu()
            {
                sortida::parcial(&app, text);
            }
        }
    });
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
            let (cami, vad) = (transcriure::cami_model(&app)?, transcriure::cami_vad(&app)?);
            let text = transcriure::transcriure(&cami, &vad, &audio, &idioma, &vocabulari, || false)?;
            if text.is_empty() {
                return Err("No s'ha entès res".into());
            }
            if !mode_net {
                return sortida::copiar(&app, &text, None);
            }
            // Mode net: si Claude falla, es copia igualment el text del Whisper i s'avisa.
            sortida::estat(&app, "endreçant");
            match config::clau_api() {
                None => sortida::copiar(&app, &text, Some("falta la clau d'API")),
                Some(clau) => match net::endrecar(&clau, &text, &vocabulari) {
                    Ok(net) => sortida::copiar(&app, &net, None),
                    Err(e) => sortida::copiar(&app, &text, Some(&e)),
                },
            }
        });
        if let Err(e) = res {
            sortida::error(&app, &e);
        }
    });
}
