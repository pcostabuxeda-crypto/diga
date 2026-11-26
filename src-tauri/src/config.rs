use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_store::StoreExt;

const FITXER: &str = "config.json";
const CLAU_STORE: &str = "config";
const SERVEI_CREDENCIAL: &str = "cat.diga.app";
const USUARI_CREDENCIAL: &str = "anthropic-api";

/// Es desa a `app_data_dir/config.json`. La clau d'API NO hi va: és al Gestor de credencials.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub drecera: String,
    pub mode: String,
    pub idioma: String,
    pub enganxar: bool,
    pub inici_amb_windows: bool,
    pub drecera_codi: String,
    pub llenguatge_codi: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            drecera: "Ctrl+Alt+D".into(),
            mode: "mantenir".into(),
            idioma: "ca".into(),
            enganxar: false,
            inici_amb_windows: false,
            drecera_codi: "Ctrl+Alt+C".into(),
            llenguatge_codi: "JavaScript".into(),
        }
    }
}

/// Config desada, o la per defecte si no n'hi ha o està malmesa.
pub fn carregar(app: &AppHandle) -> Config {
    app.store(FITXER)
        .ok()
        .and_then(|s| s.get(CLAU_STORE))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Config {
    carregar(&app)
}

/// Desa la config i torna a registrar les dreceres que hagin canviat.
/// Si una drecera no es pot registrar, no es desa res i les velles continuen actives.
#[tauri::command]
pub fn set_config(app: AppHandle, config: Config) -> Result<(), String> {
    if config.drecera.trim().is_empty() || config.drecera_codi.trim().is_empty() {
        return Err("Les dreceres no poden quedar buides.".into());
    }
    if config.drecera.eq_ignore_ascii_case(&config.drecera_codi) {
        return Err("La drecera de dictat i la del mode codi han de ser diferents.".into());
    }

    let vella = carregar(&app);
    canviar_drecera(&app, &vella.drecera, &config.drecera)?;
    if let Err(e) = canviar_drecera(&app, &vella.drecera_codi, &config.drecera_codi) {
        let _ = canviar_drecera(&app, &config.drecera, &vella.drecera);
        return Err(e);
    }

    let store = app.store(FITXER).map_err(|e| e.to_string())?;
    store.set(CLAU_STORE, serde_json::to_value(&config).map_err(|e| e.to_string())?);
    store.save().map_err(|e| format!("No s'ha pogut desar la configuració: {e}"))
}

fn canviar_drecera(app: &AppHandle, vella: &str, nova: &str) -> Result<(), String> {
    if vella.eq_ignore_ascii_case(nova) {
        return Ok(());
    }
    let nova_drecera: Shortcut = nova
        .parse()
        .map_err(|_| format!("No entenc la drecera «{nova}». Escriu-la com «Ctrl+Alt+D»."))?;
    let dreceres = app.global_shortcut();
    dreceres
        .register(nova_drecera)
        .map_err(|e| format!("Aquesta drecera («{nova}») la fa servir una altra app: tria-ne una altra. ({e})"))?;
    let _ = dreceres.unregister(vella); // pot ser que la vella no estigués registrada
    Ok(())
}

fn credencial() -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVEI_CREDENCIAL, USUARI_CREDENCIAL)
}

/// La clau d'API d'Anthropic, per a `codi.rs`. Mai s'envia a la UI.
pub fn clau_api() -> Option<String> {
    credencial().ok()?.get_password().ok()
}

#[tauri::command]
pub fn te_clau_api() -> bool {
    clau_api().is_some()
}

/// Desa la clau al Gestor de credencials de Windows. Buida = esborrar-la.
#[tauri::command]
pub fn set_clau_api(clau: String) -> Result<(), String> {
    let entrada = credencial().map_err(|e| e.to_string())?;
    let clau = clau.trim();
    if clau.is_empty() {
        return match entrada.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        };
    }
    if !clau.starts_with("sk-ant-") {
        return Err("No sembla una clau d'Anthropic: ha de començar per «sk-ant-».".into());
    }
    entrada
        .set_password(clau)
        .map_err(|e| format!("No s'ha pogut desar la clau: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_antiga_omple_els_camps_nous_amb_els_valors_per_defecte() {
        let antiga = serde_json::json!({ "drecera": "Ctrl+Shift+D", "enganxar": true });
        let config: Config = serde_json::from_value(antiga).unwrap();
        assert_eq!(config.drecera, "Ctrl+Shift+D");
        assert!(config.enganxar);
        assert_eq!(config.drecera_codi, Config::default().drecera_codi);
        assert_eq!(config.idioma, "ca");
    }

    #[test]
    fn les_dreceres_per_defecte_es_poden_llegir() {
        let config = Config::default();
        assert!(config.drecera.parse::<Shortcut>().is_ok());
        assert!(config.drecera_codi.parse::<Shortcut>().is_ok());
        assert!("Ctrl+Alt+".parse::<Shortcut>().is_err());
    }
}
