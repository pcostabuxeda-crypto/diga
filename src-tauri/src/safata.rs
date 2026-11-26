use tauri::{
    App, AppHandle, Manager, WindowEvent,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

/// Icona de la safata amb el menú Configuració / Sortir.
pub fn crear(app: &mut App) -> tauri::Result<()> {
    let configuracio = MenuItem::with_id(app, "configuracio", "Configuració", true, None::<&str>)?;
    let sortir = MenuItem::with_id(app, "sortir", "Sortir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&configuracio, &sortir])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("falta la icona de l'app").clone())
        .tooltip("Diga")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "configuracio" => mostrar_configuracio(app),
            "sortir" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    // Tancar la finestra només l'amaga: Diga continua viva a la safata.
    if let Some(finestra) = app.get_webview_window("main") {
        let f = finestra.clone();
        finestra.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = f.hide();
            }
        });
    }

    // Primer cop (sense model): obre la configuració perquè es vegi el botó de baixar-lo.
    if crate::models::model_present(app.handle().clone()) == Ok(false) {
        mostrar_configuracio(app.handle());
    }
    Ok(())
}

fn mostrar_configuracio(app: &AppHandle) {
    if let Some(finestra) = app.get_webview_window("main") {
        let _ = finestra.show();
        let _ = finestra.unminimize();
        let _ = finestra.set_focus();
    }
}
