//! Pruebas sin ventana de verdad (backend de pruebas de Slint): el agente real con una carpeta de datos temporal.

use crate::glue::{wire, Ctx, SlintUi};
use crate::{boot, AppWindow};
use slint::{ComponentHandle, Model};
use std::time::Duration;

/// Deja correr el bucle de eventos un rato (las cargas de archivos y los avisos son asíncronos).
fn spin(ms: u64) {
    slint::Timer::single_shot(Duration::from_millis(ms), || {
        let _ = slint::quit_event_loop();
    });
    slint::run_event_loop().unwrap();
}

#[test]
fn lock_screen_unlock_notifications_and_lock() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let dir = std::env::temp_dir().join(format!("syncroumbral-slint-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let (core, _tls) = boot::init(&dir).unwrap();

    let win = AppWindow::new().unwrap();
    let ctx = Ctx::new(core.clone(), rt.handle().clone(), win.as_weak());
    wire(&win, &ctx);
    let ui = SlintUi(ctx.clone());

    // Arranca bloqueado y sin clave: pide crearla.
    ctx.push_state();
    spin(200);
    assert!(win.get_locked());
    assert!(!win.get_has_password());

    // Una clave demasiado corta no abre.
    win.invoke_unlock("corta".into());
    spin(200);
    assert!(win.get_locked());
    assert!(!win.get_lock_error().is_empty());

    // Crear la clave abre y carga los archivos del PC.
    win.invoke_unlock("clave-de-prueba-1".into());
    spin(1500);
    assert!(!win.get_locked(), "la clave nueva debe desbloquear");
    assert!(win.get_pc_files().row_count() > 0, "se listan las carpetas del PC");
    assert!(win.get_phone_message().to_string().contains("no está conectado"));

    // Llega una notificación con respuesta: aparece, y al descartarla (sin móvil) se quita del PC.
    core.push_notif(&ui, &serde_json::json!({"key":"k1","pkg":"com.x","app":"X","title":"Hola","text":"texto","when":0,"quiet":true,
        "actions":[{"index":3,"title":"Responder","reply":true,"hint":"Responder"},{"index":4,"title":"Pausa"}]}));
    spin(300);
    assert_eq!(win.get_notifs().row_count(), 1);
    let n = win.get_notifs().row_data(0).unwrap();
    assert_eq!((n.reply_index, n.actions.row_count()), (3, 1));
    win.invoke_notif_dismiss("k1".into());
    spin(500);
    assert_eq!(win.get_notifs().row_count(), 0);

    // El código QR de vinculación se genera.
    win.invoke_make_qr();
    spin(300);
    assert!(win.get_qr().size().width > 0);

    // Bloquear limpia la pantalla.
    win.invoke_lock_now();
    spin(300);
    assert!(win.get_locked());
    assert_eq!(win.get_pc_files().row_count(), 0);
    assert_eq!(win.get_qr().size().width, 0);
    let _ = std::fs::remove_dir_all(&dir);
}
