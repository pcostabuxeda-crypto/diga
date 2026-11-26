mod config;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            config::get_config,
            config::set_config,
            config::te_clau_api,
            config::set_clau_api,
        ])
        .run(tauri::generate_context!())
        .expect("error en arrencar Diga");
}
