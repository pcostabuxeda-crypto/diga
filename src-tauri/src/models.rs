use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::transcriure::{MODEL, cami_model};

const ORIGEN: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";
const CADA: Duration = Duration::from_millis(150); // no inundar la UI d'esdeveniments

static BAIXANT: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
struct Progres {
    baixat: u64,
    total: u64,
}

#[tauri::command]
pub fn model_present(app: AppHandle) -> Result<bool, String> {
    Ok(cami_model(&app)?.exists())
}

/// Baixa el model a `app_data_dir/models` i envia `diga://progres-model`.
#[tauri::command]
pub async fn descarregar_model(app: AppHandle) -> Result<(), String> {
    if BAIXANT.swap(true, Ordering::SeqCst) {
        return Err("Ja s'està baixant el model.".into());
    }
    let res = tauri::async_runtime::spawn_blocking(move || baixar(&app))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
    BAIXANT.store(false, Ordering::SeqCst);
    res
}

fn baixar(app: &AppHandle) -> Result<(), String> {
    let desti = cami_model(app)?;
    if desti.exists() {
        return Ok(());
    }
    let dir = desti.parent().expect("el camí del model té carpeta");
    fs::create_dir_all(dir).map_err(|e| format!("No puc crear {}: {e}", dir.display()))?;

    // Es baixa a .part i es reanomena al final: mai queda un model a mitges amb el nom bo.
    let parcial = desti.with_extension("bin.part");
    let res = baixar_a(app, &parcial);
    match res {
        Ok(()) => fs::rename(&parcial, &desti).map_err(|e| format!("No puc desar el model: {e}")),
        Err(e) => {
            let _ = fs::remove_file(&parcial);
            Err(e)
        }
    }
}

// shortcut: només es comprova la mida (Content-Length), no el SHA-256; afegir-lo si algun dia es serveix des d'un mirall no fiable.
fn baixar_a(app: &AppHandle, parcial: &Path) -> Result<(), String> {
    let sense_xarxa = |e: ureq::Error| format!("No s'ha pogut baixar el model. Tens internet? ({e})");
    let mut resposta = ureq::get(format!("{ORIGEN}{MODEL}")).call().map_err(sense_xarxa)?;
    let total = resposta.body().content_length().unwrap_or(0);
    let mut lector = resposta.body_mut().as_reader();

    let error_disc = |e: std::io::Error| format!("No puc escriure el model al disc: {e}");
    let mut fitxer = BufWriter::new(File::create(parcial).map_err(error_disc)?);
    let mut buf = vec![0u8; 256 * 1024];
    let mut baixat = 0u64;
    let mut ultim = Instant::now();

    loop {
        let n = lector
            .read(&mut buf)
            .map_err(|e| format!("La baixada s'ha tallat: {e}"))?;
        if n == 0 {
            break;
        }
        fitxer.write_all(&buf[..n]).map_err(error_disc)?;
        baixat += n as u64;
        if ultim.elapsed() >= CADA {
            let _ = app.emit("diga://progres-model", Progres { baixat, total });
            ultim = Instant::now();
        }
    }
    fitxer.flush().map_err(error_disc)?;

    if total > 0 && baixat != total {
        return Err(format!("Baixada incompleta: {baixat} de {total} bytes."));
    }
    let _ = app.emit("diga://progres-model", Progres { baixat, total: total.max(baixat) });
    Ok(())
}
