//! Une el agente (core, servidor, transferencias) con la interfaz Slint.
//! Todo lo que toca la interfaz ocurre en su hilo; el agente corre en el motor asíncrono.

use crate::core::Core;
use crate::transfer::Item;
use crate::ui::Ui;
use crate::{AppItem, AppWindow, DeviceItem, FileItem, Notif, NotifAction, ServiceItem, ToastWindow};
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
    path: String,
    parent: Option<String>,
    rows: Vec<Row>,
    sel: Vec<bool>,
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
        app: v["app"].as_str().or(v["pkg"].as_str()).unwrap_or("").into(),
        title: v["title"].as_str().unwrap_or("").into(),
        text: v["text"].as_str().unwrap_or("").into(),
        time: ago(v["when"].as_u64().unwrap_or(0)).into(),
        call: v["call"] == true,
        incoming: v["incoming"] == true,
        reply_index: reply.and_then(|r| r["index"].as_i64()).map_or(-1, |i| i as i32),
        reply_hint: reply.and_then(|r| r["hint"].as_str()).unwrap_or("").into(),
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
            let items: Vec<FileItem> = p
                .rows
                .iter()
                .zip(&p.sel)
                .map(|(r, &sel)| FileItem {
                    name: r.name.clone().into(),
                    meta: if r.dir { String::new() } else { format!("{}  {}", fmt_size(r.size), fmt_date(r.mtime)) }.into(),
                    dir: r.dir,
                    selected: sel,
                })
                .collect();
            let count = p.sel.iter().filter(|s| **s).count();
            let footer = if count > 0 { format!("{count} seleccionado(s)") } else { format!("{} elementos", p.rows.len()) };
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
                        f.pc.rows = l.items.iter().filter(|e| !e.name.starts_with('.')).map(|e| Row { name: e.name.clone(), dir: e.dir, size: e.size, mtime: e.mtime }).collect();
                        f.pc.sel = vec![false; f.pc.rows.len()];
                        f.pc.path = l.path;
                        f.pc.parent = l.parent;
                    }
                    me.apply_files(false);
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
                        f.phone.sel = vec![false; rows.len()];
                        f.phone.rows = rows;
                        f.phone.parent = if p.is_empty() { None } else { Some(p.rfind('/').map_or(String::new(), |i| p[..i].to_string())) };
                        f.phone.path = p;
                    }
                    me.apply_files(true);
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
        let hint = if st["granted"] == false { "El móvil no ha dado permiso para leer notificaciones." } else { "" };
        self.ui(move |w| {
            w.set_notifs(model(items.iter().map(notif_of).collect()));
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
        let items: Vec<Notif> = self.core.popup_items().iter().take(4).map(notif_of).collect();
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
            let running = crate::appwin::running(&me.core.app_windows);
            let items: Vec<AppItem> = me
                .apps
                .lock()
                .unwrap()
                .iter()
                .filter(|a| show_sys || !a.system)
                .map(|a| AppItem { pkg: a.pkg.clone().into(), label: a.label.clone().into(), system: a.system, open: running.contains(&a.pkg) })
                .collect();
            w.set_apps(model(items));
            w.set_app_selected(-1);
        });
    }

    // ---------------------------------------------------------------- mensajería (Edge aislado)

    fn services(&self) -> Vec<ServiceItem> {
        crate::services::SERVICES
            .iter()
            .map(|s| ServiceItem { id: s.id.into(), name: s.name.into(), has_session: self.core.dir.join("web-edge").join(s.id).exists() })
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
        let Some(s) = crate::services::SERVICES.get(idx) else { return };
        if self.core.is_locked() {
            return;
        }
        {
            let mut map = self.edge.lock().unwrap();
            if let Some(ch) = map.get_mut(s.id) {
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
        let profile = self.core.dir.join("web-edge").join(s.id);
        let _ = std::fs::create_dir_all(&profile);
        match std::process::Command::new(exe)
            .arg(format!("--app={}", s.url))
            .arg(format!("--user-data-dir={}", profile.display()))
            .args(["--no-first-run", "--no-default-browser-check", "--disable-sync"])
            .spawn()
        {
            Ok(ch) => {
                self.edge.lock().unwrap().insert(s.id.to_string(), ch);
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
        let Some(s) = crate::services::SERVICES.get(idx) else { return };
        if let Some(mut c) = self.edge.lock().unwrap().remove(s.id) {
            Self::kill_tree(&mut c);
        }
        let _ = std::fs::remove_dir_all(self.core.dir.join("web-edge").join(s.id));
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
            if c.apps.lock().unwrap().is_empty() {
                c.load_apps();
            }
        }
        3 => {
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
            if let Some(s) = p.sel.get_mut(i as usize) {
                *s = !*s;
            }
        }
        c.apply_files(phone);
    });
    let c = ctx.clone();
    win.on_pane_open(move |phone, i| {
        let target = {
            let f = c.files.lock().unwrap();
            let p = if phone { &f.phone } else { &f.pc };
            p.rows.get(i as usize).filter(|r| r.dir).map(|r| if phone { join_phone(&p.path, &r.name) } else { join_pc(&p.path, &r.name) })
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
        if c.apps.lock().unwrap().is_empty() { c.load_apps() } else { c.apply_apps(false) }
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
