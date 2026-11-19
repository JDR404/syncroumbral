//! Clientes web oficiales (WhatsApp Web, Instagram...) en ventanas propias con la sesión guardada
//! en disco, para no volver a escanear el código. Al bloquear SyncroUmbral solo se cierran las ventanas;
//! la sesión se conserva. Son páginas remotas: no tienen acceso a los comandos internos de SyncroUmbral.

use crate::core::Core;
use serde::Serialize;
use std::path::PathBuf;
use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Rect, Runtime, Url, WebviewUrl};

use crate::services::{find, SERVICES};

#[derive(Serialize)]
pub struct ServiceInfo {
    id: &'static str,
    name: &'static str,
    has_session: bool,
    open: bool,
}

fn label(id: &str) -> String {
    format!("web-{id}")
}

fn data_dir(core: &Core, id: &str) -> PathBuf {
    core.dir.join("web").join(id)
}

pub fn list<R: Runtime>(h: &AppHandle<R>, core: &Core) -> Vec<ServiceInfo> {
    SERVICES
        .iter()
        .map(|s| ServiceInfo {
            id: s.id,
            name: s.name,
            has_session: data_dir(core, s.id).exists(),
            open: h.get_webview(&label(s.id)).is_some(),
        })
        .collect()
}

/// ¿La URL pertenece a alguno de los dominios permitidos? Solo https (y blob/about, que usan estas webs).
fn allowed(url: &Url, hosts: &[&str]) -> bool {
    match url.scheme() {
        "blob" | "about" => true,
        "https" => url
            .host_str()
            .map(|h| hosts.iter().any(|d| h == *d || h.ends_with(&format!(".{d}"))))
            .unwrap_or(false),
        _ => false,
    }
}

/// Los enlaces que salen de la web (por ejemplo una URL en un chat) se abren en el navegador, no dentro de SyncroUmbral.
fn open_in_browser(url: &Url) {
    if matches!(url.scheme(), "http" | "https") {
        let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url.as_str()]).spawn();
    }
}

/// Área (en píxeles lógicos, respecto a la ventana principal) donde se muestra el servicio.
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

fn bounds(a: &Area) -> Rect {
    Rect {
        position: LogicalPosition::new(a.x, a.y).into(),
        size: LogicalSize::new(a.w.max(50.0), a.h.max(50.0)).into(),
    }
}

/// Muestra el servicio DENTRO de la ventana principal, sobre el área indicada (un webview hijo). Oculta los demás.
/// La primera vez crea el webview, con su perfil propio: la sesión vive en disco y sobrevive a cerrar la ventana.
pub fn show_embedded<R: Runtime>(h: &AppHandle<R>, core: &Core, id: &str, area: &Area) -> Result<(), String> {
    // La interfaz pide mostrar/colocar a la vez (al elegir y al cambiar de tamaño): una petición a la vez, para no crear dos.
    static EMBED_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = EMBED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if core.is_locked() {
        return Err("La aplicación está bloqueada".into());
    }
    let s = find(id).ok_or("Servicio desconocido")?;
    for other in SERVICES.iter().filter(|o| o.id != id) {
        if let Some(wv) = h.get_webview(&label(other.id)) {
            let _ = wv.hide();
        }
    }
    if let Some(wv) = h.get_webview(&label(id)) {
        wv.set_bounds(bounds(area)).map_err(|e| e.to_string())?;
        let _ = wv.show();
        let _ = wv.set_focus();
        return Ok(());
    }
    let window = h.get_window("main").ok_or("La ventana principal no está abierta")?;
    let hosts = s.hosts;
    let url = s.url.parse().map_err(|_| "URL no válida")?;
    let builder = WebviewBuilder::new(label(id), WebviewUrl::External(url));
    // Solo en compilaciones de prueba: puerto propio para inspeccionar este servicio (cada perfil es un entorno aparte).
    #[cfg(feature = "webdebug")]
    let builder = builder.additional_browser_args(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port=9444",
    );
    let builder = builder
        .data_directory(data_dir(core, id))
        .on_navigation(move |url| {
            if allowed(url, hosts) {
                true
            } else {
                open_in_browser(url);
                false
            }
        });
    window
        .add_child(builder, LogicalPosition::new(area.x, area.y), LogicalSize::new(area.w.max(50.0), area.h.max(50.0)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Oculta los servicios (al salir de la pestaña). Siguen cargados: no hay que volver a entrar.
pub fn hide_embedded<R: Runtime>(h: &AppHandle<R>) {
    for s in SERVICES {
        if let Some(wv) = h.get_webview(&label(s.id)) {
            let _ = wv.hide();
        }
    }
}

/// Cierra todos los servicios (al bloquear). La sesión guardada no se borra.
pub fn close_all<R: Runtime>(h: &AppHandle<R>) {
    for (label, wv) in h.webviews() {
        if label.starts_with("web-") {
            let _ = wv.close();
        }
    }
}

/// Cierra el servicio y borra la sesión guardada.
pub fn forget<R: Runtime>(h: &AppHandle<R>, core: &Core, id: &str) -> Result<(), String> {
    find(id).ok_or("Servicio desconocido")?;
    if let Some(wv) = h.get_webview(&label(id)) {
        let _ = wv.close();
    }
    let dir = data_dir(core, id);
    // WebView2 puede tardar un instante en soltar los archivos.
    for _ in 0..15 {
        if !dir.exists() || std::fs::remove_dir_all(&dir).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Err("No se pudo borrar la sesión; cierra SyncroUmbral y vuelve a intentarlo".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_stays_inside_the_service_domains() {
        let wa = find("whatsapp").unwrap().hosts;
        let ok = |u: &str| allowed(&u.parse().unwrap(), wa);
        assert!(ok("https://web.whatsapp.com/"));
        assert!(ok("https://static.whatsapp.net/rsrc.php/x.js"));
        assert!(ok("https://media.fbcdn.net/a.jpg"));
        assert!(ok("blob:https://web.whatsapp.com/abc"));
        assert!(!ok("http://web.whatsapp.com/"), "solo https");
        assert!(!ok("https://evil.com/"));
        assert!(!ok("https://whatsapp.com.evil.com/"), "el dominio debe terminar en el permitido");
        assert!(!ok("https://notwhatsapp.com/"));
        assert!(!ok("file:///C:/Windows/win.ini"));
        assert!(!ok("javascript:alert(1)"));
    }
}
