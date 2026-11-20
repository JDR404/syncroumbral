//! Une el agente (core, servidor, transferencias) con la interfaz Slint.
//! Todo lo que toca la interfaz ocurre en su hilo; el agente corre en el motor asíncrono.

use crate::core::Core;
use crate::transfer::Item;
use crate::ui::Ui;
use crate::{AlbumItem, AppItem, AppWindow, CallItem, ContactItem, DeviceItem, FileItem, MsgItem, Notif, NotifAction, Photo, ServiceItem, ThreadItem, ToastWindow};
use serde::Serialize;
use serde_json::{json, Value};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ------------------------------------------------------------------------------------------------ estado

#[derive(Clone)]
struct Row {
    name: String,
    dir: bool,
    size: u64,
    mtime: u64,
}

#[derive(Default)]
struct PaneState {
    filter: String,
    path: String,
    parent: Option<String>,
    rows: Vec<Row>,
    sel: Vec<bool>,
    /// Vistas previas de la carpeta abierta, por nombre de archivo.
    thumbs: HashMap<String, slint::SharedPixelBuffer<slint::Rgba8Pixel>>,
}

#[derive(Default)]
struct Files {
    pc: PaneState,
    phone: PaneState,
}

#[derive(Clone)]
struct AppEntry {
    pkg: String,
    label: String,
    system: bool,
}

pub struct Ctx {
    albums: Mutex<Vec<AlbumMeta>>,
    album: Mutex<String>,
    app_icons: Mutex<HashMap<String, slint::SharedPixelBuffer<slint::Rgba8Pixel>>>,
    apps_q: Mutex<String>,
    show_system: std::sync::atomic::AtomicBool,
    icons_busy: std::sync::atomic::AtomicBool,
    threads: Mutex<Vec<(String, String)>>,
    sms_open: Mutex<Option<String>>,
    sms_log: Mutex<Vec<(String, bool, String)>>,
    settings: Mutex<Settings>,
    last_present: std::sync::atomic::AtomicBool,
    gallery: Mutex<Gallery>,
    pub core: Arc<Core>,
    rt: tokio::runtime::Handle,
    win: slint::Weak<AppWindow>,
    files: Mutex<Files>,
    apps: Mutex<Vec<AppEntry>>,
    status_gen: AtomicU64,
    edge: Mutex<HashMap<String, std::process::Child>>,
    toast_gen: AtomicU64,
    /// Última transferencia: 1 = envío al móvil, 2 = descarga al PC (para refrescar solo el panel que cambia).
    xfer_kind: AtomicU64,
}

/// Interfaz Slint como implementación de `Ui` para el agente.
#[derive(Clone)]
pub struct SlintUi(pub Arc<Ctx>);

thread_local! {
    static MAIN: RefCell<Option<slint::Weak<AppWindow>>> = const { RefCell::new(None) };
    static TOAST: RefCell<Option<ToastWindow>> = const { RefCell::new(None) };
}

pub fn set_main(w: slint::Weak<AppWindow>) {
    MAIN.with(|m| *m.borrow_mut() = Some(w));
}

/// Muestra y trae al frente la ventana principal (desde la bandeja o una segunda copia).
pub fn show_main() {
    let w = MAIN.with(|m| m.borrow().as_ref().and_then(|w| w.upgrade()));
    if let Some(w) = w {
        let _ = w.show();
        use slint::winit_030::WinitWindowAccessor;
        w.window().with_winit_window(|ww| {
            ww.set_minimized(false);
            ww.focus_window();
        });
    }
}

impl Ui for SlintUi {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        let v = serde_json::to_value(&payload).unwrap_or(Value::Null);
        let ctx = &self.0;
        match event {
            "state" => ctx.push_state(),
            "notif" | "notif_gone" | "notif_sync" | "notif_clear" => {
                if event == "notif" && v["n"]["clip"] == true {
                    ctx.auto_clip(v["n"]["text"].as_str().unwrap_or("").to_string());
                }
                ctx.push_notifs();
                ctx.refresh_toast();
            }
            "transfer" => ctx.on_transfer(&v),
            "mode" => {
                let on = v["advanced"].as_bool().unwrap_or(false);
                ctx.ui(move |w| w.set_advanced(on));
            }
            _ => {}
        }
    }
    fn toast_show(&self) {
        self.0.refresh_toast();
    }
    fn toast_hide(&self) {
        self.0.hide_toast();
    }
    fn close_web(&self) {
        self.0.edge_close_all();
    }
}


// ------------------------------------------------------------------------------------------------ códigos 2FA

/// Código de verificación (4 a 8 dígitos, o 3+3 separados) si el texto habla de códigos; vacío si no.
pub fn extract_code(s: &str) -> String {
    let l = s.to_lowercase();
    const KEYS: [&str; 12] = ["código", "codigo", "code", "verific", "otp", "clave", "pin", "contraseña", "password", "one-time", "único", "seguridad"];
    if !KEYS.iter().any(|k| l.contains(k)) {
        return String::new();
    }
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if !c[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let st = i;
        while i < c.len() && c[i].is_ascii_digit() {
            i += 1;
        }
        let run = i - st;
        if (4..=8).contains(&run) {
            return c[st..i].iter().collect();
        }
        if run == 3 && i + 1 < c.len() && (c[i] == '-' || c[i] == ' ') && c[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < c.len() && c[j].is_ascii_digit() {
                j += 1;
            }
            if j - (i + 1) == 3 {
                return c[st..i].iter().chain(c[i + 1..j].iter()).collect();
            }
        }
    }
    String::new()
}

fn decode_thumb(b64: &str) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    let bytes = base64_decode(b64)?;
    let mut d = jpeg_decoder::Decoder::new(&bytes[..]);
    let px = d.decode().ok()?;
    let info = d.info()?;
    let (w, h) = (info.width as u32, info.height as u32);
    let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
    let out = buf.make_mut_slice();
    match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            for (o, p) in out.iter_mut().zip(px.chunks_exact(3)) {
                *o = slint::Rgba8Pixel { r: p[0], g: p[1], b: p[2], a: 255 };
            }
        }
        jpeg_decoder::PixelFormat::L8 => {
            for (o, p) in out.iter_mut().zip(px.iter()) {
                *o = slint::Rgba8Pixel { r: *p, g: *p, b: *p, a: 255 };
            }
        }
        _ => return None,
    }
    Some(buf)
}

/// 1 si el nombre es de una imagen, 2 si es de un vídeo, 0 si es otra cosa.
fn media_kind(name: &str) -> i32 {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "heic" | "heif" => 1,
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "3gp" | "m4v" => 2,
        _ => 0,
    }
}

/// Vista previa de una imagen del PC (jpg o png) de hasta 160 px de lado. None si no se puede leer.
fn local_thumb(path: &str) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    const MAX: u32 = 160;
    let ext = path.rsplit('.').next()?.to_lowercase();
    let len = std::fs::metadata(path).ok()?.len();
    if len > 40_000_000 || (ext == "png" && len > 12_000_000) {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let (w, h, px): (u32, u32, Vec<slint::Rgba8Pixel>) = match ext.as_str() {
        "jpg" | "jpeg" => {
            let mut d = jpeg_decoder::Decoder::new(&bytes[..]);
            // Descodificar ya reducido: una foto de 12 MP se abre en una fracción del tiempo.
            let (sw, sh) = d.scale(MAX as u16, MAX as u16).ok()?;
            let raw = d.decode().ok()?;
            let fmt = d.info()?.pixel_format;
            let px: Vec<slint::Rgba8Pixel> = match fmt {
                jpeg_decoder::PixelFormat::RGB24 => raw.chunks_exact(3).map(|p| slint::Rgba8Pixel { r: p[0], g: p[1], b: p[2], a: 255 }).collect(),
                jpeg_decoder::PixelFormat::L8 => raw.iter().map(|&p| slint::Rgba8Pixel { r: p, g: p, b: p, a: 255 }).collect(),
                _ => return None,
            };
            (sw as u32, sh as u32, px)
        }
        "png" => {
            let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
            dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
            let mut r = dec.read_info().ok()?;
            let mut buf = vec![0; r.output_buffer_size()];
            let info = r.next_frame(&mut buf).ok()?;
            let px: Vec<slint::Rgba8Pixel> = match info.color_type {
                png::ColorType::Rgba => buf.chunks_exact(4).map(|p| slint::Rgba8Pixel { r: p[0], g: p[1], b: p[2], a: p[3] }).collect(),
                png::ColorType::Rgb => buf.chunks_exact(3).map(|p| slint::Rgba8Pixel { r: p[0], g: p[1], b: p[2], a: 255 }).collect(),
                png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).map(|p| slint::Rgba8Pixel { r: p[0], g: p[0], b: p[0], a: p[1] }).collect(),
                png::ColorType::Grayscale => buf.iter().map(|&p| slint::Rgba8Pixel { r: p, g: p, b: p, a: 255 }).collect(),
                _ => return None,
            };
            (info.width, info.height, px)
        }
        _ => return None,
    };
    if w == 0 || h == 0 || px.len() < (w * h) as usize {
        return None;
    }
    // Reducción por vecino más cercano hasta MAX px de lado (basta para una miniatura).
    let k = (w.max(h) as f32 / MAX as f32).max(1.0);
    let (tw, th) = (((w as f32 / k) as u32).max(1), ((h as f32 / k) as u32).max(1));
    let mut out = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(tw, th);
    let dst = out.make_mut_slice();
    for y in 0..th {
        let sy = ((y as f32 * k) as u32).min(h - 1);
        for x in 0..tw {
            let sx = ((x as f32 * k) as u32).min(w - 1);
            dst[(y * tw + x) as usize] = px[(sy * w + sx) as usize];
        }
    }
    Some(out)
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s).ok()
}

#[derive(Clone)]
struct PhotoMeta {
    path: String,
    name: String,
    video: bool,
    size: u64,
    selected: bool,
}

#[derive(Default)]
struct Gallery {
    items: Vec<PhotoMeta>,
    thumbs: HashMap<String, slint::SharedPixelBuffer<slint::Rgba8Pixel>>,
}

// ------------------------------------------------------------------------------------------------ utilidades

fn fmt_size(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let (mut v, mut i) = (n as f64, 0);
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}

/// dd/mm/aaaa a partir de milisegundos desde 1970 (calendario civil, sin dependencias).
fn fmt_date(ms: u64) -> String {
    if ms == 0 {
        return String::new();
    }
    let z = (ms / 86_400_000) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{d:02}/{m:02}/{y}")
}

fn ago(ms: u64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    let s = now.saturating_sub(ms) / 1000;
    match s {
        0..=59 => "ahora".into(),
        60..=3599 => format!("hace {} min", s / 60),
        3600..=86_399 => format!("hace {} h", s / 3600),
        _ => format!("hace {} d", s / 86_400),
    }
}

fn notif_of(v: &Value) -> Notif {
    let reply = v["actions"].as_array().and_then(|a| a.iter().find(|x| x["reply"] == true));
    Notif {
        key: v["key"].as_str().unwrap_or("").into(),
        app: one_line(v["app"].as_str().or(v["pkg"].as_str()).unwrap_or(""), 40).into(),
        title: one_line(v["title"].as_str().unwrap_or(""), 90).into(),
        text: one_line(v["text"].as_str().unwrap_or(""), 260).into(),
        time: ago(v["when"].as_u64().unwrap_or(0)).into(),
        call: v["call"] == true,
        incoming: v["incoming"] == true,
        reply_index: reply.and_then(|r| r["index"].as_i64()).map_or(-1, |i| i as i32),
        reply_hint: reply.and_then(|r| r["hint"].as_str()).unwrap_or("").into(),
        code: extract_code(&format!("{} {}", v["title"].as_str().unwrap_or(""), v["text"].as_str().unwrap_or(""))).into(),
        actions: model(
            v["actions"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|x| x["reply"] != true && x["title"].as_str().is_some_and(|t| !t.is_empty()))
                        .take(4)
                        .map(|x| NotifAction { index: x["index"].as_i64().unwrap_or(0) as i32, title: x["title"].as_str().unwrap_or("").into() })
                        .collect()
                })
                .unwrap_or_default(),
        ),
    }
}

/// Igual que `notif_of`, pero sin título, texto ni código si el usuario pidió ocultar el contenido.
fn notif_for_display(v: &Value, hide: bool) -> Notif {
    let mut n = notif_of(v);
    if hide && !n.call {
        n.title = n.app.clone();
        n.text = "Contenido oculto".into();
        n.code = "".into();
    }
    n
}

fn join_pc(base: &str, name: &str) -> String {
    if base.is_empty() {
        format!("{name}\\")
    } else if base.ends_with('\\') {
        format!("{base}{name}")
    } else {
        format!("{base}\\{name}")
    }
}

fn join_phone(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else {
        format!("{base}/{name}")
    }
}

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

fn qr_image(payload: &str) -> Option<slint::Image> {
    let code = qrcode::QrCode::new(payload.as_bytes()).ok()?;
    let (n, scale, quiet) = (code.width(), 6usize, 2usize);
    let size = (n + quiet * 2) * scale;
    let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(size as u32, size as u32);
    let px = buf.make_mut_slice();
    px.fill(slint::Rgba8Pixel { r: 255, g: 255, b: 255, a: 255 });
    for (i, c) in code.to_colors().into_iter().enumerate() {
        if c == qrcode::Color::Dark {
            let (mx, my) = ((i % n + quiet) * scale, (i / n + quiet) * scale);
            for y in my..my + scale {
                for x in mx..mx + scale {
                    px[y * size + x] = slint::Rgba8Pixel { r: 0, g: 0, b: 0, a: 255 };
                }
            }
        }
    }
    Some(slint::Image::from_rgba8(buf))
}

// ------------------------------------------------------------------------------------------------ Ctx

impl Ctx {
    pub fn new(core: Arc<Core>, rt: tokio::runtime::Handle, win: slint::Weak<AppWindow>) -> Arc<Self> {
        Arc::new(Ctx {
            albums: Mutex::new(Vec::new()),
            album: Mutex::new(String::new()),
            app_icons: Mutex::new(HashMap::new()),
            apps_q: Mutex::new(String::new()),
            show_system: std::sync::atomic::AtomicBool::new(false),
            icons_busy: std::sync::atomic::AtomicBool::new(false),
            threads: Mutex::new(Vec::new()),
            sms_open: Mutex::new(None),
            sms_log: Mutex::new(Vec::new()),
            settings: Mutex::new(load_settings(&core.dir)),
            last_present: std::sync::atomic::AtomicBool::new(false),
            gallery: Mutex::new(Gallery::default()),
            core,
            rt,
            win,
            files: Mutex::new(Files::default()),
            apps: Mutex::new(Vec::new()),
            status_gen: AtomicU64::new(0),
            edge: Mutex::new(HashMap::new()),
            toast_gen: AtomicU64::new(0),
            xfer_kind: AtomicU64::new(0),
        })
    }

    fn sink(self: &Arc<Self>) -> SlintUi {
        SlintUi(self.clone())
    }

    /// Ejecuta algo con la ventana principal, en el hilo de la interfaz.
    fn ui(&self, f: impl FnOnce(&AppWindow) + Send + 'static) {
        let _ = self.win.upgrade_in_event_loop(move |w| f(&w));
    }

    /// Mensaje breve en la barra inferior.
    pub fn status(self: &Arc<Self>, msg: impl Into<String>) {
        let msg: String = msg.into();
        let gen = self.status_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let me = self.clone();
        self.ui(move |w| {
            w.set_status(msg.into());
            let weak = w.as_weak();
            slint::Timer::single_shot(Duration::from_secs(6), move || {
                if me.status_gen.load(Ordering::SeqCst) == gen {
                    if let Some(w) = weak.upgrade() {
                        w.set_status("".into());
                    }
                }
            });
        });
    }

    // ---------------------------------------------------------------- estado general

    pub fn push_state(self: &Arc<Self>) {
        let s = self.core.snapshot();
        self.on_presence(s.present);
        let (advanced, mute) = (self.core.advanced(), self.core.mute_on_call());
        let me = self.clone();
        self.ui(move |w| {
            let was_locked = w.get_locked();
            w.set_has_password(s.has_password);
            w.set_present(s.present);
            w.set_device(s.device.clone().unwrap_or_default().into());
            let sub = match s.battery {
                Some(b) => format!("Conectado · {b}%{}", if s.charging { " · cargando" } else { "" }),
                None => "Conectado".into(),
            };
            w.set_device_sub(sub.into());
            w.set_warn(s.warn.unwrap_or("").into());
            w.set_battery(s.battery.map_or(-1, |b| b as i32));
            w.set_charging(s.charging);
            w.set_lock_hint(
                if s.lockout_secs > 0 {
                    format!("Demasiados intentos. Espera {} s.", s.lockout_secs)
                } else {
                    "Esperando a tu móvil. Se abrirá solo cuando esté en la red.".into()
                }
                .into(),
            );
            w.set_devices(model(s.devices.iter().map(|d| DeviceItem { id: d.id.clone().into(), name: d.name.clone().into() }).collect()));
            w.set_advanced(advanced);
            w.set_mute_on_call(mute);
            w.set_locked(s.locked);
            if s.locked && !was_locked {
                me.on_locked(w);
            } else if !s.locked && was_locked {
                me.on_unlocked(w);
            }
        });
    }

    fn on_locked(self: &Arc<Self>, w: &AppWindow) {
        *self.files.lock().unwrap() = Files::default();
        self.apps.lock().unwrap().clear();
        *self.gallery.lock().unwrap() = Gallery::default();
        self.albums.lock().unwrap().clear();
        self.album.lock().unwrap().clear();
        self.app_icons.lock().unwrap().clear();
        w.set_albums(model(vec![]));
        w.set_album_selected(-1);
        self.threads.lock().unwrap().clear();
        self.sms_log.lock().unwrap().clear();
        *self.sms_open.lock().unwrap() = None;
        w.set_threads(model(vec![]));
        w.set_messages(model(vec![]));
        w.set_sms_title("".into());
        w.set_sms_selected(-1);
        w.set_photos(model(vec![]));
        w.set_contacts(model(vec![]));
        w.set_call_log(model(vec![]));
        w.set_ringing(false);
        w.set_notifs(model(vec![]));
        w.set_pc_files(model(vec![]));
        w.set_phone_files(model(vec![]));
        w.set_apps(model(vec![]));
        w.set_qr(slint::Image::default());
        w.set_qr_text("".into());
        w.set_transfer_text("".into());
        w.set_status("".into());
        w.set_page(0);
        self.hide_toast();
        self.edge_close_all();
    }

    fn on_unlocked(self: &Arc<Self>, w: &AppWindow) {
        w.set_autostart(crate::autostart::autostart_enabled());
        w.set_services(model(self.services()));
        self.push_notifs();
        self.load_pc(None);
        self.load_phone(None);
        self.refresh_tools();
    }

    // ---------------------------------------------------------------- archivos

    fn apply_files(self: &Arc<Self>, phone: bool) {
        let (items, path, footer) = {
            let f = self.files.lock().unwrap();
            let p = if phone { &f.phone } else { &f.pc };
            let vis = p.visible();
            // Los datos viajan sin imágenes de Slint (no se pueden enviar entre hilos); se convierten ya en el hilo de la interfaz.
            let items: Vec<(String, String, bool, bool, i32, Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>)> = vis
                .iter()
                .map(|&k| {
                    let r = &p.rows[k];
                    (
                        r.name.clone(),
                        if r.dir { String::new() } else { format!("{}  {}", fmt_size(r.size), fmt_date(r.mtime)) },
                        r.dir,
                        p.sel[k],
                        if r.dir { 0 } else { media_kind(&r.name) },
                        p.thumbs.get(&r.name).cloned(),
                    )
                })
                .collect();
            let count = p.sel.iter().filter(|s| **s).count();
            let footer = if count > 0 {
                format!("{count} marcado(s)")
            } else if vis.len() != p.rows.len() {
                format!("{} de {} elementos", vis.len(), p.rows.len())
            } else {
                format!("{} elementos", p.rows.len())
            };
            (items, p.path.clone(), footer)
        };
        let shown = if phone {
            if path.is_empty() { "Almacenamiento interno".to_string() } else { format!("Almacenamiento interno/{path}") }
        } else if path.is_empty() {
            "Unidades".to_string()
        } else {
            path
        };
        self.ui(move |w| {
            let items: Vec<FileItem> = items
                .into_iter()
                .map(|(name, meta, dir, selected, kind, thumb)| FileItem {
                    name: name.into(),
                    meta: meta.into(),
                    dir,
                    selected,
                    kind,
                    has_thumb: thumb.is_some(),
                    thumb: thumb.map(slint::Image::from_rgba8).unwrap_or_default(),
                })
                .collect();
            if phone {
                w.set_phone_files(model(items));
                w.set_phone_path(shown.into());
                w.set_phone_footer(footer.into());
                w.set_phone_message("".into());
            } else {
                w.set_pc_files(model(items));
                w.set_pc_path(shown.into());
                w.set_pc_footer(footer.into());
                w.set_pc_message("".into());
            }
        });
    }

    fn load_pc(self: &Arc<Self>, path: Option<String>) {
        let me = self.clone();
        self.rt.spawn_blocking(move || {
            let path = path.unwrap_or_else(|| {
                let cur = me.files.lock().unwrap().pc.path.clone();
                if cur.is_empty() { std::env::var("USERPROFILE").unwrap_or_default() } else { cur }
            });
            match crate::api::local_ls(&me.core, path) {
                Ok(l) => {
                    {
                        let mut f = me.files.lock().unwrap();
                        f.pc.filter.clear();
                        f.pc.rows = l.items.iter().filter(|e| !e.name.starts_with('.')).map(|e| Row { name: e.name.clone(), dir: e.dir, size: e.size, mtime: e.mtime }).collect();
                        f.pc.sel = vec![false; f.pc.rows.len()];
                        f.pc.path = l.path;
                        f.pc.parent = l.parent;
                        f.pc.thumbs.clear();
                    }
                    me.ui(|w| w.set_pc_filter("".into()));
                    me.apply_files(false);
                    me.load_thumbs(false);
                }
                Err(e) => me.ui(move |w| w.set_pc_message(e.into())),
            }
        });
    }

    fn load_phone(self: &Arc<Self>, path: Option<String>) {
        let me = self.clone();
        self.rt.spawn(async move {
            let path = path.unwrap_or_else(|| me.files.lock().unwrap().phone.path.clone());
            match me.core.call("ls", json!({ "path": path })).await {
                Ok(r) => {
                    let mut rows: Vec<Row> = r["items"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|x| Row {
                                    name: x["name"].as_str().unwrap_or("").to_string(),
                                    dir: x["dir"] == true,
                                    size: x["size"].as_u64().unwrap_or(0),
                                    mtime: x["mtime"].as_u64().unwrap_or(0),
                                })
                                .filter(|x| !x.name.starts_with('.'))
                                .collect()
                        })
                        .unwrap_or_default();
                    rows.sort_by(|a, b| b.dir.cmp(&a.dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
                    let p = r["path"].as_str().unwrap_or("").to_string();
                    let free = r["free"].as_u64();
                    {
                        let mut f = me.files.lock().unwrap();
                        f.phone.filter.clear();
                        f.phone.sel = vec![false; rows.len()];
                        f.phone.rows = rows;
                        f.phone.parent = if p.is_empty() { None } else { Some(p.rfind('/').map_or(String::new(), |i| p[..i].to_string())) };
                        f.phone.path = p;
                        f.phone.thumbs.clear();
                    }
                    me.ui(|w| w.set_phone_filter("".into()));
                    me.apply_files(true);
                    me.load_thumbs(true);
                    if let Some(fr) = free {
                        me.ui(move |w| {
                            let cur = w.get_phone_footer().to_string();
                            w.set_phone_footer(format!("{cur} · {} libres", fmt_size(fr)).into());
                        });
                    }
                }
                Err(e) => {
                    let e = if me.core.snapshot().present { e } else { "El móvil no está conectado. Los archivos del móvil aparecen cuando se conecte.".to_string() };
                    me.ui(move |w| {
                        w.set_phone_files(model(vec![]));
                        w.set_phone_message(e.into());
                    });
                }
            }
        });
    }

    /// Pide las vistas previas de las imágenes y vídeos de la carpeta abierta; van apareciendo poco a poco.
    fn load_thumbs(self: &Arc<Self>, phone: bool) {
        let (path, names): (String, Vec<String>) = {
            let f = self.files.lock().unwrap();
            let p = if phone { &f.phone } else { &f.pc };
            (
                p.path.clone(),
                p.rows.iter().filter(|r| !r.dir && media_kind(&r.name) > 0 && (phone || r.size <= 40_000_000)).take(150).map(|r| r.name.clone()).collect(),
            )
        };
        if names.is_empty() {
            return;
        }
        let me = self.clone();
        if phone {
            self.rt.spawn(async move {
                for chunk in names.chunks(12) {
                    let paths: Vec<String> = chunk.iter().map(|n| join_phone(&path, n)).collect();
                    if let Ok(t) = me.core.call("media_thumb", json!({ "paths": paths })).await {
                        let got: Vec<(String, slint::SharedPixelBuffer<slint::Rgba8Pixel>)> = chunk
                            .iter()
                            .zip(&paths)
                            .filter_map(|(n, p)| decode_thumb(t["thumbs"][p.as_str()].as_str()?).map(|b| (n.clone(), b)))
                            .collect();
                        if !me.add_thumbs(true, &path, got) {
                            return;
                        }
                    }
                }
            });
        } else {
            self.rt.spawn_blocking(move || {
                let mut batch = Vec::new();
                for n in names {
                    if let Some(b) = local_thumb(&join_pc(&path, &n)) {
                        batch.push((n, b));
                    }
                    if batch.len() >= 6 && !me.add_thumbs(false, &path, std::mem::take(&mut batch)) {
                        return;
                    }
                }
                if !batch.is_empty() {
                    me.add_thumbs(false, &path, batch);
                }
            });
        }
    }

    /// Guarda vistas previas recién llegadas. Devuelve false si el usuario ya cambió de carpeta (se abandona el trabajo).
    fn add_thumbs(self: &Arc<Self>, phone: bool, path: &str, got: Vec<(String, slint::SharedPixelBuffer<slint::Rgba8Pixel>)>) -> bool {
        {
            let mut f = self.files.lock().unwrap();
            let p = if phone { &mut f.phone } else { &mut f.pc };
            if p.path != path {
                return false;
            }
            p.thumbs.extend(got);
        }
        self.apply_files(phone);
        true
    }

    fn selected(self: &Arc<Self>, phone: bool) -> Vec<Item> {
        let f = self.files.lock().unwrap();
        let p = if phone { &f.phone } else { &f.pc };
        p.rows
            .iter()
            .zip(&p.sel)
            .filter(|(_, s)| **s)
            .map(|(r, _)| Item {
                path: if phone { join_phone(&p.path, &r.name) } else { join_pc(&p.path, &r.name) },
                name: r.name.clone(),
                dir: r.dir,
                size: r.size,
            })
            .collect()
    }

    fn on_transfer(self: &Arc<Self>, v: &Value) {
        let (label, state, msg) = (v["label"].as_str().unwrap_or("").to_string(), v["state"].as_str().unwrap_or(""), v["msg"].as_str().unwrap_or("").to_string());
        let (done, total) = (v["done"].as_u64().unwrap_or(0), v["total"].as_u64().unwrap_or(0));
        let frac = if total > 0 { (done as f32 / total as f32).clamp(0.0, 1.0) } else { 0.0 };
        let (active, text) = match state {
            "planning" => (true, format!("{label} · preparando")),
            "running" => (true, format!("{label} · {} de {}", fmt_size(done), fmt_size(total))),
            "done" => (false, format!("Terminado: {label}")),
            "cancelled" => (false, "Transferencia cancelada".to_string()),
            _ => (false, if msg.is_empty() { "Transferencia fallida".to_string() } else { msg }),
        };
        let refresh = state == "done";
        self.ui(move |w| {
            w.set_transfer_active(active);
            w.set_transfer_frac(frac);
            w.set_transfer_text(text.into());
        });
        if refresh {
            match self.xfer_kind.load(Ordering::SeqCst) {
                1 => self.load_phone(None),
                2 => self.load_pc(None),
                _ => {
                    self.load_pc(None);
                    self.load_phone(None);
                }
            }
        }
    }

    // ---------------------------------------------------------------- notificaciones

    pub fn push_notifs(self: &Arc<Self>) {
        let st = self.core.notifs_state();
        let items: Vec<Value> = st["items"].as_array().cloned().unwrap_or_default();
        let hide = self.hide_content();
        let hint = if st["granted"] == false { "El móvil no ha dado permiso para leer notificaciones." } else { "" };
        self.ui(move |w| {
            w.set_notifs(model(items.iter().map(|v| notif_for_display(v, hide)).collect()));
            w.set_notif_hint(hint.into());
        });
    }

    pub fn refresh_toast(self: &Arc<Self>) {
        let me = self.clone();
        let _ = slint::invoke_from_event_loop(move || me.toast_on_ui());
    }

    pub fn hide_toast(self: &Arc<Self>) {
        let _ = slint::invoke_from_event_loop(|| {
            TOAST.with(|t| {
                if let Some(w) = t.borrow_mut().take() {
                    let _ = w.hide();
                }
            })
        });
    }

    /// Ventana emergente: abajo a la derecha del área útil, sin robar el foco, y se cierra sola a los 12 s (no con una llamada).
    fn toast_on_ui(self: &Arc<Self>) {
        let hide = self.hide_content();
        let items: Vec<Notif> = self
            .core
            .popup_items()
            .iter()
            .filter(|v| v["call"] == true || !self.muted_by_words(v["title"].as_str().unwrap_or(""), v["text"].as_str().unwrap_or("")))
            .take(4)
            .map(|v| notif_for_display(v, hide))
            .collect();
        if items.is_empty() {
            TOAST.with(|t| {
                if let Some(w) = t.borrow_mut().take() {
                    let _ = w.hide();
                }
            });
            return;
        }
        let mut height = 8.0f32;
        for n in &items {
            height += 64.0
                + if n.reply_index >= 0 { 46.0 } else { 0.0 }
                + if n.call { 48.0 } else { 0.0 }
                + if n.actions.row_count() > 0 && !n.call { 42.0 } else { 0.0 }
                + 8.0;
        }
        let has_call = items.iter().any(|n| n.call);
        let me = self.clone();
        TOAST.with(|t| {
            let mut t = t.borrow_mut();
            let fresh = t.is_none();
            let w = t.get_or_insert_with(|| {
                let w = ToastWindow::new().expect("ventana emergente");
                let c = me.clone();
                w.on_dismiss(move |_| c.core.clear_popups(&c.sink()));
                let c = me.clone();
                w.on_reply(move |key, idx, text| c.reply(key.to_string(), idx, text.to_string()));
                let c = me.clone();
                w.on_answer(move |key| c.call_op("call_answer", key.to_string(), true));
                let c = me.clone();
                w.on_end(move |key| c.call_op("call_end", key.to_string(), false));
                let c = me.clone();
                w.on_action(move |key, i| c.notif_action(key.to_string(), i));
                let c = me.clone();
                w.on_copy_code(move |code| c.copy_code(code.to_string()));
                w
            });
            w.set_items(model(items));
            w.window().set_size(slint::LogicalSize::new(400.0, height));
            place_bottom_right(w, 400.0, height);
            if fresh {
                let _ = w.show();
                use slint::winit_030::{winit::platform::windows::WindowExtWindows, WinitWindowAccessor};
                w.window().with_winit_window(|ww| ww.set_skip_taskbar(true));
                place_bottom_right(w, 400.0, height);
            }
        });
        let gen = self.toast_gen.fetch_add(1, Ordering::SeqCst) + 1;
        if !has_call {
            let me = self.clone();
            slint::Timer::single_shot(Duration::from_secs(12), move || {
                if me.toast_gen.load(Ordering::SeqCst) == gen {
                    me.core.clear_popups(&me.sink());
                }
            });
        }
    }

    fn reply(self: &Arc<Self>, key: String, idx: i32, text: String) {
        if text.trim().is_empty() {
            self.status("Escribe una respuesta");
            return;
        }
        let me = self.clone();
        self.rt.spawn(async move {
            match me.core.call("notif_reply", json!({ "key": key, "index": idx, "text": text })).await {
                Ok(_) => me.status("Respuesta enviada"),
                Err(e) => me.status(e),
            }
        });
    }

    /// Contestar o colgar una llamada. Al contestar se abre la ventana principal.
    fn call_op(self: &Arc<Self>, op: &'static str, key: String, open: bool) {
        let me = self.clone();
        self.rt.spawn(async move {
            match me.core.call(op, json!({ "key": key })).await {
                Ok(_) => {
                    me.ui(|w| w.set_call_status("Hecho".into()));
                    if open {
                        let _ = slint::invoke_from_event_loop(show_main);
                    }
                }
                Err(e) => me.ui(move |w| w.set_call_status(e.into())),
            }
        });
    }

    fn notif_action(self: &Arc<Self>, key: String, idx: i32) {
        let me = self.clone();
        self.rt.spawn(async move {
            if let Err(e) = me.core.call("notif_action", json!({ "key": key, "index": idx })).await {
                me.status(e);
            }
        });
    }

    // ---------------------------------------------------------------- aplicaciones

    fn load_apps(self: &Arc<Self>) {
        let me = self.clone();
        self.ui(|w| w.set_apps_message("Cargando aplicaciones del móvil".into()));
        self.rt.spawn(async move {
            match me.core.call("apps", json!({})).await {
                Ok(r) => {
                    let mut list: Vec<AppEntry> = r["apps"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|x| AppEntry {
                                    pkg: x["pkg"].as_str().unwrap_or("").into(),
                                    label: x["label"].as_str().unwrap_or("").into(),
                                    system: x["system"] == true,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    list.sort_by(|a, b| a.system.cmp(&b.system).then(a.label.to_lowercase().cmp(&b.label.to_lowercase())));
                    *me.apps.lock().unwrap() = list;
                    me.ui(|w| w.set_apps_message("".into()));
                    me.apply_apps(false);
                    me.fetch_missing_icons();
                }
                Err(e) => {
                    let msg = if me.core.snapshot().present { e } else { "El móvil no está conectado.".to_string() };
                    me.ui(move |w| w.set_apps_message(msg.into()));
                }
            }
        });
    }

    /// Aplica la lista (sin las de sistema salvo que se pida) y quita la selección.
    fn apply_apps(self: &Arc<Self>, _keep: bool) {
        let me = self.clone();
        self.ui(move |w| {
            let show_sys = w.get_show_system();
            me.show_system.store(show_sys, Ordering::SeqCst);
            let q = me.apps_q.lock().unwrap().to_lowercase();
            let running = crate::appwin::running(&me.core.app_windows);
            let icons = me.app_icons.lock().unwrap().clone();
            let items: Vec<AppItem> = me
                .apps
                .lock()
                .unwrap()
                .iter()
                .filter(|a| show_sys || !a.system)
                .filter(|a| q.is_empty() || a.label.to_lowercase().contains(&q) || a.pkg.to_lowercase().contains(&q))
                .map(|a| AppItem {
                    pkg: a.pkg.clone().into(),
                    label: a.label.clone().into(),
                    system: a.system,
                    open: running.contains(&a.pkg),
                    has_icon: icons.contains_key(&a.pkg),
                    icon: icons.get(&a.pkg).cloned().map(slint::Image::from_rgba8).unwrap_or_default(),
                })
                .collect();
            w.set_apps(model(items));
            w.set_app_selected(-1);
        });
    }

    // ---------------------------------------------------------------- mensajería (Edge aislado)

    fn services(&self) -> Vec<ServiceItem> {
        self.all_services()
            .into_iter()
            .map(|s| ServiceItem {
                has_session: self.core.dir.join("web-edge").join(&s.id).exists(),
                initial: s.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into()).into(),
                id: s.id.into(),
                name: s.name.into(),
                url: s.url.into(),
                custom: s.custom,
            })
            .collect()
    }

    fn edge_exe() -> Option<std::path::PathBuf> {
        ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"]
            .iter()
            .filter_map(|v| std::env::var(v).ok())
            .map(|b| std::path::PathBuf::from(b).join(r"Microsoft\Edge\Application\msedge.exe"))
            .find(|p| p.exists())
    }

    fn web_open(self: &Arc<Self>, idx: usize) {
        let Some(s) = self.all_services().into_iter().nth(idx) else { return };
        if self.core.is_locked() {
            return;
        }
        {
            let mut map = self.edge.lock().unwrap();
            if let Some(ch) = map.get_mut(&s.id) {
                if matches!(ch.try_wait(), Ok(None)) {
                    drop(map);
                    self.status(format!("{} ya está abierto", s.name));
                    return;
                }
            }
        }
        let Some(exe) = Self::edge_exe() else {
            self.status("No se encontró Microsoft Edge en este PC");
            return;
        };
        let profile = self.core.dir.join("web-edge").join(&s.id);
        let _ = std::fs::create_dir_all(&profile);
        match std::process::Command::new(exe)
            .arg(format!("--app={}", s.url))
            .arg(format!("--user-data-dir={}", profile.display()))
            .args(["--no-first-run", "--no-default-browser-check", "--disable-sync"])
            .spawn()
        {
            Ok(ch) => {
                self.edge.lock().unwrap().insert(s.id.clone(), ch);
                let list = self.services();
                self.ui(move |w| w.set_services(model(list)));
            }
            Err(e) => self.status(format!("No se pudo abrir {}: {e}", s.name)),
        }
    }

    fn kill_tree(ch: &mut std::process::Child) {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill").args(["/PID", &ch.id().to_string(), "/T", "/F"]).creation_flags(0x0800_0000).status();
        let _ = ch.wait();
    }

    pub fn edge_close_all(&self) {
        let children: Vec<_> = self.edge.lock().unwrap().drain().map(|(_, c)| c).collect();
        for mut c in children {
            Self::kill_tree(&mut c);
        }
    }

    fn web_forget(self: &Arc<Self>, idx: usize) {
        let Some(s) = self.all_services().into_iter().nth(idx) else { return };
        if let Some(mut c) = self.edge.lock().unwrap().remove(&s.id) {
            Self::kill_tree(&mut c);
        }
        let _ = std::fs::remove_dir_all(self.core.dir.join("web-edge").join(&s.id));
        let list = self.services();
        self.ui(move |w| w.set_services(model(list)));
        self.status(format!("Sesión de {} olvidada", s.name));
    }

    // ---------------------------------------------------------------- ajustes

    fn refresh_tools(self: &Arc<Self>) {
        let me = self.clone();
        self.rt.spawn_blocking(move || {
            let adb = crate::adb::adb_version();
            let scrcpy = crate::appwin::find_scrcpy().is_some();
            let text = format!(
                "adb: {}. scrcpy: {}.",
                adb.map_or("no instalado".to_string(), |v| format!("instalado ({v})")),
                if scrcpy { "instalado" } else { "no instalado" }
            );
            me.ui(move |w| w.set_tools_text(text.into()));
        });
    }
}

#[cfg(windows)]
fn place_bottom_right(w: &ToastWindow, width: f32, height: f32) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};
    let mut r = RECT { left: 0, top: 0, right: 1920, bottom: 1080 };
    unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut RECT as *mut _, 0);
    }
    let scale = w.window().scale_factor();
    let x = r.right as f32 - (width + 16.0) * scale;
    let y = r.bottom as f32 - (height + 16.0) * scale;
    w.window().set_position(slint::PhysicalPosition::new(x as i32, y as i32));
}

// ------------------------------------------------------------------------------------------------ cableado

pub fn start_timers(ctx: &Arc<Ctx>) -> slint::Timer {
    // Cada 30 s: tiempos relativos de las notificaciones y estado (cuenta atrás de intentos).
    let t = slint::Timer::default();
    let c = ctx.clone();
    t.start(slint::TimerMode::Repeated, Duration::from_secs(30), move || {
        c.push_notifs();
        c.push_state();
    });
    t
}

pub fn wire(win: &AppWindow, ctx: &Arc<Ctx>) {
    let c = ctx.clone();
    win.on_unlock(move |pw| {
        let (c, pw) = (c.clone(), pw.to_string());
        let r = if !c.core.snapshot().has_password { c.core.set_password(&pw).and_then(|_| c.core.unlock_with_password(&pw)) } else { c.core.unlock_with_password(&pw) };
        c.core.notify(&c.sink());
        let err = r.err().unwrap_or_default();
        c.ui(move |w| w.set_lock_error(err.into()));
    });
    let c = ctx.clone();
    win.on_lock_now(move || {
        c.core.lock_now();
        c.core.notify(&c.sink());
    });
    let c = ctx.clone();
    win.on_page_changed(move |p| match p {
        0 => {
            c.load_pc(None);
            c.load_phone(None);
        }
        1 => c.push_notifs(),
        2 => {
            if c.gallery.lock().unwrap().items.is_empty() {
                c.load_albums();
                c.load_gallery(true);
            }
        }
        3 => {
            c.load_contacts(String::new());
            c.load_calllog();
        }
        7 => c.load_threads(),
        4 => {
            if c.apps.lock().unwrap().is_empty() {
                c.load_apps();
            }
        }
        5 => {
            let list = c.services();
            c.ui(move |w| w.set_services(model(list)));
        }
        _ => c.refresh_tools(),
    });

    // ---- archivos
    let c = ctx.clone();
    win.on_pane_toggle(move |phone, i| {
        {
            let mut f = c.files.lock().unwrap();
            let p = if phone { &mut f.phone } else { &mut f.pc };
            if let Some(k) = p.visible().get(i as usize).copied() {
                p.sel[k] = !p.sel[k];
            }
        }
        c.apply_files(phone);
    });
    let c = ctx.clone();
    win.on_pane_open(move |phone, i| {
        let target = {
            let f = c.files.lock().unwrap();
            let p = if phone { &f.phone } else { &f.pc };
            p.visible().get(i as usize).and_then(|&k| p.rows.get(k)).filter(|r| r.dir).map(|r| if phone { join_phone(&p.path, &r.name) } else { join_pc(&p.path, &r.name) })
        };
        if let Some(t) = target {
            if phone { c.load_phone(Some(t)) } else { c.load_pc(Some(t)) }
        }
    });
    let c = ctx.clone();
    win.on_pane_up(move |phone| {
        let parent = {
            let f = c.files.lock().unwrap();
            if phone { f.phone.parent.clone() } else { f.pc.parent.clone() }
        };
        if let Some(p) = parent {
            if phone { c.load_phone(Some(p)) } else { c.load_pc(Some(p)) }
        }
    });
    let c = ctx.clone();
    win.on_pane_refresh(move |phone| if phone { c.load_phone(None) } else { c.load_pc(None) });
    let c = ctx.clone();
    win.on_send_to_phone(move || {
        let items = c.selected(false);
        if items.is_empty() {
            c.status("Marca primero lo que quieras enviar en Este PC");
            return;
        }
        let remote = c.files.lock().unwrap().phone.path.clone();
        c.xfer_kind.store(1, Ordering::SeqCst);
        let (id, core) = (c.core.next_job_id(), c.core.clone());
        c.rt.spawn(crate::transfer::run_upload(c.sink(), core, id, items, remote));
    });
    let c = ctx.clone();
    win.on_bring_to_pc(move || {
        let items = c.selected(true);
        let local = c.files.lock().unwrap().pc.path.clone();
        if items.is_empty() {
            c.status("Marca primero lo que quieras traer en Móvil");
            return;
        }
        if local.is_empty() {
            c.status("Abre una carpeta del PC como destino");
            return;
        }
        c.xfer_kind.store(2, Ordering::SeqCst);
        let (id, core) = (c.core.next_job_id(), c.core.clone());
        c.rt.spawn(crate::transfer::run_download(c.sink(), core, id, items, local));
    });
    let c = ctx.clone();
    win.on_cancel_transfer(move || {
        c.core.epoch.fetch_add(1, Ordering::SeqCst);
    });

    // ---- notificaciones
    let c = ctx.clone();
    win.on_notif_dismiss(move |key| {
        let (c, key) = (c.clone(), key.to_string());
        c.rt.clone().spawn(async move {
            if c.core.call("notif_dismiss", json!({ "key": key })).await.is_err() {
                // Las que no existen en el móvil (texto o archivos compartidos) se quitan solo del PC.
                c.core.remove_local(&c.sink(), &key);
            }
        });
    });
    let c = ctx.clone();
    win.on_notif_reply(move |key, idx, text| c.reply(key.to_string(), idx, text.to_string()));
    let c = ctx.clone();
    win.on_call_answer(move |key| c.call_op("call_answer", key.to_string(), true));
    let c = ctx.clone();
    win.on_call_end(move |key| c.call_op("call_end", key.to_string(), false));
    let c = ctx.clone();
    win.on_notif_action(move |key, i| c.notif_action(key.to_string(), i));
    let c = ctx.clone();
    win.on_clear_notifs(move || {
        let c = c.clone();
        c.rt.clone().spawn(async move {
            match c.core.call("notif_clear_all", json!({})).await {
                Ok(r) => {
                    c.core.remove_shared(&c.sink());
                    let n = r["n"].as_u64().unwrap_or(0);
                    c.status(if n > 0 { format!("{n} descartadas en el móvil") } else { "Nada que descartar".into() });
                }
                Err(e) => c.status(e),
            }
        });
    });

    // ---- aplicaciones
    let c = ctx.clone();
    win.on_apps_refresh(move || {
        if c.apps.lock().unwrap().is_empty() {
            c.load_apps()
        } else {
            c.apply_apps(false);
            c.fetch_missing_icons();
        }
    });
    let c = ctx.clone();
    win.on_app_select(move |i| c.ui(move |w| w.set_app_selected(i)));
    let c = ctx.clone();
    win.on_app_open_pc(move || {
        let c = c.clone();
        c.ui_with_app(|c, pkg, label| {
            c.status(format!("Abriendo {label}"));
            let c2 = c.clone();
            c.rt.spawn_blocking(move || match crate::api::app_open_pc_blocking(&c2.core, &pkg, &label) {
                Ok(m) => {
                    c2.status(m);
                    c2.apply_apps(false);
                }
                Err(e) => c2.status(e),
            });
        });
    });
    let c = ctx.clone();
    win.on_app_action(move |action| {
        let action = action.to_string();
        c.ui_with_app(move |c, pkg, _| {
            let c2 = c.clone();
            let action = action.clone();
            c.rt.spawn_blocking(move || match crate::api::app_action_blocking(&c2.core, &pkg, &action, None) {
                Ok(m) => c2.status(m),
                Err(e) => c2.status(e),
            });
        });
    });

    // ---- galería, llamadas y acciones
    let c = ctx.clone();
    win.on_gallery_refresh(move || c.load_gallery(true));
    let c = ctx.clone();
    win.on_gallery_more(move || c.load_gallery(false));
    let c = ctx.clone();
    win.on_gallery_toggle(move |i| {
        if let Some(m) = c.gallery.lock().unwrap().items.get_mut(i as usize) {
            m.selected = !m.selected;
        }
        c.apply_gallery();
    });
    let c = ctx.clone();
    win.on_gallery_fetch(move || c.gallery_fetch());
    let c = ctx.clone();
    win.on_dial(move |n| c.dial(n.to_string()));
    let c = ctx.clone();
    win.on_contacts_search(move |q| c.load_contacts(q.to_string()));
    let c = ctx.clone();
    win.on_ring_phone(move |on| c.ring_phone(on));
    let c = ctx.clone();
    win.on_send_clipboard(move || c.send_clipboard());
    let c = ctx.clone();
    win.on_copy_code(move |code| c.copy_code(code.to_string()));

    wire_extras(win, ctx);
    wire_files_extra(win, ctx);
    wire_more(win, ctx);
    wire_sms(win, ctx);

    // ---- mensajería
    let c = ctx.clone();
    win.on_web_open(move |i| c.web_open(i as usize));
    let c = ctx.clone();
    win.on_web_forget(move |i| c.web_forget(i as usize));

    // ---- ajustes
    let c = ctx.clone();
    win.on_make_qr(move || match crate::api::pairing_info(&c.core) {
        Ok(info) => {
            let text = format!("Huella del PC: {}. Comprueba que la misma aparezca en el móvil al confirmar.", info.fp_short);
            c.ui(move |w| {
                if let Some(img) = qr_image(&info.payload) {
                    w.set_qr(img);
                }
                w.set_qr_text(text.into());
                let weak = w.as_weak();
                slint::Timer::single_shot(Duration::from_secs(info.expires_secs), move || {
                    if let Some(w) = weak.upgrade() {
                        w.set_qr(slint::Image::default());
                        w.set_qr_text("El código caducó. Genera otro.".into());
                    }
                });
            });
        }
        Err(e) => c.status(e),
    });
    let c = ctx.clone();
    win.on_copy_pairing(move || match crate::api::pairing_info(&c.core) {
        Ok(info) => match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(info.payload)) {
            Ok(()) => c.status("Código de vinculación copiado: pégalo en «Vincular con el código» de la app del móvil (vale 5 minutos)"),
            Err(e) => c.status(format!("No se pudo copiar: {e}")),
        },
        Err(e) => c.status(e),
    });
    let c = ctx.clone();
    win.on_remove_device(move |id| {
        if c.core.is_locked() {
            return;
        }
        c.core.remove_device(&id);
        c.core.notify(&c.sink());
    });
    let c = ctx.clone();
    win.on_toggled_autostart(move |on| {
        if let Err(e) = crate::autostart::set_autostart(on) {
            c.status(e);
        }
    });
    let c = ctx.clone();
    win.on_toggled_mute(move |on| c.core.set_mute_on_call(on));
    let c = ctx.clone();
    win.on_toggled_advanced(move |on| {
        c.core.set_advanced(on);
        let c = c.clone();
        c.rt.clone().spawn(async move {
            let _ = c.core.call("set_mode", json!({ "advanced": on })).await;
        });
    });
    let c = ctx.clone();
    win.on_install_tool(move |which| {
        let (c, which) = (c.clone(), which.to_string());
        c.status(format!("Instalando {which}..."));
        c.rt.clone().spawn_blocking(move || {
            let r = match which.as_str() {
                "adb" => crate::tools::install_adb().map(|_| "adb instalado".to_string()),
                _ => crate::tools::install_scrcpy().map(|_| "scrcpy instalado".to_string()),
            };
            match r {
                Ok(m) => c.status(m),
                Err(e) => c.status(e),
            }
            c.refresh_tools();
        });
    });
}

impl Ctx {
    /// Llama a `f` con la app seleccionada en la pestaña Aplicaciones (paquete y nombre).
    fn ui_with_app(self: &Arc<Self>, f: impl FnOnce(&Arc<Ctx>, String, String) + Send + 'static) {
        let me = self.clone();
        self.ui(move |w| {
            let i = w.get_app_selected();
            if let Some(a) = w.get_apps().row_data(i.max(0) as usize).filter(|_| i >= 0) {
                f(&me, a.pkg.to_string(), a.label.to_string());
            }
        });
    }
}

impl Ctx {
    // ---------------------------------------------------------------- galería

    fn apply_gallery(self: &Arc<Self>) {
        let (rows, selected): (Vec<(PhotoMeta, Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>)>, usize) = {
            let g = self.gallery.lock().unwrap();
            (g.items.iter().map(|m| (m.clone(), g.thumbs.get(&m.path).cloned())).collect(), g.items.iter().filter(|m| m.selected).count())
        };
        self.ui(move |w| {
            let photos: Vec<Photo> = rows
                .into_iter()
                .map(|(m, t)| Photo {
                    path: m.path.into(),
                    name: m.name.into(),
                    video: m.video,
                    selected: m.selected,
                    has_thumb: t.is_some(),
                    thumb: t.map(slint::Image::from_rgba8).unwrap_or_default(),
                })
                .collect();
            w.set_photos(model(photos));
            w.set_gallery_selected(selected as i32);
        });
    }

    fn load_gallery(self: &Arc<Self>, reset: bool) {
        let me = self.clone();
        self.rt.spawn(async move {
            let offset = if reset { 0 } else { me.gallery.lock().unwrap().items.len() };
            let album = me.album.lock().unwrap().clone();
            match me.core.call("media_recent", json!({ "offset": offset, "limit": 36, "refresh": reset, "album": album })).await {
                Ok(r) => {
                    let new: Vec<PhotoMeta> = r["items"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|x| PhotoMeta {
                                    path: x["path"].as_str().unwrap_or("").into(),
                                    name: x["name"].as_str().unwrap_or("").into(),
                                    video: x["video"] == true,
                                    size: x["size"].as_u64().unwrap_or(0),
                                    selected: false,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let total = r["total"].as_u64().unwrap_or(0);
                    {
                        let mut g = me.gallery.lock().unwrap();
                        if reset {
                            *g = Gallery::default();
                        }
                        g.items.extend(new.iter().cloned());
                    }
                    let shown = me.gallery.lock().unwrap().items.len();
                    me.ui(move |w| w.set_gallery_message(if total == 0 { "No hay fotos ni vídeos en DCIM, Pictures o Movies.".into() } else { format!("{shown} de {total}").into() }));
                    me.apply_gallery();
                    // Miniaturas en tandas de 12 (así aparecen poco a poco).
                    for chunk in new.chunks(12) {
                        let paths: Vec<&str> = chunk.iter().map(|m| m.path.as_str()).collect();
                        if let Ok(t) = me.core.call("media_thumb", json!({ "paths": paths })).await {
                            let decoded: Vec<(String, slint::SharedPixelBuffer<slint::Rgba8Pixel>)> = t["thumbs"]
                                .as_object()
                                .map(|o| o.iter().filter_map(|(k, v)| decode_thumb(v.as_str()?).map(|b| (k.clone(), b))).collect())
                                .unwrap_or_default();
                            me.gallery.lock().unwrap().thumbs.extend(decoded);
                            me.apply_gallery();
                        }
                    }
                }
                Err(e) => {
                    let e = if me.core.snapshot().present { e } else { "El móvil no está conectado.".to_string() };
                    me.ui(move |w| w.set_gallery_message(e.into()));
                }
            }
        });
    }

    fn gallery_fetch(self: &Arc<Self>) {
        let items: Vec<Item> = self
            .gallery
            .lock()
            .unwrap()
            .items
            .iter()
            .filter(|m| m.selected)
            .map(|m| Item { path: m.path.clone(), name: m.name.clone(), dir: false, size: m.size })
            .collect();
        if items.is_empty() {
            return;
        }
        let dir = crate::share::inbox_dir().join("Galería");
        let _ = std::fs::create_dir_all(&dir);
        self.status(format!("Trayendo {} archivo(s) a {}", items.len(), dir.display()));
        self.xfer_kind.store(2, Ordering::SeqCst);
        let (id, core) = (self.core.next_job_id(), self.core.clone());
        self.rt.spawn(crate::transfer::run_download(self.sink(), core, id, items, dir.to_string_lossy().into_owned()));
    }

    // ---------------------------------------------------------------- llamadas y contactos

    fn load_contacts(self: &Arc<Self>, q: String) {
        let me = self.clone();
        self.rt.spawn(async move {
            match me.core.call("contacts", json!({ "q": q })).await {
                Ok(r) => {
                    let rows: Vec<(String, String)> = r["items"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .flat_map(|c| {
                                    let name = c["name"].as_str().unwrap_or("").to_string();
                                    c["numbers"].as_array().cloned().unwrap_or_default().into_iter().map(move |n| (name.clone(), n.as_str().unwrap_or("").to_string()))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    me.ui(move |w| {
                        w.set_contacts(model(rows.into_iter().map(|(name, number)| ContactItem { name: name.into(), number: number.into() }).collect()));
                        w.set_calls_message("".into());
                    });
                }
                Err(e) => {
                    let e = if me.core.snapshot().present { e } else { "El móvil no está conectado.".to_string() };
                    me.ui(move |w| w.set_calls_message(e.into()));
                }
            }
        });
    }

    fn load_calllog(self: &Arc<Self>) {
        let me = self.clone();
        self.rt.spawn(async move {
            if let Ok(r) = me.core.call("calllog", json!({ "limit": 80 })).await {
                let rows: Vec<CallItem> = r["items"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|x| {
                                let number = x["number"].as_str().unwrap_or("").to_string();
                                let name = x["name"].as_str().unwrap_or("");
                                let kind = match x["type"].as_i64().unwrap_or(0) {
                                    1 => 1,
                                    2 => 2,
                                    _ => 3,
                                };
                                let secs = x["secs"].as_u64().unwrap_or(0);
                                let label = ["", "Entrante", "Saliente", "Perdida"][kind as usize];
                                let dur = if secs > 0 { format!(" · {}:{:02}", secs / 60, secs % 60) } else { String::new() };
                                CallItem {
                                    title: if name.is_empty() { number.clone() } else { name.to_string() }.into(),
                                    detail: format!("{label} · {}{dur}", ago(x["date"].as_u64().unwrap_or(0))).into(),
                                    number: number.into(),
                                    kind,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                me.ui(move |w| w.set_call_log(model(rows)));
            }
        });
    }

    fn dial(self: &Arc<Self>, number: String) {
        let number = number.trim().to_string();
        if number.is_empty() {
            self.status("Escribe un número");
            return;
        }
        let me = self.clone();
        self.rt.spawn(async move {
            match me.core.call("dial", json!({ "number": number })).await {
                Ok(_) => me.status(format!("Llamando a {number}")),
                Err(e) => me.status(e),
            }
        });
    }

    // ---------------------------------------------------------------- acciones del móvil

    fn ring_phone(self: &Arc<Self>, on: bool) {
        let me = self.clone();
        self.rt.spawn(async move {
            match me.core.call("find_ring", json!({ "on": on })).await {
                Ok(r) => {
                    let ringing = r["on"] == true;
                    me.ui(move |w| w.set_ringing(ringing));
                    if ringing {
                        // El móvil lo apaga solo al minuto: la interfaz también.
                        tokio::time::sleep(Duration::from_secs(61)).await;
                        me.ui(|w| w.set_ringing(false));
                    }
                }
                Err(e) => me.status(e),
            }
        });
    }

    fn send_clipboard(self: &Arc<Self>) {
        let text = arboard::Clipboard::new().and_then(|mut c| c.get_text()).unwrap_or_default();
        if text.trim().is_empty() {
            self.status("El portapapeles del PC no tiene texto");
            return;
        }
        let me = self.clone();
        self.rt.spawn(async move {
            match me.core.call("clip_set", json!({ "text": text })).await {
                Ok(_) => me.status("Portapapeles copiado en el móvil"),
                Err(e) => me.status(e),
            }
        });
    }

    fn copy_code(self: &Arc<Self>, code: String) {
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(code)) {
            Ok(()) => self.status("Código copiado al portapapeles"),
            Err(e) => self.status(format!("No se pudo copiar: {e}")),
        }
    }

    /// Archivos soltados desde el Explorador sobre la ventana: se envían a la carpeta del móvil que se está viendo.
    pub fn drop_files(self: &Arc<Self>, paths: Vec<String>) {
        if self.core.is_locked() {
            return;
        }
        if !self.core.snapshot().present {
            self.status("El móvil no está conectado: no se puede enviar");
            return;
        }
        let items = match crate::api::local_stat(&self.core, paths) {
            Ok(i) if !i.is_empty() => i,
            _ => return,
        };
        let remote = self.files.lock().unwrap().phone.path.clone();
        self.status(format!("Enviando {} elemento(s) al móvil", items.len()));
        self.xfer_kind.store(1, Ordering::SeqCst);
        let (id, core) = (self.core.next_job_id(), self.core.clone());
        self.rt.spawn(crate::transfer::run_upload(self.sink(), core, id, items, remote));
    }
}

#[cfg(test)]
mod code_tests {
    use super::extract_code;

    #[test]
    fn verification_codes_are_found_only_when_the_text_talks_about_codes() {
        assert_eq!(extract_code("Tu código de verificación es 482913"), "482913");
        assert_eq!(extract_code("G-123456 es tu código de Google"), "123456");
        assert_eq!(extract_code("Your verification code: 123 456"), "123456");
        assert_eq!(extract_code("Tu pedido 123456789012 llegará mañana"), "");
        assert_eq!(extract_code("Llámame a las 5555"), "");
    }
}

// ------------------------------------------------------------------------------------------------ ajustes guardados, seguridad,
// copia de fotos, envío rápido y actualizaciones

#[derive(Serialize, serde::Deserialize, Default, Clone)]
struct Settings {
    lock_windows: bool,
    hide_content: bool,
    mute_words: String,
    photo_backup: bool,
    /// Instante (ms) en que se activó la copia de fotos: solo se copian las posteriores.
    backup_since: u64,
    backed_up: Vec<String>,
    custom_services: Vec<CustomService>,
}

/// Los ajustes de la interfaz van en `ui-settings.json`. `settings.json` es del agente (sellado con firma): la 0.4.0 escribía aquí por error
/// y se pisaban entre sí; si queda un archivo de esa época se migra y se retira para que el agente arranque limpio.
fn load_settings(dir: &std::path::Path) -> Settings {
    let new = dir.join("ui-settings.json");
    let old = dir.join("settings.json");
    if let Ok(t) = std::fs::read_to_string(&old) {
        if t.contains("\"lock_windows\"") {
            if !new.exists() {
                let _ = std::fs::write(&new, &t);
            }
            let _ = std::fs::remove_file(&old);
            let _ = std::fs::remove_file(dir.join("settings.json.sig"));
        }
    }
    std::fs::read_to_string(&new).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// ¿La notificación contiene alguna de las palabras a silenciar (separadas por comas)?
pub fn matches_mute(words: &str, title: &str, text: &str) -> bool {
    let hay = format!("{title} {text}").to_lowercase();
    words.split(',').map(|w| w.trim().to_lowercase()).any(|w| !w.is_empty() && hay.contains(&w))
}

/// Clave pública con la que se firman las publicaciones (la privada vive fuera del repositorio; ver tools/release-sign).
const UPDATE_PUBKEY: &str = "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEQZm3UR1eoau29Dz8aMS4dggO8d48L9zrVymDRyeDrjEjOXvbgo7v1EuAYGwp9VbxXcHCHnv65JG7MvidM3Iqvg==";

/// ¿`sig_b64` (ECDSA P-256 SHA-256, DER en base64) es una firma válida de `data` con la clave pública `pub_b64`?
pub fn verify_sig(pub_b64: &str, data: &[u8], sig_b64: &str) -> bool {
    use base64::Engine;
    use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
    use p256::pkcs8::DecodePublicKey;
    let b = base64::engine::general_purpose::STANDARD;
    let Ok(pk) = b.decode(pub_b64.trim()) else { return false };
    let Ok(sg) = b.decode(sig_b64.trim()) else { return false };
    let (Ok(key), Ok(sig)) = (VerifyingKey::from_public_key_der(&pk), Signature::from_der(&sg)) else { return false };
    key.verify(data, &sig).is_ok()
}

fn version_tuple(v: &str) -> Vec<u32> {
    v.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    version_tuple(latest) > version_tuple(current)
}

thread_local! {
    static SEND: RefCell<(Vec<String>, slint::Timer)> = RefCell::new((Vec::new(), slint::Timer::default()));
}

/// Junta los archivos soltados o enviados desde el Explorador y, medio segundo después, los envía al móvil de una vez.
pub fn queue_send(ctx: &Arc<Ctx>, path: String) {
    let c = ctx.clone();
    SEND.with(|s| {
        let mut s = s.borrow_mut();
        s.0.push(path);
        s.1.start(slint::TimerMode::SingleShot, Duration::from_millis(500), move || {
            let paths = SEND.with(|s| std::mem::take(&mut s.borrow_mut().0));
            c.drop_files(paths);
        });
    });
}

impl Ctx {
    fn save_settings(&self) {
        let s = self.settings.lock().unwrap().clone();
        if let Ok(t) = serde_json::to_string_pretty(&s) {
            let _ = std::fs::write(self.core.dir.join("ui-settings.json"), t);
        }
    }

    /// Carga los ajustes guardados en la interfaz.
    pub fn apply_settings(self: &Arc<Self>) {
        let s = self.settings.lock().unwrap().clone();
        self.ui(move |w| {
            w.set_lock_windows(s.lock_windows);
            w.set_hide_content(s.hide_content);
            w.set_mute_words(s.mute_words.into());
            w.set_photo_backup(s.photo_backup);
        });
    }

    pub fn hide_content(&self) -> bool {
        self.settings.lock().unwrap().hide_content
    }

    pub fn muted_by_words(&self, title: &str, text: &str) -> bool {
        matches_mute(&self.settings.lock().unwrap().mute_words, title, text)
    }

    /// Bloquea la sesión de Windows si el móvil sigue ausente 15 s después de irse (y el ajuste está activo).
    pub fn on_presence(self: &Arc<Self>, present: bool) {
        let was = self.last_present.swap(present, Ordering::SeqCst);
        if was && !present && self.settings.lock().unwrap().lock_windows {
            let me = self.clone();
            self.rt.spawn(async move {
                tokio::time::sleep(Duration::from_secs(15)).await;
                if !me.core.snapshot().present && me.settings.lock().unwrap().lock_windows {
                    me.core.seclog("el móvil se fue: se bloquea la sesión de Windows");
                    #[cfg(windows)]
                    unsafe {
                        windows_sys::Win32::System::Shutdown::LockWorkStation();
                    }
                }
            });
        }
    }

    // ---------------------------------------------------------------- copia automática de fotos

    pub fn start_photo_backup(self: &Arc<Self>) {
        let me = self.clone();
        self.rt.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(300)).await;
                if me.settings.lock().unwrap().photo_backup && !me.core.is_locked() && me.core.snapshot().present {
                    me.backup_round().await;
                }
            }
        });
    }

    async fn backup_round(self: &Arc<Self>) {
        let Ok(r) = self.core.call("media_recent", json!({ "offset": 0, "limit": 40, "refresh": true })).await else { return };
        let (since, done) = {
            let s = self.settings.lock().unwrap();
            (s.backup_since, s.backed_up.clone())
        };
        let items: Vec<Item> = r["items"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|x| x["mtime"].as_u64().unwrap_or(0) > since)
                    .filter(|x| x["path"].as_str().is_some_and(|p| !done.iter().any(|d| d == p)))
                    .map(|x| Item {
                        path: x["path"].as_str().unwrap_or("").into(),
                        name: x["name"].as_str().unwrap_or("").into(),
                        dir: false,
                        size: x["size"].as_u64().unwrap_or(0),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if items.is_empty() {
            return;
        }
        let dir = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Pictures").join("SyncroUmbral").join("Copia");
        let _ = std::fs::create_dir_all(&dir);
        {
            let mut s = self.settings.lock().unwrap();
            for i in &items {
                s.backed_up.push(i.path.clone());
            }
            let n = s.backed_up.len();
            if n > 5000 {
                s.backed_up.drain(..n - 5000);
            }
        }
        self.save_settings();
        self.status(format!("Copiando {} foto(s) nueva(s) al PC", items.len()));
        self.xfer_kind.store(2, Ordering::SeqCst);
        let (id, core) = (self.core.next_job_id(), self.core.clone());
        crate::transfer::run_download(self.sink(), core, id, items, dir.to_string_lossy().into_owned()).await;
    }

    // ---------------------------------------------------------------- menú del Explorador

    fn context_menu(self: &Arc<Self>, on: bool) {
        let Ok(exe) = std::env::current_exe() else { return };
        let mut ok = true;
        for base in [r"HKCU\Software\Classes\*\shell\SyncroUmbral", r"HKCU\Software\Classes\Directory\shell\SyncroUmbral"] {
            let r = if on {
                let cmd = format!("\"{}\" --send \"%1\"", exe.display());
                crate::autostart::reg_run(&["add", base, "/ve", "/d", "Enviar al móvil con SyncroUmbral", "/f"])
                    && crate::autostart::reg_run(&["add", &format!(r"{base}\command"), "/ve", "/d", &cmd, "/f"])
            } else {
                crate::autostart::reg_run(&["delete", base, "/f"])
            };
            ok &= r || !on;
        }
        self.status(match (on, ok) {
            (true, true) => "Añadido: clic derecho en un archivo, «Enviar al móvil con SyncroUmbral»",
            (false, _) => "Quitado del menú del Explorador",
            _ => "No se pudo modificar el menú del Explorador",
        });
    }

    // ---------------------------------------------------------------- actualizaciones (GitHub Releases)

    pub fn start_update_check(self: &Arc<Self>) {
        let me = self.clone();
        self.rt.spawn(async move {
            loop {
                let m2 = me.clone();
                let found = tokio::task::spawn_blocking(move || m2.check_update()).await.ok().flatten();
                if let Some((ver, _, _, _)) = found {
                    me.ui(move |w| w.set_update_text(format!("Hay una versión nueva de SyncroUmbral: {ver}").into()));
                }
                tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
            }
        });
    }

    /// (versión, url del exe, url de SHA256SUMS) de la última versión publicada si es más nueva que la instalada.
    fn check_update(&self) -> Option<(String, String, String, String)> {
        let r: Value = ureq::get("https://api.github.com/repos/juandr404/syncroumbral/releases?per_page=1")
            .set("User-Agent", "SyncroUmbral")
            .timeout(Duration::from_secs(15))
            .call()
            .ok()?
            .into_json()
            .ok()?;
        let rel = r.get(0)?;
        let tag = rel["tag_name"].as_str()?.to_string();
        if !is_newer(&tag, env!("CARGO_PKG_VERSION")) {
            return None;
        }
        let find = |suffix: &str| {
            rel["assets"].as_array()?.iter().find_map(|a| {
                let n = a["name"].as_str()?;
                let u = a["browser_download_url"].as_str()?;
                (n.ends_with(suffix) && u.starts_with("https://github.com/juandr404/syncroumbral/releases/download/")).then(|| u.to_string())
            })
        };
        Some((tag.trim_start_matches('v').to_string(), find("windows-x64.exe")?, find("SHA256SUMS.txt")?, find("SHA256SUMS.txt.sig")?))
    }

    /// Descarga la versión nueva, comprueba su huella SHA-256 contra la de la misma publicación, reemplaza el ejecutable y lo reinicia.
    fn do_update(self: &Arc<Self>) {
        let me = self.clone();
        self.status("Descargando la actualización...");
        self.rt.spawn_blocking(move || {
            let r = (|| -> Result<(), String> {
                let (_, exe_url, sums_url, sig_url) = me.check_update().ok_or("Ya tienes la última versión (o la publicación no está firmada)")?;
                let get = |u: &str| -> Result<Vec<u8>, String> {
                    let mut v = Vec::new();
                    let resp = ureq::get(u).set("User-Agent", "SyncroUmbral").timeout(Duration::from_secs(300)).call().map_err(|e| e.to_string())?;
                    std::io::Read::read_to_end(&mut std::io::Read::take(resp.into_reader(), 100 << 20), &mut v).map_err(|e| e.to_string())?;
                    Ok(v)
                };
                let sums_raw = get(&sums_url)?;
                let sig = String::from_utf8(get(&sig_url)?).map_err(|e| e.to_string())?;
                // La lista de huellas debe venir firmada con la clave del autor: una cuenta de GitHub comprometida no basta.
                if !verify_sig(UPDATE_PUBKEY, &sums_raw, &sig) {
                    return Err("La firma de la actualización no es válida: no se instala".into());
                }
                let sums = String::from_utf8(sums_raw).map_err(|e| e.to_string())?;
                let bin = get(&exe_url)?;
                let name = exe_url.rsplit('/').next().unwrap_or("");
                let want = sums.lines().find_map(|l| {
                    let (h, n) = l.split_once(' ')?;
                    (n.trim().trim_start_matches('*') == name).then(|| h.to_lowercase())
                }).ok_or("La publicación no trae la huella del archivo")?;
                use sha2::{Digest, Sha256};
                if hex::encode(Sha256::digest(&bin)) != want {
                    return Err("La huella de la descarga no coincide: no se instala".into());
                }
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let old = exe.with_extension("old");
                let _ = std::fs::remove_file(&old);
                std::fs::rename(&exe, &old).map_err(|e| e.to_string())?; // un ejecutable en marcha se puede renombrar
                if let Err(e) = std::fs::write(&exe, &bin) {
                    let _ = std::fs::rename(&old, &exe);
                    return Err(e.to_string());
                }
                std::process::Command::new(&exe).arg("--background").spawn().map_err(|e| e.to_string())?;
                Ok(())
            })();
            match r {
                Ok(()) => {
                    let _ = slint::invoke_from_event_loop(|| {
                        let _ = slint::quit_event_loop();
                    });
                }
                Err(e) => me.status(e),
            }
        });
    }
}

pub fn wire_extras(win: &AppWindow, ctx: &Arc<Ctx>) {
    let c = ctx.clone();
    win.on_toggled_lock_windows(move |v| {
        c.settings.lock().unwrap().lock_windows = v;
        c.save_settings();
    });
    let c = ctx.clone();
    win.on_toggled_hide(move |v| {
        c.settings.lock().unwrap().hide_content = v;
        c.save_settings();
        c.push_notifs();
        c.refresh_toast();
    });
    let c = ctx.clone();
    win.on_mute_words_edited(move |t| {
        c.settings.lock().unwrap().mute_words = t.to_string();
        c.save_settings();
    });
    let c = ctx.clone();
    win.on_toggled_backup(move |v| {
        {
            let mut s = c.settings.lock().unwrap();
            s.photo_backup = v;
            if v {
                s.backup_since = now_ms();
            }
        }
        c.save_settings();
        c.status(if v { "Copia automática activada: se copiarán las fotos nuevas" } else { "Copia automática desactivada" });
    });
    let c = ctx.clone();
    win.on_context_menu(move |on| c.context_menu(on));
    let c = ctx.clone();
    win.on_do_update(move || c.do_update());
    ctx.apply_settings();
}

#[cfg(test)]
mod extras_tests {
    use super::*;

    #[test]
    fn update_signatures_are_checked() {
        use base64::Engine;
        use p256::ecdsa::{signature::Signer, Signature, SigningKey};
        use p256::pkcs8::EncodePublicKey;
        let b = base64::engine::general_purpose::STANDARD;
        let key = SigningKey::random(&mut rand::rngs::OsRng);
        let pubk = b.encode(key.verifying_key().to_public_key_der().unwrap().as_bytes());
        let sig: Signature = key.sign(b"hash  archivo");
        let sig = b.encode(sig.to_der().as_bytes());
        assert!(verify_sig(&pubk, b"hash  archivo", &sig));
        assert!(!verify_sig(&pubk, b"hash  archivo MODIFICADO", &sig), "un texto cambiado no vale");
        let otra = SigningKey::random(&mut rand::rngs::OsRng);
        let otra_pub = b.encode(otra.verifying_key().to_public_key_der().unwrap().as_bytes());
        assert!(!verify_sig(&otra_pub, b"hash  archivo", &sig), "otra clave no vale");
        assert!(!verify_sig(UPDATE_PUBKEY, b"x", "basura"));
    }

    #[test]
    fn mute_words_and_versions() {
        assert!(matches_mute("oferta, Sorteo", "Gran OFERTA hoy", ""));
        assert!(!matches_mute("", "oferta", "x"));
        assert!(!matches_mute(" , ", "a", "b"));
        assert!(is_newer("v0.4.1", "0.4.0") && is_newer("0.10.0", "0.9.9") && !is_newer("v0.4.0", "0.4.0"));
    }
}

impl Ctx {
    // ---------------------------------------------------------------- SMS

    fn load_threads(self: &Arc<Self>) {
        let me = self.clone();
        self.rt.spawn(async move {
            match me.core.call("sms_threads", json!({})).await {
                Ok(r) => {
                    let rows: Vec<(String, String, String, String, bool)> = r["items"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|x| {
                                    let addr = x["address"].as_str().unwrap_or("").to_string();
                                    let name = x["name"].as_str().unwrap_or("");
                                    (
                                        if name.is_empty() { addr.clone() } else { name.to_string() },
                                        addr,
                                        format!("{}{}", if x["mine"] == true { "Tú: " } else { "" }, x["snippet"].as_str().unwrap_or("")),
                                        ago(x["date"].as_u64().unwrap_or(0)),
                                        x["unread"] == true,
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    *me.threads.lock().unwrap() = rows.iter().map(|r| (r.0.clone(), r.1.clone())).collect();
                    me.ui(move |w| {
                        w.set_threads(model(rows.into_iter().map(|(title, address, snippet, time, unread)| ThreadItem { title: title.into(), address: address.into(), snippet: snippet.into(), time: time.into(), unread }).collect()));
                        w.set_sms_message("".into());
                    });
                }
                Err(e) => {
                    let e = if me.core.snapshot().present { e } else { "El móvil no está conectado.".to_string() };
                    me.ui(move |w| w.set_sms_message(e.into()));
                }
            }
        });
    }

    fn open_thread(self: &Arc<Self>, i: usize) {
        let Some((title, address)) = self.threads.lock().unwrap().get(i).cloned() else { return };
        *self.sms_open.lock().unwrap() = Some(address.clone());
        self.ui(move |w| {
            w.set_sms_selected(i as i32);
            w.set_sms_title(title.into());
        });
        let me = self.clone();
        self.rt.spawn(async move {
            if let Ok(r) = me.core.call("sms_list", json!({ "address": address, "limit": 80 })).await {
                let mut rows: Vec<(String, bool, String)> = r["items"]
                    .as_array()
                    .map(|a| a.iter().map(|x| (x["text"].as_str().unwrap_or("").to_string(), x["mine"] == true, ago(x["date"].as_u64().unwrap_or(0)))).collect())
                    .unwrap_or_default();
                rows.reverse(); // la lista llega de más nueva a más antigua
                me.sms_log.lock().unwrap().clear();
                me.sms_log.lock().unwrap().extend(rows);
                me.apply_messages();
            }
        });
    }

    fn apply_messages(self: &Arc<Self>) {
        let rows = self.sms_log.lock().unwrap().clone();
        self.ui(move |w| w.set_messages(model(rows.into_iter().map(|(text, mine, time)| MsgItem { text: text.into(), mine, time: time.into() }).collect())));
    }

    fn sms_send(self: &Arc<Self>, text: String) {
        let Some(address) = self.sms_open.lock().unwrap().clone() else { return };
        if text.trim().is_empty() {
            return;
        }
        let me = self.clone();
        self.rt.spawn(async move {
            match me.core.call("sms_send", json!({ "address": address, "text": text })).await {
                Ok(_) => {
                    // Android no guarda en Mensajes lo enviado por otra app: se muestra aquí.
                    me.sms_log.lock().unwrap().push((text, true, "ahora".into()));
                    me.apply_messages();
                }
                Err(e) => me.status(e),
            }
        });
    }
}

pub fn wire_sms(win: &AppWindow, ctx: &Arc<Ctx>) {
    let c = ctx.clone();
    win.on_sms_refresh(move || c.load_threads());
    let c = ctx.clone();
    win.on_sms_open(move |i| c.open_thread(i as usize));
    let c = ctx.clone();
    win.on_sms_send(move |t| c.sms_send(t.to_string()));
}

// ------------------------------------------------------------------------------------------------ álbumes e iconos de apps

#[derive(Clone)]
struct AlbumMeta {
    name: String,
    path: String,
    count: i32,
}

fn decode_png(b64: &str) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    let bytes = base64_decode(b64)?;
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut r = dec.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let mut out = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
    let px = out.make_mut_slice();
    match info.color_type {
        png::ColorType::Rgba => {
            for (o, p) in px.iter_mut().zip(buf.chunks_exact(4)) {
                *o = slint::Rgba8Pixel { r: p[0], g: p[1], b: p[2], a: p[3] };
            }
        }
        png::ColorType::Rgb => {
            for (o, p) in px.iter_mut().zip(buf.chunks_exact(3)) {
                *o = slint::Rgba8Pixel { r: p[0], g: p[1], b: p[2], a: 255 };
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for (o, p) in px.iter_mut().zip(buf.chunks_exact(2)) {
                *o = slint::Rgba8Pixel { r: p[0], g: p[0], b: p[0], a: p[1] };
            }
        }
        png::ColorType::Grayscale => {
            for (o, p) in px.iter_mut().zip(buf.iter()) {
                *o = slint::Rgba8Pixel { r: *p, g: *p, b: *p, a: 255 };
            }
        }
        _ => return None,
    }
    Some(out)
}

/// Una sola línea: los saltos de línea de una notificación (correos, resúmenes) se convierten en separadores y se acorta.
fn one_line(s: &str, max: usize) -> String {
    let t = s.split(['\n', '\r']).map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("  ·  ");
    if t.chars().count() > max {
        format!("{}…", t.chars().take(max).collect::<String>())
    } else {
        t
    }
}

impl Ctx {
    fn apply_albums(self: &Arc<Self>) {
        let list = self.albums.lock().unwrap().clone();
        let sel = {
            let cur = self.album.lock().unwrap().clone();
            if cur.is_empty() { -1 } else { list.iter().position(|a| a.path == cur).map_or(-1, |i| i as i32) }
        };
        self.ui(move |w| {
            w.set_albums(model(list.into_iter().map(|a| AlbumItem { name: a.name.into(), count: a.count, selected: false }).collect()));
            w.set_album_selected(sel);
        });
    }

    fn load_albums(self: &Arc<Self>) {
        let me = self.clone();
        self.rt.spawn(async move {
            if let Ok(r) = me.core.call("media_albums", json!({ "refresh": true })).await {
                let list: Vec<AlbumMeta> = r["items"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|x| AlbumMeta {
                                name: x["name"].as_str().unwrap_or("").into(),
                                path: x["path"].as_str().unwrap_or("").into(),
                                count: x["count"].as_i64().unwrap_or(0) as i32,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                *me.albums.lock().unwrap() = list;
                me.apply_albums();
            }
        });
    }

    fn choose_album(self: &Arc<Self>, i: i32) {
        let path = if i < 0 { String::new() } else { self.albums.lock().unwrap().get(i as usize).map(|a| a.path.clone()).unwrap_or_default() };
        *self.album.lock().unwrap() = path;
        self.apply_albums();
        self.load_gallery(true);
    }

    /// Pide los iconos de las apps visibles que aún no los tienen (de 40 en 40) y refresca la lista a medida que llegan.
    fn fetch_missing_icons(self: &Arc<Self>) {
        let (show_sys, q) = (self.show_system.load(Ordering::SeqCst), self.apps_q.lock().unwrap().to_lowercase());
        let missing: Vec<String> = {
            let have = self.app_icons.lock().unwrap();
            self.apps
                .lock()
                .unwrap()
                .iter()
                .filter(|a| (show_sys || !a.system) && (q.is_empty() || a.label.to_lowercase().contains(&q) || a.pkg.to_lowercase().contains(&q)))
                .filter(|a| !have.contains_key(&a.pkg))
                .take(240)
                .map(|a| a.pkg.clone())
                .collect()
        };
        if missing.is_empty() || self.icons_busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        self.rt.spawn(async move {
            for chunk in missing.chunks(40) {
                if let Ok(r) = me.core.call("icons", json!({ "pkgs": chunk })).await {
                    let got: Vec<(String, slint::SharedPixelBuffer<slint::Rgba8Pixel>)> = r["icons"]
                        .as_object()
                        .map(|o| o.iter().filter_map(|(k, v)| decode_png(v.as_str()?).map(|b| (k.clone(), b))).collect())
                        .unwrap_or_default();
                    me.app_icons.lock().unwrap().extend(got);
                    me.apply_apps(true);
                }
            }
            me.icons_busy.store(false, Ordering::SeqCst);
        });
    }
}

pub fn wire_more(win: &AppWindow, ctx: &Arc<Ctx>) {
    let c = ctx.clone();
    win.on_gallery_album(move |i| c.choose_album(i));
    let c = ctx.clone();
    win.on_apps_search(move |q| {
        *c.apps_q.lock().unwrap() = q.to_string();
        c.apply_apps(false);
        c.fetch_missing_icons();
    });
}

// ------------------------------------------------------------------------------------------------ servicios propios y lista visible

#[derive(Serialize, serde::Deserialize, Clone)]
struct CustomService {
    id: String,
    name: String,
    url: String,
}

#[derive(Clone)]
struct SvcView {
    id: String,
    name: String,
    url: String,
    custom: bool,
}

impl PaneState {
    /// Posiciones de `rows` que se ven con el filtro actual (sin distinguir mayúsculas).
    fn visible(&self) -> Vec<usize> {
        let f = self.filter.to_lowercase();
        (0..self.rows.len()).filter(|&i| f.is_empty() || self.rows[i].name.to_lowercase().contains(&f)).collect()
    }
}

/// Dirección https válida para abrir como servicio: sin espacios ni comillas (se pasa a Edge como argumento).
fn valid_service_url(u: &str) -> bool {
    let u = u.trim();
    let Some(rest) = u.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    u.len() <= 300 && !host.is_empty() && host.contains('.') && !u.chars().any(|c| c.is_whitespace() || c == '"' || c == '\'' || c.is_control())
}

impl Ctx {
    fn all_services(&self) -> Vec<SvcView> {
        let mut v: Vec<SvcView> = crate::services::SERVICES
            .iter()
            .map(|s| SvcView { id: s.id.into(), name: s.name.into(), url: s.url.into(), custom: false })
            .collect();
        v.extend(self.settings.lock().unwrap().custom_services.iter().map(|c| SvcView { id: c.id.clone(), name: c.name.clone(), url: c.url.clone(), custom: true }));
        v
    }

    fn add_service(self: &Arc<Self>, name: String, url: String) {
        let (name, url) = (name.trim().to_string(), url.trim().to_string());
        if name.is_empty() || name.chars().count() > 40 {
            self.status("Ponle un nombre (hasta 40 letras)");
            return;
        }
        if !valid_service_url(&url) {
            self.status("La dirección debe empezar por https:// y ser una web válida");
            return;
        }
        {
            let mut s = self.settings.lock().unwrap();
            if s.custom_services.len() >= 20 {
                drop(s);
                self.status("Ya tienes 20 servicios propios: quita alguno antes");
                return;
            }
            let slug: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
            let mut id = format!("c-{slug}");
            let mut n = 2;
            while crate::services::SERVICES.iter().any(|x| x.id == id) || s.custom_services.iter().any(|x| x.id == id) {
                id = format!("c-{slug}-{n}");
                n += 1;
            }
            s.custom_services.push(CustomService { id, name: name.clone(), url });
        }
        self.save_settings();
        let list = self.services();
        self.ui(move |w| w.set_services(model(list)));
        self.status(format!("{name} añadido"));
    }

    fn remove_service(self: &Arc<Self>, idx: usize) {
        let Some(s) = self.all_services().into_iter().nth(idx).filter(|s| s.custom) else { return };
        if let Some(mut c) = self.edge.lock().unwrap().remove(&s.id) {
            Self::kill_tree(&mut c);
        }
        let _ = std::fs::remove_dir_all(self.core.dir.join("web-edge").join(&s.id));
        self.settings.lock().unwrap().custom_services.retain(|x| x.id != s.id);
        self.save_settings();
        let list = self.services();
        self.ui(move |w| w.set_services(model(list)));
        self.status(format!("{} quitado", s.name));
    }

    /// Texto recibido con «Copiar al PC» del móvil: se deja directamente en el portapapeles del PC.
    fn auto_clip(self: &Arc<Self>, text: String) {
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(text)) {
            Ok(()) => self.status("Portapapeles del móvil copiado: ya puedes pegar"),
            Err(e) => self.status(format!("No se pudo copiar al portapapeles del PC: {e}")),
        }
    }
}

pub fn wire_files_extra(win: &AppWindow, ctx: &Arc<Ctx>) {
    let c = ctx.clone();
    win.on_pane_filter(move |phone, t| {
        {
            let mut f = c.files.lock().unwrap();
            let p = if phone { &mut f.phone } else { &mut f.pc };
            p.filter = t.to_string();
        }
        c.apply_files(phone);
    });
    let c = ctx.clone();
    win.on_pane_select_all(move |phone| {
        {
            let mut f = c.files.lock().unwrap();
            let p = if phone { &mut f.phone } else { &mut f.pc };
            let vis = p.visible();
            let all = !vis.is_empty() && vis.iter().all(|&k| p.sel[k]);
            for k in vis {
                p.sel[k] = !all;
            }
        }
        c.apply_files(phone);
    });
    let c = ctx.clone();
    win.on_add_service(move |n, u| c.add_service(n.to_string(), u.to_string()));
    let c = ctx.clone();
    win.on_remove_service(move |i| c.remove_service(i as usize));
}
