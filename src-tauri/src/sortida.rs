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
}

fn emetre(app: &AppHandle, estat: Estat) {
    let _ = app.emit("diga://estat", estat);
}

pub fn estat(app: &AppHandle, estat: &str) {
    emetre(app, Estat { estat, text: None, error: None });
}

pub fn error(app: &AppHandle, error: &str) {
    eprintln!("diga: {error}");
    emetre(app, Estat { estat: "error", text: None, error: Some(error) });
}

pub fn copiar(app: &AppHandle, text: &str) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())?;
    emetre(app, Estat { estat: "copiat", text: Some(text), error: None });
    Ok(())
}
