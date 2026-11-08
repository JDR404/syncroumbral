//! SyncroUmbral como servicio residente: icono en la bandeja, ventana principal bajo demanda e inicio con Windows.
//! Cerrar la ventana la destruye (libera su WebView) pero el agente sigue recibiendo notificaciones.

use std::process::Command;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Runtime, WebviewWindowBuilder};

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "SyncroUmbral";

/// Muestra la ventana principal; si se había cerrado, la crea de nuevo.
pub fn open_main<R: Runtime>(h: &AppHandle<R>) {
    if let Some(w) = h.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    if let Some(cfg) = h.config().app.windows.first() {
        if let Ok(builder) = WebviewWindowBuilder::from_config(h, cfg) {
            let _ = builder.build();
        }
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

fn reg(args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new("reg");
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.output()
}

pub fn autostart_enabled() -> bool {
    reg(&["query", RUN_KEY, "/v", RUN_VALUE]).map(|o| o.status.success()).unwrap_or(false)
}

/// Migración desde el nombre anterior de la aplicación: si el usuario tenía el inicio con Windows activado con la entrada
/// vieja ("Umbral", que apunta al ejecutable antiguo), se quita y se crea la nueva apuntando a este ejecutable.
pub fn migrate_legacy_autostart() {
    const OLD: &str = "Umbral";
    if reg(&["query", RUN_KEY, "/v", OLD]).map(|o| o.status.success()).unwrap_or(false) {
        let _ = reg(&["delete", RUN_KEY, "/v", OLD, "/f"]);
        let _ = set_autostart(true);
    }
}

/// Activa o desactiva el inicio con Windows (en segundo plano, sin abrir la ventana).
pub fn set_autostart(on: bool) -> Result<(), String> {
    let out = if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let value = format!("\"{}\" --background", exe.display());
        reg(&["add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &value, "/f"])
    } else {
        reg(&["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
    }
    .map_err(|e| e.to_string())?;
    if out.status.success() || !on {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
