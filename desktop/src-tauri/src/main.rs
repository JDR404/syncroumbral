#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// La función de pruebas abre un puerto de depuración con control de la interfaz: nunca puede ir en una versión final.
#[cfg(all(feature = "webdebug", not(debug_assertions)))]
compile_error!("la feature `webdebug` es solo para pruebas y no se puede usar en una compilación release");

mod adb;
mod appwin;
mod audio;
mod commands;
mod core;
mod discovery;
mod limits;
mod places;
mod ring;
mod server;
mod share;
mod shell;
mod toast;
mod tools;
mod transfer;
mod vault;
mod web;

use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::Manager;
use tokio_rustls::rustls::{
    self,
    pki_types::{CertificateDer, PrivatePkcs8KeyDer},
};

/// Certificado autofirmado del PC: se crea una vez y se reutiliza (la huella va en el QR).
/// La clave privada se guarda cifrada con DPAPI (ver vault.rs); una `key.der` en claro de la versión anterior se migra.
fn load_or_create_cert(dir: &std::path::Path) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let cp = dir.join("cert.der");
    if let (Ok(c), Some(k)) = (std::fs::read(&cp), vault::load_tls_key(dir)) {
        return Ok((c, k));
    }
    let ck = rcgen::generate_simple_self_signed(vec!["umbral.local".to_string()])?;
    let (c, k) = (ck.cert.der().to_vec(), ck.key_pair.serialize_der());
    vault::save_tls_key(dir, &k)?;
    std::fs::write(&cp, &c)?;
    Ok((c, k))
}

/// Migración desde el nombre anterior (Umbral, identificador `app.umbral.desktop`): si la carpeta de datos nueva aún no existe y
/// la antigua sí, se mueve entera (claves, móviles vinculados, sesiones web). Las claves van cifradas con DPAPI por usuario
/// y los sellos dependen del nombre de cada archivo, no de la ruta, así que siguen siendo válidos.
fn migrate_legacy_data(new_dir: &std::path::Path) {
    let Some(parent) = new_dir.parent() else { return };
    let old = parent.join("app.umbral.desktop");
    if new_dir.exists() || !old.is_dir() {
        return;
    }
    if std::fs::rename(&old, new_dir).is_err() {
        // Otra unidad o un archivo en uso: se copia y, si salió bien, se deja la antigua intacta como respaldo.
        let _ = copy_dir(&old, new_dir);
    }
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Carpeta de datos: `--data-dir=RUTA` (perfil aparte o modo portátil) o la de la aplicación. No se lee del entorno:
/// una variable de entorno heredada no debe poder redirigir dónde SyncroUmbral guarda sus claves.
fn data_dir_arg() -> Option<std::path::PathBuf> {
    std::env::args().find_map(|a| a.strip_prefix("--data-dir=").map(std::path::PathBuf::from))
}

fn tls_config(cert: Vec<u8>, key: Vec<u8>) -> anyhow::Result<rustls::ServerConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Ok(rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(vec![CertificateDer::from(cert)], PrivatePkcs8KeyDer::from(key).into())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_data_folder_is_moved_once_and_never_overwrites_the_new_one() {
        let base = std::env::temp_dir().join(format!("syncroumbral-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (old, new) = (base.join("app.umbral.desktop"), base.join("app.syncroapp.umbral"));
        std::fs::create_dir_all(old.join("web").join("whatsapp")).unwrap();
        std::fs::write(old.join("devices.json"), "[1]").unwrap();
        std::fs::write(old.join("web").join("whatsapp").join("cookie"), "sesion").unwrap();

        migrate_legacy_data(&new);
        assert_eq!(std::fs::read_to_string(new.join("devices.json")).unwrap(), "[1]");
        assert_eq!(std::fs::read_to_string(new.join("web").join("whatsapp").join("cookie")).unwrap(), "sesion");
        assert!(!old.exists(), "la antigua se movió");

        // Si la carpeta nueva ya existe no se toca (ni se mezcla con otra antigua que reaparezca).
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("devices.json"), "[999]").unwrap();
        migrate_legacy_data(&new);
        assert_eq!(std::fs::read_to_string(new.join("devices.json")).unwrap(), "[1]");
        let _ = std::fs::remove_dir_all(&base);
    }
}

fn main() {
    tauri::Builder::default()
        // Una sola copia: si se vuelve a abrir (menú Inicio, inicio automático) se muestra la ventana de la que ya corre.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::open_main(app)))
        .setup(|app| {
            let dir = match data_dir_arg() {
                Some(p) => p,
                None => {
                    let d = app.path().app_data_dir()?;
                    migrate_legacy_data(&d);
                    shell::migrate_legacy_autostart();
                    d
                }
            };
            std::fs::create_dir_all(&dir)?;
            // Primera vez en este equipo: se activa el inicio con Windows (es un agente que debe estar siempre: sin él el PC no
            // se desbloquea con el móvil ni llegan notificaciones hasta abrirlo a mano). Se puede desactivar en Móviles y ajustes
            // y no se vuelve a tocar. Con --data-dir (pruebas, modo portátil) nunca se toca el registro.
            if data_dir_arg().is_none() {
                let marker = dir.join("autostart-default");
                if !marker.exists() {
                    let _ = shell::set_autostart(true);
                    let _ = std::fs::write(&marker, "1");
                }
            }
            tools::init(&dir);
            audio::init(&dir);
            let (cert, key) = load_or_create_cert(&dir)?;
            let fp = hex::encode(Sha256::digest(&cert));
            let tls = Arc::new(tls_config(cert, key)?);

            let mut core = core::Core::load(dir, fp);
            core.sync_notifs = true;
            let core = Arc::new(core);
            app.manage(core.clone());

            // Solo en compilaciones de prueba (feature webdebug): crea una clave de prueba y abre un servicio web integrado.
            #[cfg(feature = "webdebug")]
            if let Ok(id) = std::env::var("UMBRAL_TEST_OPEN_WEB") {
                let (h, c) = (app.handle().clone(), core.clone());
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
                    let _ = c.set_password("clave-de-prueba-1");
                    c.notify(&h);
                    for _ in 0..30 {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        if web::show_embedded(&h, &c, &id, &web::Area { x: 20.0, y: 130.0, w: 800.0, h: 400.0 }).is_ok() {
                            break;
                        }
                    }
                });
            }

            // El PC se anuncia en la red para que el móvil lo encuentre aunque cambie la IP.
            if let Some(daemon) = discovery::advertise(core::PORT, &core.tls_fp) {
                app.manage(daemon);
            }
            appwin::spawn_awake_keeper(core.clone());
            tauri::async_runtime::spawn(server::run(app.handle().clone(), core, tls));

            shell::setup_tray(app)?;
            // Con --background (inicio con Windows) solo queda el icono de la bandeja.
            if !std::env::args().any(|a| a == "--background") {
                shell::open_main(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_password,
            commands::unlock,
            commands::lock,
            commands::pairing_info,
            commands::remove_device,
            commands::enable_cable,
            commands::local_home,
            commands::local_places,
            commands::local_ls,
            commands::local_stat,
            commands::phone_ls,
            commands::phone_mkdir,
            commands::phone_delete,
            commands::phone_rename,
            commands::transfer_download,
            commands::transfer_upload,
            commands::cancel_transfers,
            commands::apps_list,
            commands::apps_icons,
            commands::adb_status,
            commands::app_action,
            commands::web_list,
            commands::web_show,
            commands::web_hide,
            commands::web_forget,
            commands::app_open_pc,
            commands::app_windows,
            commands::notifs_list,
            commands::toast_items,
            commands::toast_hide,
            commands::toast_resize,
            commands::notif_reply,
            commands::notif_action,
            commands::call_answer,
            commands::call_end,
            commands::call_perms,
            commands::get_mute_on_call,
            commands::set_mute_on_call,
            commands::get_advanced,
            commands::set_advanced,
            commands::notif_dismiss,
            commands::notif_clear_all,
            commands::notif_remove_local,
            commands::notif_mute,
            commands::notif_unmute,
            commands::notif_muted,
            commands::show_received,
            commands::phone_clip_set,
            commands::tools_status,
            commands::tools_install,
            commands::adb_pairing_services,
            commands::adb_pair,
            commands::autostart_get,
            commands::autostart_set,
        ])
        .build(tauri::generate_context!())
        .expect("error al iniciar SyncroUmbral")
        .run(|_app, event| {
            // Cerrar la última ventana no cierra SyncroUmbral: sigue en la bandeja recibiendo notificaciones.
            // Solo sale con "Salir" (código de salida explícito).
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
