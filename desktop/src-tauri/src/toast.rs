//! Ventana emergente de notificaciones. Se crea cuando llega una y se destruye al terminar,
//! para no mantener un WebView (memoria) abierto sin necesidad.

use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "toast";
const WIDTH: f64 = 340.0;
const MARGIN: f64 = 16.0;

pub fn show<R: Runtime>(h: &AppHandle<R>) {
    if let Some(w) = h.get_webview_window(LABEL) {
        let _ = w.show();
        return;
    }
    let built = WebviewWindowBuilder::new(h, LABEL, WebviewUrl::App("toast.html".into()))
        .title("SyncroUmbral: notificaciones")
        .inner_size(WIDTH, 140.0)
        .decorations(false)
        .transparent(true) // sin marco rectangular detrás: solo se ven las tarjetas redondeadas
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false) // se muestra cuando ya está colocada
        .build();
    if let Ok(w) = built {
        place(&w, 140.0);
        let _ = w.show();
    }
}

/// Destruye la ventana (libera su WebView).
pub fn hide<R: Runtime>(h: &AppHandle<R>) {
    if let Some(w) = h.get_webview_window(LABEL) {
        let _ = w.destroy();
    }
}

pub fn resize<R: Runtime>(h: &AppHandle<R>, height: f64) {
    if let Some(w) = h.get_webview_window(LABEL) {
        let height = height.clamp(80.0, 720.0);
        let _ = w.set_size(LogicalSize::new(WIDTH, height));
        place(&w, height);
    }
}

/// Esquina inferior derecha del área útil del monitor principal (sin tapar la barra de tareas).
fn place<R: Runtime>(w: &WebviewWindow<R>, height: f64) {
    let Ok(Some(m)) = w.primary_monitor() else { return };
    let scale = m.scale_factor();
    let area = m.work_area();
    let x = area.position.x as f64 + area.size.width as f64 - (WIDTH + MARGIN) * scale;
    let y = area.position.y as f64 + area.size.height as f64 - (height + MARGIN) * scale;
    let _ = w.set_position(PhysicalPosition::new(x as i32, y as i32));
}
