use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Contingut de l'esdeveniment `diga://estat` (vegeu PLA.md).
#[derive(Clone, Serialize)]
struct Estat<'a> {
    estat: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
    /// Amb `copiat`: el mode net ha fallat i s'ha copiat el text del Whisper tal qual.
    #[serde(skip_serializing_if = "Option::is_none")]
    avis: Option<&'a str>,
}

fn emetre(app: &AppHandle, estat: Estat) {
    let _ = app.emit("diga://estat", estat);
}

pub fn estat(app: &AppHandle, estat: &str) {
    emetre(app, Estat { estat, text: None, error: None, avis: None });
}

/// El text que es va entenent mentre es parla.
pub fn parcial(app: &AppHandle, text: &str) {
    emetre(app, Estat { estat: "escoltant", text: Some(text), error: None, avis: None });
}

pub fn error(app: &AppHandle, error: &str) {
    eprintln!("diga: {error}");
    emetre(app, Estat { estat: "error", text: None, error: Some(error), avis: None });
}

pub fn copiar(app: &AppHandle, text: &str, avis: Option<&str>) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())?;
    if let Some(a) = avis {
        eprintln!("diga: {a}");
    }
    emetre(app, Estat { estat: "copiat", text: Some(text), error: None, avis });
    // El text ja és al porta-retalls: si enganxar falla, n'hi ha prou amb avisar al log.
    if crate::config::carregar(app).enganxar
        && let Err(e) = enganxar()
    {
        eprintln!("diga: no s'ha pogut enganxar: {e}");
    }
    Ok(())
}

/// Simula Ctrl+V on hi ha el cursor, quan l'usuari ja ha deixat anar Ctrl/Alt/Maj/Win de la drecera
/// (si no, l'app rebria Ctrl+Alt+V).
#[cfg(windows)]
fn enganxar() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let fins = Instant::now() + Duration::from_secs(2);
    while modificadors_premuts() {
        if Instant::now() > fins {
            return Err("encara hi ha tecles de la drecera premudes".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut teclat = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    teclat.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
    let v = teclat.key(Key::V, Direction::Click);
    let ctrl = teclat.key(Key::Control, Direction::Release); // sempre, encara que la V falli
    v.and(ctrl).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn modificadors_premuts() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };
    // El bit alt (valor negatiu) vol dir que la tecla està premuda ara mateix.
    [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
        .into_iter()
        .any(|tecla| unsafe { GetAsyncKeyState(i32::from(tecla)) } < 0)
}

#[cfg(not(windows))]
fn enganxar() -> Result<(), String> {
    Err("de moment només a Windows".into())
}
