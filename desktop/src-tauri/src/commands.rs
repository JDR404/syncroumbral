//! Comandos que llama la interfaz.

use crate::core::{Core, Snapshot, PORT};
use crate::adb;
use crate::transfer::{self, Item};
use qrcode::render::svg;
use qrcode::QrCode;
use serde::Serialize;
use serde_json::{json, Value};
use std::net::IpAddr;
use std::sync::Arc;
use tauri::{AppHandle, State};

type Shared<'a> = State<'a, Arc<Core>>;

#[derive(Serialize)]
pub struct PairingInfo {
    payload: String,
    qr_svg: String,
    expires_secs: u64,
    /// Huella corta del certificado ("ab12-cd34-ef56") para compararla con la que muestra el móvil al confirmar.
    fp_short: String,
}

#[tauri::command]
pub fn get_state(core: Shared) -> Snapshot {
    core.snapshot()
}

#[tauri::command]
pub fn set_password(h: AppHandle, core: Shared, password: String) -> Result<(), String> {
    core.set_password(&password)?;
    core.notify(&h);
    Ok(())
}

#[tauri::command]
pub fn unlock(h: AppHandle, core: Shared, password: String) -> Result<(), String> {
    let r = core.unlock_with_password(&password);
    core.notify(&h);
    r
}

#[tauri::command]
pub fn lock(h: AppHandle, core: Shared) {
    core.lock_now();
    core.notify(&h);
}

#[tauri::command]
pub fn pairing_info(core: Shared) -> Result<PairingInfo, String> {
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

#[tauri::command]
pub fn remove_device(h: AppHandle, core: Shared, id: String) -> Result<(), String> {
    if core.is_locked() {
        return Err("Desbloquea la aplicación primero".into());
    }
    core.remove_device(&id);
    core.notify(&h);
    Ok(())
}

// ---------------------------------------------------------------- archivos

#[derive(Serialize)]
pub struct Entry {
    name: String,
    dir: bool,
    size: u64,
    mtime: u64,
}

#[derive(Serialize)]
pub struct LocalListing {
    path: String,
    /// Carpeta superior; "" = lista de unidades; None = ya estamos en la lista de unidades.
    parent: Option<String>,
    items: Vec<Entry>,
}

fn ensure_unlocked(core: &Core) -> Result<(), String> {
    if core.is_locked() {
        Err("La aplicación está bloqueada".into())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub fn local_home(core: Shared) -> Result<String, String> {
    ensure_unlocked(&core)?;
    Ok(std::env::var("USERPROFILE").unwrap_or_default())
}

/// Accesos directos a Escritorio, Descargas, Documentos e Imágenes (resueltos por Windows, también con OneDrive).
#[tauri::command]
pub fn local_places(core: Shared) -> Result<Vec<crate::places::Place>, String> {
    ensure_unlocked(&core)?;
    Ok(crate::places::places())
}

#[tauri::command]
pub fn local_ls(core: Shared, path: String) -> Result<LocalListing, String> {
    ensure_unlocked(&core)?;
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
#[tauri::command]
pub fn local_stat(core: Shared, paths: Vec<String>) -> Result<Vec<Item>, String> {
    ensure_unlocked(&core)?;
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

#[tauri::command]
pub async fn phone_ls(core: Shared<'_>, path: String) -> Result<Value, String> {
    core.call("ls", json!({ "path": path })).await
}

#[tauri::command]
pub async fn phone_mkdir(core: Shared<'_>, path: String) -> Result<(), String> {
    core.call("mkdir", json!({ "path": path })).await.map(|_| ())
}

#[tauri::command]
pub async fn phone_delete(core: Shared<'_>, paths: Vec<String>) -> Result<(), String> {
    for p in paths {
        core.call("delete", json!({ "path": p })).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn phone_rename(core: Shared<'_>, path: String, name: String) -> Result<(), String> {
    core.call("rename", json!({ "path": path, "name": name })).await.map(|_| ())
}

#[tauri::command]
pub fn transfer_download(h: AppHandle, core: Shared, items: Vec<Item>, local_dir: String) -> Result<u64, String> {
    ensure_unlocked(&core)?;
    let (id, core) = (core.next_job_id(), core.inner().clone());
    tauri::async_runtime::spawn(transfer::run_download(h, core, id, items, local_dir));
    Ok(id)
}

#[tauri::command]
pub fn transfer_upload(h: AppHandle, core: Shared, items: Vec<Item>, remote_dir: String) -> Result<u64, String> {
    ensure_unlocked(&core)?;
    let (id, core) = (core.next_job_id(), core.inner().clone());
    tauri::async_runtime::spawn(transfer::run_upload(h, core, id, items, remote_dir));
    Ok(id)
}

/// Cancela todas las transferencias en curso.
#[tauri::command]
pub fn cancel_transfers(core: Shared) {
    core.epoch.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

// ---------------------------------------------------------------- herramientas (adb, scrcpy)

#[tauri::command]
pub fn tools_status(core: Shared) -> Result<Value, String> {
    ensure_unlocked(&core)?;
    Ok(json!({
        "adb": adb::adb_version(),
        "scrcpy": crate::appwin::find_scrcpy().is_some(),
    }))
}

/// Descarga e instala `adb` o `scrcpy` en la carpeta de datos de SyncroUmbral.
#[tauri::command]
pub async fn tools_install(core: Shared<'_>, which: String) -> Result<String, String> {
    ensure_unlocked(&core)?;
    tauri::async_runtime::spawn_blocking(move || match which.as_str() {
        "adb" => crate::tools::install_adb().map(|_| "adb instalado".to_string()),
        "scrcpy" => crate::tools::install_scrcpy().map(|_| "scrcpy instalado".to_string()),
        _ => Err("Herramienta desconocida".to_string()),
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn adb_pairing_services(core: Shared<'_>) -> Result<Vec<String>, String> {
    ensure_unlocked(&core)?;
    tauri::async_runtime::spawn_blocking(adb::pairing_services).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn adb_pair(core: Shared<'_>, addr: String, code: String) -> Result<String, String> {
    ensure_unlocked(&core)?;
    tauri::async_runtime::spawn_blocking(move || adb::pair(&addr, &code)).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- inicio con Windows

#[tauri::command]
pub fn autostart_get() -> bool {
    crate::shell::autostart_enabled()
}

#[tauri::command]
pub fn autostart_set(core: Shared, on: bool) -> Result<(), String> {
    ensure_unlocked(&core)?;
    crate::shell::set_autostart(on)
}

// ---------------------------------------------------------------- servicios web

#[tauri::command]
pub fn web_list(h: AppHandle, core: Shared) -> Result<Vec<crate::web::ServiceInfo>, String> {
    ensure_unlocked(&core)?;
    Ok(crate::web::list(&h, &core))
}

/// Muestra el servicio web dentro de la ventana principal, en el rectángulo indicado (píxeles lógicos).
#[tauri::command]
pub async fn web_show(h: AppHandle, core: Shared<'_>, id: String, x: f64, y: f64, w: f64, height: f64) -> Result<(), String> {
    crate::web::show_embedded(&h, &core, &id, &crate::web::Area { x, y, w, h: height })
}

#[tauri::command]
pub fn web_hide(h: AppHandle) {
    crate::web::hide_embedded(&h);
}

#[tauri::command]
pub async fn web_forget(h: AppHandle, core: Shared<'_>, id: String) -> Result<(), String> {
    ensure_unlocked(&core)?;
    crate::web::forget(&h, &core, &id)
}

// ---------------------------------------------------------------- notificaciones

#[tauri::command]
pub fn notifs_list(core: Shared) -> Result<Value, String> {
    ensure_unlocked(&core)?;
    Ok(core.notifs_state())
}

#[tauri::command]
pub fn toast_items(core: Shared) -> Result<Vec<Value>, String> {
    ensure_unlocked(&core)?;
    Ok(core.popup_items())
}

#[tauri::command]
pub fn toast_hide(h: AppHandle, core: Shared) {
    core.clear_popups(&h);
}

#[tauri::command]
pub fn toast_resize(h: AppHandle, height: f64) {
    crate::toast::resize(&h, height);
}

#[tauri::command]
pub async fn notif_reply(core: Shared<'_>, key: String, index: u32, text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Escribe una respuesta".into());
    }
    core.call("notif_reply", json!({ "key": key, "index": index, "text": text })).await.map(|_| ())
}

#[tauri::command]
pub async fn notif_action(core: Shared<'_>, key: String, index: u32) -> Result<(), String> {
    core.call("notif_action", json!({ "key": key, "index": index })).await.map(|_| ())
}

/// Contesta una llamada entrante (normal o de una app como WhatsApp). Devuelve por dónde se hizo ("telefonia" o "notificacion").
#[tauri::command]
pub async fn call_answer(h: AppHandle, core: Shared<'_>, key: String) -> Result<String, String> {
    let r = core.call("call_answer", json!({ "key": key })).await?;
    // Al contestar desde el PC se abre la aplicación: la pantalla de llamada aparece sola en cuanto el móvil
    // confirma que la llamada está en curso.
    crate::shell::open_main(&h);
    Ok(r["via"].as_str().unwrap_or("").to_string())
}

/// Cuelga o rechaza una llamada.
#[tauri::command]
pub async fn call_end(core: Shared<'_>, key: String) -> Result<String, String> {
    let r = core.call("call_end", json!({ "key": key })).await?;
    Ok(r["via"].as_str().unwrap_or("").to_string())
}

/// Permisos de llamadas del móvil (contestar llamadas normales y mostrar sobre otras apps), para avisar si faltan.
#[tauri::command]
pub async fn call_perms(core: Shared<'_>) -> Result<Value, String> {
    core.call("call_perms", json!({})).await
}

#[tauri::command]
pub fn get_mute_on_call(core: Shared) -> bool {
    core.mute_on_call()
}

#[tauri::command]
pub fn set_mute_on_call(core: Shared, on: bool) -> Result<(), String> {
    ensure_unlocked(&core)?;
    core.set_mute_on_call(on);
    Ok(())
}

#[tauri::command]
pub fn get_advanced(core: Shared) -> bool {
    core.advanced()
}

/// Cambia entre modo básico y avanzado y se lo comunica al móvil (si está conectado).
#[tauri::command]
pub async fn set_advanced(core: Shared<'_>, on: bool) -> Result<(), String> {
    ensure_unlocked(&core)?;
    core.set_advanced(on);
    let _ = core.call("set_mode", json!({ "advanced": on })).await;
    Ok(())
}

fn ensure_advanced(core: &Core) -> Result<(), String> {
    if core.advanced() {
        Ok(())
    } else {
        Err("El modo avanzado está desactivado. Actívalo en Móviles y ajustes (o en la app del móvil) para usar esto.".into())
    }
}

#[tauri::command]
pub fn notif_mute(h: AppHandle, core: Shared, pkg: String, app: String) -> Result<(), String> {
    ensure_unlocked(&core)?;
    core.mute_app(&h, &pkg, &app);
    Ok(())
}

#[tauri::command]
pub fn notif_unmute(core: Shared, pkg: String) -> Result<(), String> {
    ensure_unlocked(&core)?;
    core.unmute_app(&pkg);
    Ok(())
}

#[tauri::command]
pub fn notif_muted(core: Shared) -> Result<Vec<Value>, String> {
    ensure_unlocked(&core)?;
    Ok(core.muted_list())
}

/// Muestra en el Explorador un archivo recibido del móvil. Solo dentro de Descargas\SyncroUmbral.
#[tauri::command]
pub fn show_received(core: Shared, path: String) -> Result<(), String> {
    ensure_unlocked(&core)?;
    let inbox = crate::share::inbox_dir().canonicalize().map_err(|e| e.to_string())?;
    let p = std::path::Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    if !p.starts_with(&inbox) {
        return Err("Ruta no permitida".into());
    }
    std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Copia un texto en el portapapeles del móvil.
#[tauri::command]
pub async fn phone_clip_set(core: Shared<'_>, text: String) -> Result<(), String> {
    core.call("clip_set", json!({ "text": text })).await.map(|_| ())
}

/// Descarta en el móvil todas las notificaciones que se pueden descartar, y quita del PC lo compartido desde el móvil.
#[tauri::command]
pub async fn notif_clear_all(h: AppHandle, core: Shared<'_>) -> Result<u64, String> {
    let r = core.call("notif_clear_all", json!({})).await?;
    core.remove_shared(&h);
    Ok(r["n"].as_u64().unwrap_or(0))
}

/// Quita una notificación solo del PC (las que no existen en el móvil: texto o archivos compartidos).
#[tauri::command]
pub fn notif_remove_local(h: AppHandle, core: Shared, key: String) -> Result<(), String> {
    ensure_unlocked(&core)?;
    core.remove_local(&h, &key);
    Ok(())
}

#[tauri::command]
pub async fn notif_dismiss(core: Shared<'_>, key: String) -> Result<(), String> {
    core.call("notif_dismiss", json!({ "key": key })).await.map(|_| ())
}

// ---------------------------------------------------------------- aplicaciones

#[tauri::command]
pub async fn apps_list(core: Shared<'_>) -> Result<Value, String> {
    core.call("apps", json!({})).await
}

#[tauri::command]
pub async fn apps_icons(core: Shared<'_>, pkgs: Vec<String>) -> Result<Value, String> {
    core.call("icons", json!({ "pkgs": pkgs })).await
}

/// Comprueba que el PC puede hablar con el móvil por adb; devuelve el identificador del dispositivo.
#[tauri::command]
pub async fn adb_status(core: Shared<'_>) -> Result<String, String> {
    ensure_unlocked(&core)?;
    ensure_advanced(&core)?;
    let ip = core.phone_ip();
    tauri::async_runtime::spawn_blocking(move || adb::find_device(ip.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

/// Abre la app del móvil en una ventana del PC (pantalla virtual: el móvil puede seguir bloqueado).
#[tauri::command]
pub async fn app_open_pc(core: Shared<'_>, pkg: String, label: String) -> Result<String, String> {
    ensure_unlocked(&core)?;
    ensure_advanced(&core)?;
    if !adb::valid_pkg(&pkg) {
        return Err("Nombre de paquete no válido".into());
    }
    let ip = core.phone_ip();
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let serial = adb::find_device(ip.as_deref())?;
        crate::appwin::open(&core.app_windows, &serial, &pkg, &label)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn app_windows(core: Shared) -> Result<Vec<String>, String> {
    ensure_unlocked(&core)?;
    Ok(crate::appwin::running(&core.app_windows))
}

/// Acciones sobre una app: launch, stop, settings, clear, uninstall, pull (extraer APK a `local_dir`).
#[tauri::command]
pub async fn app_action(core: Shared<'_>, pkg: String, action: String, local_dir: Option<String>) -> Result<String, String> {
    ensure_unlocked(&core)?;
    ensure_advanced(&core)?;
    if !adb::valid_pkg(&pkg) {
        return Err("Nombre de paquete no válido".into());
    }
    let ip = core.phone_ip();
    let core_arc = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let serial = adb::find_device(ip.as_deref())?;
        // Detener, borrar datos o desinstalar deja sin contenido la ventana de esa app en el PC: se cierra también.
        let close_window = |r: Result<String, String>| {
            if r.is_ok() {
                crate::appwin::close_one(&core_arc.app_windows, &pkg);
            }
            r
        };
        match action.as_str() {
            "launch" => adb::launch(&serial, &pkg),
            "stop" => close_window(adb::force_stop(&serial, &pkg)),
            "settings" => adb::app_settings(&serial, &pkg),
            "clear" => close_window(adb::clear_data(&serial, &pkg)),
            "uninstall" => close_window(adb::uninstall(&serial, &pkg)),
            "pull" => {
                let dir = local_dir.ok_or("Falta la carpeta de destino")?;
                let dir = std::path::Path::new(&dir);
                if !dir.is_dir() {
                    return Err("La carpeta de destino no existe".to_string());
                }
                adb::pull_apk(&serial, &pkg, dir).map(|p| format!("APK guardado en {p}"))
            }
            _ => Err("Acción desconocida".to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Modo cable: hace que el móvil alcance este PC por USB en 127.0.0.1:PORT.
#[tauri::command]
pub fn enable_cable(core: Shared) -> Result<String, String> {
    if core.is_locked() {
        return Err("Desbloquea la aplicación primero".into());
    }
    let adb = std::env::var("ANDROID_HOME")
        .map(|p| format!("{p}\\platform-tools\\adb.exe"))
        .ok()
        .filter(|p| std::path::Path::new(p).exists())
        .unwrap_or_else(|| "adb".into());
    let mut cmd = std::process::Command::new(adb);
    cmd.args(["reverse", &format!("tcp:{PORT}"), &format!("tcp:{PORT}")]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // sin ventana de consola
    }
    let out = cmd.output().map_err(|e| format!("No se pudo ejecutar adb: {e}"))?;
    if out.status.success() {
        Ok("Cable listo: el móvil conectado por USB ya puede llegar a este PC.".into())
    } else {
        Err(format!(
            "adb falló: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
