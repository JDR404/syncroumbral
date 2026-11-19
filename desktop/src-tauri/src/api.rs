//! Operaciones del agente que no dependen de la interfaz (las usan los comandos de Tauri y la interfaz Slint).

use crate::core::{Core, PORT};
use crate::transfer::Item;
use qrcode::render::svg;
use qrcode::QrCode;
use serde::Serialize;
use serde_json::json;
use std::net::IpAddr;

#[derive(Serialize)]
pub struct PairingInfo {
    pub payload: String,
    pub qr_svg: String,
    pub expires_secs: u64,
    /// Huella corta del certificado ("ab12-cd34-ef56") para compararla con la que muestra el móvil al confirmar.
    pub fp_short: String,
}

pub fn pairing_info(core: &Core) -> Result<PairingInfo, String> {
    if core.is_locked() {
        return Err("Desbloquea la aplicación primero".into());
    }
    let hosts: Vec<String> = local_ip_address::list_afinet_netifas()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, ip)| match ip {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() => Some(v4.to_string()),
            _ => None,
        })
        .collect();
    let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC".into());
    let payload = json!({
        "v": 1,
        "name": name,
        "hosts": hosts,
        "port": PORT,
        "fp": core.tls_fp,
        "tok": core.new_token(),
    })
    .to_string();
    let qr_svg = QrCode::new(payload.as_bytes())
        .map_err(|e| e.to_string())?
        .render::<svg::Color>()
        .min_dimensions(260, 260)
        .quiet_zone(true)
        .build();
    let fp_short = core
        .tls_fp
        .as_bytes()
        .chunks(4)
        .take(3)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join("-");
    Ok(PairingInfo { payload, qr_svg, expires_secs: 300, fp_short })
}

#[derive(Serialize)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    pub size: u64,
    pub mtime: u64,
}

#[derive(Serialize)]
pub struct LocalListing {
    pub path: String,
    /// Carpeta superior; "" = lista de unidades; None = ya estamos en la lista de unidades.
    pub parent: Option<String>,
    pub items: Vec<Entry>,
}

pub fn ensure_unlocked(core: &Core) -> Result<(), String> {
    if core.is_locked() {
        Err("La aplicación está bloqueada".into())
    } else {
        Ok(())
    }
}

pub fn local_ls(core: &Core, path: String) -> Result<LocalListing, String> {
    ensure_unlocked(core)?;
    if path.is_empty() {
        let items = ('A'..='Z')
            .filter(|c| std::path::Path::new(&format!("{c}:\\")).exists())
            .map(|c| Entry { name: format!("{c}:"), dir: true, size: 0, mtime: 0 })
            .collect();
        return Ok(LocalListing { path, parent: None, items });
    }
    let p = std::path::Path::new(&path);
    let mut items: Vec<Entry> = std::fs::read_dir(p)
        .map_err(|e| format!("No se puede abrir la carpeta: {e}"))?
        .flatten()
        .filter_map(|e| {
            let md = e.metadata().ok()?;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                dir: md.is_dir(),
                size: if md.is_dir() { 0 } else { md.len() },
                mtime: md
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis() as u64),
            })
        })
        .collect();
    items.sort_by(|a, b| b.dir.cmp(&a.dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    let parent = Some(p.parent().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default());
    Ok(LocalListing { path, parent, items })
}

/// Convierte rutas del PC (por ejemplo soltadas desde el Explorador) en elementos para transferir.
pub fn local_stat(core: &Core, paths: Vec<String>) -> Result<Vec<Item>, String> {
    ensure_unlocked(core)?;
    Ok(paths
        .into_iter()
        .filter_map(|p| {
            let md = std::fs::symlink_metadata(&p).ok()?;
            if md.file_type().is_symlink() {
                return None;
            }
            let name = std::path::Path::new(&p).file_name()?.to_string_lossy().into_owned();
            Some(Item { path: p, name, dir: md.is_dir(), size: if md.is_dir() { 0 } else { md.len() } })
        })
        .collect())
}


pub fn ensure_advanced(core: &Core) -> Result<(), String> {
    if core.advanced() {
        Ok(())
    } else {
        Err("El modo avanzado está desactivado. Actívalo en Móviles y ajustes (o en la app del móvil) para usar esto.".into())
    }
}

/// Abre la app del móvil en una ventana del PC (pantalla virtual: el móvil puede seguir bloqueado). Bloquea: usar en un hilo aparte.
pub fn app_open_pc_blocking(core: &std::sync::Arc<Core>, pkg: &str, label: &str) -> Result<String, String> {
    ensure_unlocked(core)?;
    ensure_advanced(core)?;
    if !crate::adb::valid_pkg(pkg) {
        return Err("Nombre de paquete no válido".into());
    }
    let serial = crate::adb::find_device(core.phone_ip().as_deref())?;
    crate::appwin::open(&core.app_windows, &serial, pkg, label)
}

/// Acción sobre una app: launch, stop, settings, clear, uninstall, pull. Bloquea: usar en un hilo aparte.
pub fn app_action_blocking(core: &std::sync::Arc<Core>, pkg: &str, action: &str, local_dir: Option<String>) -> Result<String, String> {
    use crate::adb;
    ensure_unlocked(core)?;
    ensure_advanced(core)?;
    if !adb::valid_pkg(pkg) {
        return Err("Nombre de paquete no válido".into());
    }
    let serial = adb::find_device(core.phone_ip().as_deref())?;
    // Detener, borrar datos o desinstalar deja sin contenido la ventana de esa app en el PC: se cierra también.
    let close_window = |r: Result<String, String>| {
        if r.is_ok() {
            crate::appwin::close_one(&core.app_windows, pkg);
        }
        r
    };
    match action {
        "launch" => adb::launch(&serial, pkg),
        "stop" => close_window(adb::force_stop(&serial, pkg)),
        "settings" => adb::app_settings(&serial, pkg),
        "clear" => close_window(adb::clear_data(&serial, pkg)),
        "uninstall" => close_window(adb::uninstall(&serial, pkg)),
        "pull" => {
            let dir = local_dir.ok_or("Falta la carpeta de destino")?;
            let dir = std::path::Path::new(&dir);
            if !dir.is_dir() {
                return Err("La carpeta de destino no existe".to_string());
            }
            adb::pull_apk(&serial, pkg, dir).map(|p| format!("APK guardado en {p}"))
        }
        _ => Err("Acción desconocida".to_string()),
    }
}
