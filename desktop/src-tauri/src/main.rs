#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// La función de pruebas abre un puerto de depuración con control de la interfaz: nunca puede ir en una versión final.
#[cfg(all(feature = "webdebug", not(debug_assertions)))]
compile_error!("la feature `webdebug` es solo para pruebas y no se puede usar en una compilación release");

mod adb;
mod appwin;
mod api;
mod audio;
mod autostart;
mod boot;
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
mod tauri_ui;
mod transfer;
mod ui;
mod vault;
mod web;

use tauri::Manager;
use boot::{data_dir_arg, migrate_legacy_data};

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
            let (core, tls) = boot::init(&dir)?;
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
            tauri::async_runtime::spawn(appwin::awake_keeper(core.clone()));
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
