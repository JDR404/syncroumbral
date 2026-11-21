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

/// Registro de arranque (`startup.log`, máx. ~64 KB): si la app no llega a abrirse al iniciar Windows, aquí queda el motivo.
fn startlog(msg: &str) {
    let dir = std::env::var("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir()).join("app.syncroapp.umbral");
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("startup.log");
    if std::fs::metadata(&file).map(|m| m.len() > 64 * 1024).unwrap_or(false) {
        let _ = std::fs::remove_file(&file);
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&file) {
        let _ = writeln!(f, "{now} {msg}");
    }
}

pub fn run() {
    let background = std::env::args().any(|a| a == "--background");
    startlog(if background { "inicio en segundo plano" } else { "inicio" });
    std::panic::set_hook(Box::new(|info| startlog(&format!("PÁNICO: {info}"))));
    let dir_arg = boot::data_dir_arg();

    // `--send RUTA` (menú del Explorador): la copia en marcha recibe el archivo; si no hay, se envía cuando esta arranque.
    let send_arg: Option<String> = std::env::args().skip_while(|a| a != "--send").nth(1);

    // Una sola copia: si ya hay otra, se le pide mostrar la ventana y esta sale.
    // Si el puerto no se puede abrir y tampoco responde otra copia (al arrancar Windows otro programa puede tenerlo un momento),
    // se reintenta unos segundos y, si sigue sin poderse, se arranca igualmente sin esa protección en vez de salir en silencio.
    let listener = if dir_arg.is_none() {
        let mut found = None;
        for attempt in 0..6 {
            match TcpListener::bind(("127.0.0.1", SINGLE_PORT)) {
                Ok(l) => {
                    found = Some(l);
                    break;
                }
                Err(e) => {
                    if let Ok(mut s) = TcpStream::connect(("127.0.0.1", SINGLE_PORT)) {
                        let _ = match &send_arg {
                            Some(p) => s.write_all(format!("send\n{p}").as_bytes()),
                            None => s.write_all(b"show"),
                        };
                        startlog("ya había otra copia en marcha: se le pidió mostrar la ventana");
                        return;
                    }
                    startlog(&format!("el puerto {SINGLE_PORT} no está libre (intento {}): {e}", attempt + 1));
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
        if found.is_none() {
            startlog("se continúa sin comprobar copias duplicadas");
        }
        found
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

    // Al iniciar sesión, el sistema (claves, disco) puede tardar unos segundos: se reintenta antes de rendirse.
    let (core, tls) = {
        let mut tries = 0;
        loop {
            match boot::init(&dir) {
                Ok(x) => break x,
                Err(e) => {
                    tries += 1;
                    startlog(&format!("no se pudo iniciar el agente (intento {tries}): {e}"));
                    if tries >= 8 {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(3));
                }
            }
        }
    };
    // El PC se anuncia en la red para que el móvil lo encuentre aunque cambie la IP.
    let _mdns = discovery::advertise(agent::PORT, &core.tls_fp);

    let win = {
        let mut tries = 0;
        loop {
            match AppWindow::new() {
                Ok(w) => break w,
                Err(e) => {
                    tries += 1;
                    startlog(&format!("no se pudo crear la ventana (intento {tries}): {e}"));
                    if tries >= 8 {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(3));
                }
            }
        }
    };
    let ctx = Ctx::new(core.clone(), rt.handle().clone(), win.as_weak());
    let ui = SlintUi(ctx.clone());
    glue::wire(&win, &ctx);
    glue::set_main(win.as_weak());

    rt.spawn(crate::appwin::awake_keeper(core.clone()));
    rt.spawn(server::run(ui, core.clone(), tls));

    // Otra copia pide mostrar la ventana o enviar un archivo.
    let ctx3 = ctx.clone();
    if let Some(l) = listener {
        std::thread::spawn(move || {
            for conn in l.incoming().flatten() {
                let mut conn = conn;
                let _ = conn.set_read_timeout(Some(std::time::Duration::from_secs(1)));
                let mut msg = Vec::new();
                let _ = conn.take(32 * 1024).read_to_end(&mut msg);
                let msg = String::from_utf8_lossy(&msg).into_owned();
                if let Some(path) = msg.strip_prefix("send\n") {
                    let (ctx, path) = (ctx3.clone(), path.to_string());
                    let _ = slint::invoke_from_event_loop(move || glue::queue_send(&ctx, path));
                } else if !msg.is_empty() {
                    let _ = slint::invoke_from_event_loop(glue::show_main);
                }
            }
        });
    }

    // El icono debe vivir mientras corra el bucle de eventos.
    let _tray = make_tray(ctx.clone());

    win.window().on_close_requested(|| slint::CloseRequestResponse::HideWindow);

    // Arrastrar archivos desde el Explorador sobre la ventana: se envían a la carpeta del móvil abierta.
    {
        use slint::winit_030::{winit::event::WindowEvent, EventResult, WinitWindowAccessor};
        let c = ctx.clone();
        let wk = win.as_weak();
        win.window().on_winit_window_event(move |_, ev| {
            // Mientras se arrastra un archivo sobre la ventana, la zona de envío del panel derecho se ilumina.
            let drag = match ev {
                WindowEvent::HoveredFile(_) => Some(true),
                WindowEvent::HoveredFileCancelled | WindowEvent::DroppedFile(_) => Some(false),
                _ => None,
            };
            if let (Some(on), Some(w)) = (drag, wk.upgrade()) {
                w.set_dragging(on);
            }
            if let WindowEvent::DroppedFile(p) = ev {
                glue::queue_send(&c, p.to_string_lossy().into_owned());
            }
            EventResult::Propagate
        });
    }
    ctx.push_state();
    ctx.start_photo_backup();
    ctx.start_update_check();
    if let Some(p) = send_arg {
        glue::queue_send(&ctx, p);
    }
    // Resto de una actualización anterior.
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension("old"));
    }
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
                    c.push_notif(&ui, &serde_json::json!({"key":"k3","pkg":"com.google","app":"Mensajes","title":"Google","text":"G-482913 es tu código de verificación","when":now,"actions":[]}));
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
            // Datos inventados para ver la galería y los paneles de archivos con muchas miniaturas, sin móvil.
            if std::env::args().any(|a| a == "--test-fake") {
                let w = win.as_weak();
                slint::Timer::single_shot(std::time::Duration::from_millis(2500), move || {
                    use crate::{AlbumItem, FileItem, Photo};
                    use slint::{Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
                    let Some(w) = w.upgrade() else { return };
                    let mk = |i: u32| {
                        let mut b = SharedPixelBuffer::<Rgba8Pixel>::new(64, 64);
                        for (n, p) in b.make_mut_slice().iter_mut().enumerate() {
                            *p = Rgba8Pixel { r: ((i * 37) % 255) as u8, g: ((n as u32 % 64) * 4) as u8, b: ((n as u32 / 64) * 4) as u8, a: 255 };
                        }
                        slint::Image::from_rgba8(b)
                    };
                    let photos: Vec<Photo> = (0..90u32)
                        .map(|i| Photo { path: format!("p{i}").into(), name: format!("foto{i}").into(), video: i % 7 == 0, selected: i % 5 == 0, has_thumb: true, thumb: mk(i) })
                        .collect();
                    w.set_photos(ModelRc::new(VecModel::from(photos)));
                    let albums: Vec<AlbumItem> = ["Screenshots", "Instagram", "WhatsApp", "Camera", "PokemonGO", "Facebook", "Almacenamiento", "Download", "Syncrodomi", "Pictures"]
                        .iter()
                        .map(|n| AlbumItem { name: (*n).into(), count: 12, selected: false })
                        .collect();
                    w.set_albums(ModelRc::new(VecModel::from(albums)));
                    w.set_gallery_message("36 de 843".into());
                    let files: Vec<FileItem> = (0..60u32)
                        .map(|i| FileItem {
                            name: if i % 4 == 0 { format!("Documento_20261106112002121436905{i}.pdf") } else { format!("IMG_2026{i}.jpg") }.into(),
                            meta: "9.1 KB  01/11/2026".into(),
                            dir: i < 3,
                            selected: i % 6 == 0,
                            kind: if i % 4 == 0 { 0 } else { 1 },
                            has_thumb: i % 4 != 0 && i >= 3,
                            thumb: mk(i),
                        })
                        .collect();
                    w.set_phone_message("".into());
                    w.set_phone_path("Almacenamiento interno/Download".into());
                    w.set_phone_files(ModelRc::new(VecModel::from(files)));
                    if std::env::args().any(|a| a == "--test-grid") {
                        w.set_phone_grid(true);
                        w.set_pc_grid(true);
                    }
                    let _ = w.get_photos().row_count();
                });
            }
        }
    }
    if !background {
        let _ = win.show();
    }
    startlog("listo");
    let _ = slint::run_event_loop_until_quit();
    startlog("cierre normal");
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
