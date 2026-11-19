//! Arranque de SyncroUmbral como servicio residente: una sola instancia, bandeja del sistema y ventana bajo demanda.
//! Cerrar la ventana solo la oculta: el agente sigue recibiendo notificaciones desde la bandeja.

use crate::glue::{self, Ctx, SlintUi};
use crate::{boot, core as agent, discovery, server, AppWindow};
use slint::ComponentHandle;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

/// Puerto local que usa la copia en marcha para que una segunda orden de abrir solo muestre su ventana.
const SINGLE_PORT: u16 = 47601;

pub fn run() {
    let background = std::env::args().any(|a| a == "--background");
    let dir_arg = boot::data_dir_arg();

    // Una sola copia: si ya hay otra, se le pide mostrar la ventana y esta sale.
    let listener = if dir_arg.is_none() {
        match TcpListener::bind(("127.0.0.1", SINGLE_PORT)) {
            Ok(l) => Some(l),
            Err(_) => {
                if let Ok(mut s) = TcpStream::connect(("127.0.0.1", SINGLE_PORT)) {
                    let _ = s.write_all(b"show");
                }
                return;
            }
        }
    } else {
        None
    };

    // Misma carpeta de datos que la versión Tauri (mismos móviles vinculados y claves): se puede cambiar de una a otra.
    let dir = match dir_arg {
        Some(p) => p,
        None => {
            let base = std::env::var("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
            let d = base.join("app.syncroapp.umbral");
            boot::migrate_legacy_data(&d);
            crate::autostart::migrate_legacy_autostart();
            d
        }
    };
    let _ = std::fs::create_dir_all(&dir);
    if boot::data_dir_arg().is_none() {
        let marker = dir.join("autostart-default");
        if !marker.exists() {
            let _ = crate::autostart::set_autostart(true);
            let _ = std::fs::write(&marker, "1");
        }
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("no se pudo crear el motor asíncrono");

    let (core, tls) = match boot::init(&dir) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("No se pudo iniciar SyncroUmbral: {e}");
            return;
        }
    };
    // El PC se anuncia en la red para que el móvil lo encuentre aunque cambie la IP.
    let _mdns = discovery::advertise(agent::PORT, &core.tls_fp);

    let win = AppWindow::new().expect("no se pudo crear la ventana");
    let ctx = Ctx::new(core.clone(), rt.handle().clone(), win.as_weak());
    let ui = SlintUi(ctx.clone());
    glue::wire(&win, &ctx);
    glue::set_main(win.as_weak());

    rt.spawn(crate::appwin::awake_keeper(core.clone()));
    rt.spawn(server::run(ui, core.clone(), tls));

    // Otra copia pide mostrar la ventana.
    if let Some(l) = listener {
        std::thread::spawn(move || {
            for conn in l.incoming().flatten() {
                let mut conn = conn;
                let mut buf = [0u8; 8];
                let _ = conn.set_read_timeout(Some(std::time::Duration::from_secs(1)));
                if conn.read(&mut buf).unwrap_or(0) > 0 {
                    let _ = slint::invoke_from_event_loop(glue::show_main);
                }
            }
        });
    }

    // El icono debe vivir mientras corra el bucle de eventos.
    let _tray = make_tray(ctx.clone());

    win.window().on_close_requested(|| slint::CloseRequestResponse::HideWindow);
    ctx.push_state();
    let _timer = glue::start_timers(&ctx);
    // Solo para pruebas (feature `selftest`): desbloquear con una clave y abrir una página sin teclado ni ratón.
    #[cfg(feature = "selftest")]
    {
        let arg = |k: &str| std::env::args().find_map(|a| a.strip_prefix(k).map(str::to_string));
        if let Some(pw) = arg("--test-unlock=") {
            let page: i32 = arg("--test-page=").and_then(|p| p.parse().ok()).unwrap_or(0);
            if std::env::args().any(|a| a == "--test-notif") {
                // Notificaciones de prueba (un mensaje con respuesta y una llamada entrante) para ver tarjetas y ventana emergente.
                let (c, ctx2) = (core.clone(), ctx.clone());
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(2500));
                    let ui = SlintUi(ctx2);
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
                    c.push_notif(&ui, &serde_json::json!({"key":"k1","pkg":"com.whatsapp","app":"WhatsApp","title":"Ana","text":"¿Nos vemos a las 6?","when":now,"actions":[{"index":0,"title":"Responder","reply":true,"hint":"Responder a Ana"}]}));
                    c.push_notif(&ui, &serde_json::json!({"key":"k2","pkg":"com.whatsapp","app":"WhatsApp","title":"Carlos","text":"Llamada de voz","when":now,"call":true,"incoming":true,"actions":[]}));
                });
            }
            let w = win.as_weak();
            slint::Timer::single_shot(std::time::Duration::from_millis(800), move || {
                if let Some(w) = w.upgrade() {
                    w.invoke_unlock(pw.clone().into());
                    w.set_page(page);
                    w.invoke_page_changed(page);
                    if std::env::args().any(|a| a == "--test-qr") {
                        w.invoke_make_qr();
                    }
                }
            });
        }
    }
    if !background {
        let _ = win.show();
    }
    let _ = slint::run_event_loop_until_quit();
    ctx.edge_close_all();
}

fn icon() -> Option<tray_icon::Icon> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(include_bytes!("../../desktop/src-tauri/icons/32x32.png")))
        .read_info()
        .ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    tray_icon::Icon::from_rgba(buf, info.width, info.height).ok()
}

fn make_tray(ctx: Arc<Ctx>) -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let open = MenuItem::new("Abrir SyncroUmbral", true, None);
    let lock = MenuItem::new("Bloquear ahora", true, None);
    let quit = MenuItem::new("Salir", true, None);
    let menu = Menu::new();
    menu.append_items(&[&open, &lock, &quit]).ok()?;
    let (open_id, lock_id, quit_id) = (open.id().clone(), lock.id().clone(), quit.id().clone());

    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if e.id == open_id {
            let _ = slint::invoke_from_event_loop(glue::show_main);
        } else if e.id == lock_id {
            let ctx = ctx.clone();
            ctx.core.lock_now();
            ctx.core.notify(&SlintUi(ctx.clone()));
        } else if e.id == quit_id {
            let _ = slint::invoke_from_event_loop(|| {
                let _ = slint::quit_event_loop();
            });
        }
    }));
    TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            let _ = slint::invoke_from_event_loop(glue::show_main);
        }
    }));

    let mut b = TrayIconBuilder::new()
        .with_tooltip("SyncroUmbral")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false);
    if let Some(i) = icon() {
        b = b.with_icon(i);
    }
    b.build().ok()
}
