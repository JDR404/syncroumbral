//! SyncroUmbral como servicio residente: icono en la bandeja, ventana principal bajo demanda e inicio con Windows.
//! Cerrar la ventana la destruye (libera su WebView) pero el agente sigue recibiendo notificaciones.

pub use crate::autostart::{autostart_enabled, migrate_legacy_autostart, set_autostart};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Runtime, WebviewWindowBuilder};

/// Muestra la ventana principal; si se había cerrado, la crea de nuevo.
pub fn open_main<R: Runtime>(h: &AppHandle<R>) {
    if let Some(w) = h.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    // La ventana se crea desde otro hilo. Esta función se llama desde la bandeja y desde el aviso de "segunda instancia", que
    // corren en el hilo principal de Windows: crear allí un WebView espera al propio hilo principal y la aplicación se
    // queda colgada ("dejó de interactuar con Windows"). Era justo lo que pasaba al abrir la app dos veces.
    static BUILDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if BUILDING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return; // ya se está creando: una segunda orden no debe crear otra
    }
    if let Some(cfg) = h.config().app.windows.first().cloned() {
        let h = h.clone();
        std::thread::spawn(move || {
            if h.get_webview_window("main").is_none() {
                if let Ok(builder) = WebviewWindowBuilder::from_config(&h, &cfg) {
                    let _ = builder.build();
                }
            }
            BUILDING.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    } else {
        BUILDING.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

pub fn setup_tray(app: &App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir SyncroUmbral", true, None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", "Bloquear ahora", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &lock, &quit])?;

    let mut tray = TrayIconBuilder::new()
        .tooltip("SyncroUmbral")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => open_main(app),
            "lock" => {
                if let Some(core) = app.try_state::<std::sync::Arc<crate::core::Core>>() {
                    core.lock_now();
                    core.notify(app);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                open_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

