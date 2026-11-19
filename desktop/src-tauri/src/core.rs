//! Estado compartido: presencia del móvil, bloqueo, dispositivos vinculados y limpieza.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use crate::vault::{Sealed, Vault};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use crate::ui::Ui;

pub const PORT: u16 = 47600;
pub const HB_INTERVAL_MS: u64 = 2000;
pub const HB_TIMEOUT: Duration = Duration::from_secs(10);
const TOKEN_TTL: Duration = Duration::from_secs(300);
const MAX_FAILS: u32 = 5;

#[derive(Serialize, Deserialize, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    /// Clave pública P-256 (SPKI DER) en base64.
    pub pubkey: String,
}

struct PairToken {
    tok: String,
    expires: Instant,
}

struct Inner {
    devices: Vec<Device>,
    password_hash: Option<String>,
    present: bool,
    /// Desbloqueo con clave: vale aunque el móvil no esté.
    manual: bool,
    /// Bloqueo pedido por el usuario: gana sobre la presencia hasta desbloquear con clave o reconectar el móvil.
    forced: bool,
    was_locked: bool,
    active_session: Option<u64>,
    next_session: u64,
    device_name: Option<String>,
    token: Option<PairToken>,
    fails: u32,
    lockout_until: Option<Instant>,
    battery: Option<u8>,
    charging: bool,
    /// El móvil avisa de que Android puede cortar la conexión (ahorro de batería o SyncroUmbral optimizado).
    saver: bool,
    optimized: bool,
    /// Cuándo empezó la sesión actual del móvil (para registrar cuánto duró).
    session_start: Option<Instant>,
}

/// Comparación en tiempo constante.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    let mut d = (a.len() ^ b.len()) as u8;
    for i in 0..a.len().min(b.len()) {
        d |= a[i] ^ b[i];
    }
    d == 0
}

/// Inserta o reemplaza por `key`, dejando lo más reciente primero y limitando el tamaño.
fn upsert(list: &mut Vec<Value>, item: &Value, max: usize) {
    list.retain(|x| x["key"] != item["key"]);
    list.insert(0, item.clone());
    list.truncate(max);
}

impl Inner {
    fn locked(&self) -> bool {
        self.forced || !(self.present || self.manual)
    }
}

#[derive(Serialize, Clone)]
pub struct Snapshot {
    pub locked: bool,
    pub present: bool,
    pub device: Option<String>,
    pub has_password: bool,
    pub devices: Vec<DeviceInfo>,
    pub lockout_secs: u64,
    pub battery: Option<u8>,
    pub charging: bool,
    /// Aviso de estabilidad de la conexión (ahorro de batería del móvil), si procede.
    pub warn: Option<&'static str>,
}

#[derive(Serialize, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

/// Petición del PC al móvil, que viaja por la sesión autenticada.
pub struct PhoneCmd {
    pub op: String,
    pub args: Value,
    pub reply: oneshot::Sender<Result<Value, String>>,
}

/// Marcador interno de "archivo manipulado" (no es un valor que pueda venir de un archivo real).
const TAMPERED: &str = "!manipulado";

pub struct Core {
    vault: Vault,
    pub dir: PathBuf,
    pub tls_fp: String,
    /// Sube cada vez que la app se bloquea; las transferencias en curso la comparan y se cancelan.
    pub epoch: AtomicU64,
    next_job: AtomicU64,
    phone_tx: Mutex<Option<(u64, mpsc::Sender<PhoneCmd>)>>,
    /// IP desde la que se conectó el móvil (sirve para localizarlo por adb).
    phone_ip: Mutex<Option<String>>,
    /// Notificaciones del móvil (solo en RAM) y las que están en la ventana emergente.
    notifs: Mutex<Vec<Value>>,
    popups: Mutex<Vec<Value>>,
    notif_granted: Mutex<Option<bool>>,
    /// Pedir al móvil su lista de notificaciones al conectarse.
    pub sync_notifs: bool,
    /// Apps del móvil abiertas en ventanas del PC (procesos scrcpy).
    pub app_windows: crate::appwin::Windows,
    /// ¿Está sonando el timbre de una llamada entrante?
    ringing: Arc<AtomicBool>,
    /// Archivos que el móvil está enviando al PC ("Compartir con el PC").
    pub shares: Mutex<HashMap<u64, crate::share::Incoming>>,
    /// Apps cuyas notificaciones se ignoran: paquete a nombre visible.
    muted: Mutex<std::collections::BTreeMap<String, String>>,
    /// Tiempo sin latidos tras el cual el móvil se da por ausente.
    pub hb_timeout: Duration,
    /// Límites de conexiones sin autenticar y bloqueo por intentos fallidos.
    pub limits: crate::limits::Limiter,
    /// Silenciar el resto de sonidos del PC mientras suena una llamada (ajuste del usuario, activado por defecto).
    mute_on_call: AtomicBool,
    /// Modo avanzado (usa adb y scrcpy: abrir apps del móvil en el PC, detenerlas, desinstalarlas). Apagado = modo básico.
    advanced: AtomicBool,
    inner: Mutex<Inner>,
    /// Aquí viven los datos sensibles (listados, miniaturas…). Solo en RAM; se vacía al bloquear.
    pub volatile: Mutex<HashMap<String, Vec<u8>>>,
}

impl Core {
    pub fn load(dir: PathBuf, tls_fp: String) -> Self {
        let vault = Vault::open(&dir);
        // Cada archivo de configuración lleva sello: si fue editado a mano se ignora y se deja constancia.
        let read = |name: &str| match vault.read_sealed(&dir.join(name)) {
            Sealed::Ok(s) => Some(s),
            Sealed::Missing => None,
            Sealed::Tampered => {
                crate::vault::log(&dir, &format!("MANIPULADO {name}: el sello no coincide, se ignora el archivo"));
                Some(TAMPERED.to_string())
            }
        };
        let devices = match read("devices.json") {
            Some(s) if s != TAMPERED => serde_json::from_str(&s).unwrap_or_default(),
            _ => Vec::new(), // manipulado: ningún móvil queda autorizado
        };
        // Con la clave manipulada no se deja "sin clave" (permitiría fijar una nueva): queda un valor que nunca verifica.
        let password_hash = read("password.phc").map(|s| if s == TAMPERED { TAMPERED.to_string() } else { s.trim().to_string() });
        let muted = match read("muted.json") {
            Some(s) if s != TAMPERED => serde_json::from_str(&s).unwrap_or_default(),
            _ => Default::default(),
        };
        let settings: Value = match read("settings.json") {
            Some(s) if s != TAMPERED => serde_json::from_str::<Value>(&s).unwrap_or(Value::Null),
            _ => Value::Null,
        };
        let mute_on_call = settings["mute_on_call"].as_bool().unwrap_or(true);
        let advanced = settings["advanced"].as_bool().unwrap_or(true);
        Core {
            mute_on_call: AtomicBool::new(mute_on_call),
            advanced: AtomicBool::new(advanced),
            vault,
            dir,
            tls_fp,
            epoch: AtomicU64::new(0),
            next_job: AtomicU64::new(1),
            phone_tx: Mutex::new(None),
            phone_ip: Mutex::new(None),
            notifs: Mutex::new(Vec::new()),
            popups: Mutex::new(Vec::new()),
            notif_granted: Mutex::new(None),
            sync_notifs: false,
            app_windows: Mutex::new(HashMap::new()),
            ringing: Arc::new(AtomicBool::new(false)),
            shares: Mutex::new(HashMap::new()),
            muted: Mutex::new(muted),
            hb_timeout: HB_TIMEOUT,
            limits: Default::default(),
            inner: Mutex::new(Inner {
                devices,
                password_hash,
                present: false,
                manual: false,
                forced: false,
                was_locked: true,
                active_session: None,
                next_session: 1,
                device_name: None,
                token: None,
                fails: 0,
                lockout_until: None,
                battery: None,
                charging: false,
                saver: false,
                optimized: false,
                session_start: None,
            }),
            volatile: Mutex::new(HashMap::new()),
        }
    }

    fn snapshot_of(i: &Inner) -> Snapshot {
        Snapshot {
            locked: i.locked(),
            present: i.present,
            device: i.device_name.clone(),
            has_password: i.password_hash.is_some(),
            devices: i.devices.iter().map(|d| DeviceInfo { id: d.id.clone(), name: d.name.clone() }).collect(),
            lockout_secs: i
                .lockout_until
                .map(|t| t.saturating_duration_since(Instant::now()).as_secs() + 1)
                .filter(|_| i.lockout_until.map_or(false, |t| t > Instant::now()))
                .unwrap_or(0),
            battery: i.battery,
            charging: i.charging,
            warn: if !i.present {
                None
            } else if i.saver {
                Some("Ahorro de batería activo: puede cortar la conexión")
            } else if i.optimized {
                Some("SyncroUmbral optimizado por la batería: puede desconectarse")
            } else {
                None
            },
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Self::snapshot_of(&self.inner.lock().unwrap())
    }

    /// Recalcula el bloqueo, limpia si acaba de bloquearse y avisa a la interfaz.
    pub fn notify<H: Ui>(&self, h: &H) {
        let (changed_to_locked, snap) = {
            let mut i = self.inner.lock().unwrap();
            let locked = i.locked();
            let changed = locked && !i.was_locked;
            i.was_locked = locked;
            (changed, Self::snapshot_of(&i))
        };
        if changed_to_locked {
            self.wipe(h);
        }
        let _ = h.emit("state", snap);
    }

    /// Borra todo rastro de lo que se estaba viendo.
    fn wipe<H: Ui>(&self, h: &H) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        self.volatile.lock().unwrap().clear();
        self.clear_notifs(h);
        crate::share::abort_all(self);
        crate::appwin::close_all(&self.app_windows); // las apps del móvil abiertas en el PC se cierran
        h.close_web(); // las ventanas web también (la sesión guardada se conserva)
        let tmp = std::env::temp_dir().join("umbral");
        let _ = std::fs::remove_dir_all(&tmp);
        // Ya no se borran los datos del navegador al bloquear: ocultar lo que se ve lo hace la pantalla de bloqueo, y las
        // sesiones de los servicios web deben conservarse.
    }

    // ---------- sesiones del móvil ----------

    pub fn begin_session<H: Ui>(&self, h: &H, device_name: String) -> u64 {
        let sid = {
            let mut i = self.inner.lock().unwrap();
            let sid = i.next_session;
            i.next_session += 1;
            i.active_session = Some(sid);
            i.present = true;
            // "Bloquear ahora" se mantiene aunque el móvil parpadee y se reconecte: solo la clave lo levanta. Sin clave
            // configurada no habría forma de abrir de nuevo, así que ahí sí lo levanta la reconexión.
            if i.password_hash.is_none() {
                i.forced = false;
            }
            // Desde que el móvil está presente manda la presencia: si luego se va, se bloquea
            // aunque antes se hubiera abierto con clave.
            i.manual = false;
            i.device_name = Some(device_name);
            i.session_start = Some(Instant::now());
            sid
        };
        // Se deja constancia de cada conexión y desconexión: sirve para ver si el móvil se está cayendo y cada cuánto.
        self.seclog("sesión del móvil iniciada");
        self.notify(h);
        sid
    }

    pub fn end_session<H: Ui>(&self, h: &H, sid: u64) {
        let secs;
        {
            let mut i = self.inner.lock().unwrap();
            // Si el móvil ya se reconectó con otra sesión, esta no cuenta.
            if i.active_session != Some(sid) {
                return;
            }
            secs = i.session_start.take().map_or(0, |t| t.elapsed().as_secs());
            i.active_session = None;
            i.present = false;
            i.device_name = None;
            i.battery = None;
            i.charging = false;
            i.saver = false;
            i.optimized = false;
        }
        self.seclog(&format!("sesión del móvil terminada tras {secs} s"));
        crate::share::abort_all(self); // archivos recibidos a medias
        self.clear_notifs(h); // sin móvil no hay notificaciones que mostrar
        self.notify(h);
    }

    // ---------- notificaciones del móvil ----------

    /// Estado de las notificaciones para la interfaz: si el móvil dio acceso y la lista actual.
    pub fn notifs_state(&self) -> Value {
        serde_json::json!({
            "granted": *self.notif_granted.lock().unwrap(),
            "items": *self.notifs.lock().unwrap(),
        })
    }

    pub fn popup_items(&self) -> Vec<Value> {
        self.popups.lock().unwrap().clone()
    }

    /// Añade o actualiza una notificación (del móvil o generada aquí) y abre la ventana emergente si no es silenciosa.
    pub fn push_notif<H: Ui>(&self, h: &H, v: &Value) {
        if v["key"].as_str().is_none() {
            return;
        }
        // Constancia de llamadas entrantes (solo la aplicación de origen, nunca el nombre de quien llama).
        let already = self.notifs.lock().unwrap().iter().any(|x| x["key"] == v["key"] && x["incoming"] == true);
        if v["incoming"] == true && !already {
            self.seclog(&format!("llamada entrante detectada ({})", v["pkg"].as_str().unwrap_or("?")));
        }
        upsert(&mut self.notifs.lock().unwrap(), v, 100);
        let popup = v["quiet"] != true;
        if popup {
            upsert(&mut self.popups.lock().unwrap(), v, 5);
        }
        let _ = h.emit("notif", serde_json::json!({ "n": v, "popup": popup }));
        if popup {
            h.toast_show();
        }
        if v["incoming"] == true {
            crate::ring::start(&self.ringing, self.mute_on_call());
        }
        self.stop_ring_if_no_incoming_call();
    }

    /// El timbre se corta cuando ya no queda ninguna llamada entrante (contestada, rechazada o colgada).
    fn stop_ring_if_no_incoming_call(&self) {
        if !self.notifs.lock().unwrap().iter().any(|n| n["incoming"] == true) {
            crate::ring::stop(&self.ringing);
        }
    }

    #[cfg(test)]
    pub fn is_ringing(&self) -> bool {
        self.ringing.load(Ordering::SeqCst)
    }

    /// Quita una notificación solo del PC (las generadas aquí, como lo compartido desde el móvil).
    pub fn remove_local<H: Ui>(&self, h: &H, key: &str) {
        self.notifs.lock().unwrap().retain(|x| x["key"] != key);
        let empty = {
            let mut p = self.popups.lock().unwrap();
            p.retain(|x| x["key"] != key);
            p.is_empty()
        };
        let _ = h.emit("notif_gone", serde_json::json!({ "key": key }));
        if empty {
            h.toast_hide();
        }
    }

    /// Quita del PC todo lo compartido desde el móvil (texto y archivos recibidos).
    pub fn remove_shared<H: Ui>(&self, h: &H) {
        let keys: Vec<String> = self
            .notifs
            .lock()
            .unwrap()
            .iter()
            .filter(|n| n["pkg"] == "umbral.share")
            .filter_map(|n| n["key"].as_str().map(String::from))
            .collect();
        for k in keys {
            self.remove_local(h, &k);
        }
    }

    /// Mensaje espontáneo del móvil. Con la app bloqueada se ignora: no se muestra ni se guarda nada.
    pub fn on_phone_event<H: Ui>(&self, h: &H, kind: &str, v: &Value) {
        // Botón "Desbloquear" del móvil (notificación o app): el móvil autenticado y presente es la llave. Va antes de la
        // comprobación de bloqueo porque su función es justo abrir. En el móvil la acción exige tener el teléfono desbloqueado.
        if kind == "unlock_pc" {
            self.seclog("desbloqueo pedido desde el móvil");
            self.unlock_from_phone();
            self.notify(h);
            return;
        }
        if self.is_locked() {
            return;
        }
        match kind {
            "notif" => {
                if let Some(pkg) = v["pkg"].as_str() {
                    if self.muted.lock().unwrap().contains_key(pkg) {
                        return;
                    }
                }
                self.push_notif(h, v);
            }
            "status" => {
                {
                    let mut i = self.inner.lock().unwrap();
                    i.battery = v["battery"].as_u64().map(|b| b.min(100) as u8);
                    i.charging = v["charging"].as_bool().unwrap_or(false);
                    i.saver = v["saver"].as_bool().unwrap_or(false);
                    i.optimized = v["optimized"].as_bool().unwrap_or(false);
                }
                let _ = h.emit("state", self.snapshot());
            }
            k if k.starts_with("share_") => crate::share::handle(self, h, k, v),
            // Botón "Bloquear PC ahora" de la app del móvil.
            "lock_pc" => {
                self.seclog("bloqueo pedido desde el móvil");
                self.lock_now();
                self.notify(h);
            }
            // El usuario cambió el modo (básico/avanzado) en el móvil.
            "mode" => {
                if let Some(on) = v["advanced"].as_bool() {
                    self.set_advanced(on);
                    let _ = h.emit("mode", serde_json::json!({ "advanced": on }));
                }
            }
            "notif_gone" => {
                let key = v["key"].clone();
                if self.notifs.lock().unwrap().iter().any(|x| x["key"] == key && x["incoming"] == true) {
                    // `reason` es el motivo que da Android al retirar la notificación (1 pulsada, 2 descartada, 8 retirada por la app...).
                    self.seclog(&format!(
                        "llamada entrante terminada (contestada, rechazada o colgada), motivo Android {}",
                        v["reason"].as_i64().map_or("?".to_string(), |r| r.to_string())
                    ));
                }
                self.notifs.lock().unwrap().retain(|x| x["key"] != key);
                let empty = {
                    let mut p = self.popups.lock().unwrap();
                    p.retain(|x| x["key"] != key);
                    p.is_empty()
                };
                let _ = h.emit("notif_gone", serde_json::json!({ "key": key }));
                self.stop_ring_if_no_incoming_call();
                if empty {
                    h.toast_hide();
                }
            }
            _ => {}
        }
    }

    /// Lista inicial que devuelve el móvil al conectarse (sin ventanas emergentes).
    pub fn set_notifs<H: Ui>(&self, h: &H, r: &Value) {
        if self.is_locked() {
            return;
        }
        *self.notif_granted.lock().unwrap() = r["granted"].as_bool();
        let muted = self.muted.lock().unwrap().clone();
        let local: Vec<Value> = self.notifs.lock().unwrap().iter().filter(|n| n["pkg"] == "umbral.share").cloned().collect();
        let mut all: Vec<Value> = r["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|n| n["pkg"].as_str().map_or(true, |p| !muted.contains_key(p)))
            .collect();
        all.extend(local);
        *self.notifs.lock().unwrap() = all;
        let _ = h.emit("notif_sync", self.notifs_state());
        // Una llamada que sigue sonando al (re)conectar el móvil hay que mostrarla y hacerla sonar: si el móvil parpadeó mientras
        // sonaba, la ventana y el timbre se habían cerrado y la lista inicial no los reabre.
        let ringing: Vec<Value> = self.notifs.lock().unwrap().iter().filter(|n| n["incoming"] == true).cloned().collect();
        for n in ringing {
            self.push_notif(h, &n);
        }
    }

    /// Sincronización periódica con el móvil: lo que el usuario quitó en el móvil desaparece del PC y lo que falta aparece.
    /// Conserva lo generado en el PC (lo compartido desde el móvil) y no abre ventanas emergentes, salvo para una llamada
    /// entrante que el PC no conocía.
    pub fn resync_notifs<H: Ui>(&self, h: &H, r: &Value) {
        if self.is_locked() || r["items"].as_array().is_none() {
            return;
        }
        let muted = self.muted.lock().unwrap().clone();
        let fresh: Vec<Value> = r["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|n| n["pkg"].as_str().map_or(true, |p| !muted.contains_key(p)))
            .collect();
        let (known, local): (Vec<String>, Vec<Value>) = {
            let cur = self.notifs.lock().unwrap();
            (
                cur.iter().filter_map(|n| n["key"].as_str().map(String::from)).collect(),
                cur.iter().filter(|n| n["pkg"] == "umbral.share").cloned().collect(),
            )
        };
        let same = {
            let cur = self.notifs.lock().unwrap();
            let keys = |v: &Vec<Value>| -> Vec<String> {
                let mut k: Vec<String> = v.iter().filter_map(|n| n["key"].as_str().map(String::from)).collect();
                k.sort();
                k
            };
            keys(&cur.iter().filter(|n| n["pkg"] != "umbral.share").cloned().collect()) == keys(&fresh)
        };
        if same {
            return;
        }
        let mut all = fresh.clone();
        all.extend(local);
        *self.notifs.lock().unwrap() = all;
        {
            let live: Vec<&str> = fresh.iter().filter_map(|n| n["key"].as_str()).collect();
            let mut p = self.popups.lock().unwrap();
            p.retain(|x| x["pkg"] == "umbral.share" || x["key"].as_str().map_or(false, |k| live.contains(&k)));
            if p.is_empty() {
                drop(p);
                h.toast_hide();
            }
        }
        let _ = h.emit("notif_sync", self.notifs_state());
        for n in fresh.iter().filter(|n| n["incoming"] == true && !known.iter().any(|k| Some(k.as_str()) == n["key"].as_str())) {
            self.push_notif(h, n);
        }
        self.stop_ring_if_no_incoming_call();
    }

    // ---------- ajustes ----------

    pub fn mute_on_call(&self) -> bool {
        self.mute_on_call.load(Ordering::SeqCst)
    }

    pub fn set_mute_on_call(&self, on: bool) {
        self.mute_on_call.store(on, Ordering::SeqCst);
        self.save_settings();
    }

    pub fn advanced(&self) -> bool {
        self.advanced.load(Ordering::SeqCst)
    }

    pub fn set_advanced(&self, on: bool) {
        self.advanced.store(on, Ordering::SeqCst);
        self.save_settings();
    }

    fn save_settings(&self) {
        let v = serde_json::json!({ "mute_on_call": self.mute_on_call(), "advanced": self.advanced() });
        self.vault.seal_write(&self.dir.join("settings.json"), &v.to_string());
    }

    // ---------- apps silenciadas ----------

    pub fn muted_list(&self) -> Vec<Value> {
        self.muted.lock().unwrap().iter().map(|(pkg, app)| serde_json::json!({"pkg": pkg, "app": app})).collect()
    }

    fn save_muted(&self, m: &std::collections::BTreeMap<String, String>) {
        if let Ok(s) = serde_json::to_string_pretty(m) {
            self.vault.seal_write(&self.dir.join("muted.json"), &s);
        }
    }

    /// Silencia una app: se quitan sus notificaciones actuales y no se muestran las futuras.
    pub fn mute_app<H: Ui>(&self, h: &H, pkg: &str, app: &str) {
        {
            let mut m = self.muted.lock().unwrap();
            m.insert(pkg.to_string(), app.chars().take(80).collect());
            self.save_muted(&m);
        }
        let gone: Vec<Value> = {
            let mut n = self.notifs.lock().unwrap();
            let (g, keep): (Vec<Value>, Vec<Value>) = n.drain(..).partition(|x| x["pkg"] == pkg);
            *n = keep;
            g
        };
        self.popups.lock().unwrap().retain(|x| x["pkg"] != pkg);
        for n in gone {
            let _ = h.emit("notif_gone", serde_json::json!({ "key": n["key"] }));
        }
    }

    pub fn unmute_app(&self, pkg: &str) {
        let mut m = self.muted.lock().unwrap();
        m.remove(pkg);
        self.save_muted(&m);
    }

    /// Borra todo lo de notificaciones y cierra la ventana emergente.
    pub fn clear_notifs<H: Ui>(&self, h: &H) {
        self.notifs.lock().unwrap().clear();
        self.popups.lock().unwrap().clear();
        crate::ring::stop(&self.ringing);
        *self.notif_granted.lock().unwrap() = None;
        let _ = h.emit("notif_clear", ());
        h.toast_hide();
    }

    pub fn clear_popups<H: Ui>(&self, h: &H) {
        self.popups.lock().unwrap().clear();
        h.toast_hide();
    }

    // ---------- peticiones al móvil ----------

    pub fn attach_phone(&self, sid: u64, tx: mpsc::Sender<PhoneCmd>, ip: Option<String>) {
        *self.phone_tx.lock().unwrap() = Some((sid, tx));
        *self.phone_ip.lock().unwrap() = ip;
    }

    pub fn detach_phone(&self, sid: u64) {
        let mut g = self.phone_tx.lock().unwrap();
        if g.as_ref().map(|(s, _)| *s) == Some(sid) {
            *g = None;
            *self.phone_ip.lock().unwrap() = None;
        }
    }

    /// Sesión del móvil conectado ahora (None si no hay).
    pub fn phone_session(&self) -> Option<u64> {
        self.phone_tx.lock().unwrap().as_ref().map(|(s, _)| *s)
    }

    pub fn phone_ip(&self) -> Option<String> {
        self.phone_ip.lock().unwrap().clone()
    }

    pub fn next_job_id(&self) -> u64 {
        self.next_job.fetch_add(1, Ordering::SeqCst)
    }

    /// Pide algo al móvil y espera la respuesta. Falla si la app está bloqueada o el móvil no está.
    pub async fn call(&self, op: &str, args: Value) -> Result<Value, String> {
        if self.is_locked() {
            return Err("La aplicación está bloqueada".into());
        }
        self.send_to_phone(op, args).await
    }

    /// Operaciones de limpieza que deben poder ejecutarse aunque la app esté bloqueada: borrar un temporal propio ("abort")
    /// y soltar el bloqueo que mantiene despierto el móvil ("keep_awake" apagado).
    pub async fn cleanup_call(&self, op: &str, args: Value) {
        if matches!(op, "abort" | "keep_awake") {
            let _ = self.send_to_phone(op, args).await;
        }
    }

    /// Solo para limpiar un temporal propio en el móvil (op "abort"), incluso con la app bloqueada.
    pub async fn abort_partial(&self, path: &str) {
        let _ = self.send_to_phone("abort", serde_json::json!({ "path": path })).await;
    }

    async fn send_to_phone(&self, op: &str, args: Value) -> Result<Value, String> {
        let tx = self
            .phone_tx
            .lock()
            .unwrap()
            .as_ref()
            .map(|(_, t)| t.clone())
            .ok_or("El móvil no está conectado")?;
        let (reply, rx) = oneshot::channel();
        tx.send(PhoneCmd { op: op.to_string(), args, reply })
            .await
            .map_err(|_| "El móvil se desconectó".to_string())?;
        match tokio::time::timeout(Duration::from_secs(60), rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err("El móvil se desconectó".into()),
            Err(_) => Err("El móvil no respondió".into()),
        }
    }

    // ---------- dispositivos y vinculación ----------

    pub fn find_device(&self, id: &str) -> Option<Device> {
        self.inner.lock().unwrap().devices.iter().find(|d| d.id == id).cloned()
    }

    pub fn new_token(&self) -> String {
        use rand::RngCore;
        let mut b = [0u8; 16];
        OsRng.fill_bytes(&mut b);
        let tok = hex::encode(b);
        self.inner.lock().unwrap().token = Some(PairToken { tok: tok.clone(), expires: Instant::now() + TOKEN_TTL });
        tok
    }

    /// Consume el token (un solo uso) solo si coincide y no ha caducado. Un código equivocado NO gasta el bueno
    /// (si no, cualquiera en la red podría impedir la vinculación); el límite de intentos por origen lo pone el servidor.
    pub fn take_token(&self, tok: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        let ok = match &i.token {
            Some(t) => t.expires > Instant::now() && ct_eq(t.tok.as_bytes(), tok.as_bytes()),
            None => false,
        };
        if ok {
            i.token = None;
        }
        ok
    }

    /// Deja constancia de un evento de seguridad en `security.log` (sin datos sensibles).
    pub fn seclog(&self, event: &str) {
        crate::vault::log(&self.dir, event);
    }

    pub fn add_device(&self, d: Device) {
        self.seclog(&format!("móvil vinculado: {} ({})", d.name, &d.id[..12.min(d.id.len())]));
        let mut i = self.inner.lock().unwrap();
        i.devices.retain(|x| x.id != d.id);
        i.devices.push(d);
        self.save_devices(&i.devices);
    }

    pub fn remove_device(&self, id: &str) {
        self.seclog(&format!("móvil desvinculado: {}", &id[..12.min(id.len())]));
        let mut i = self.inner.lock().unwrap();
        i.devices.retain(|x| x.id != id);
        self.save_devices(&i.devices);
    }

    fn save_devices(&self, devices: &[Device]) {
        if let Ok(s) = serde_json::to_string_pretty(devices) {
            self.vault.seal_write(&self.dir.join("devices.json"), &s);
        }
    }

    // ---------- clave de respaldo ----------

    pub fn set_password(&self, pw: &str) -> Result<(), String> {
        if pw.chars().count() < 6 {
            return Err("La clave debe tener al menos 6 caracteres".into());
        }
        let mut i = self.inner.lock().unwrap();
        if i.password_hash.is_some() {
            return Err("Ya existe una clave".into());
        }
        let salt = SaltString::generate(&mut OsRng);
        let phc = Argon2::default().hash_password(pw.as_bytes(), &salt).map_err(|e| e.to_string())?.to_string();
        self.vault.seal_write(&self.dir.join("password.phc"), &phc);
        self.seclog("clave de respaldo creada");
        i.password_hash = Some(phc);
        // Con el móvil presente manda la presencia; sin él, la sesión queda abierta con clave.
        i.manual = !i.present;
        Ok(())
    }

    pub fn unlock_with_password(&self, pw: &str) -> Result<(), String> {
        let mut i = self.inner.lock().unwrap();
        if let Some(until) = i.lockout_until {
            if until > Instant::now() {
                return Err(format!("Demasiados intentos. Espera {} s", (until - Instant::now()).as_secs() + 1));
            }
        }
        let phc = i.password_hash.clone().ok_or("No hay clave configurada")?;
        let ok = PasswordHash::new(&phc)
            .map(|p| Argon2::default().verify_password(pw.as_bytes(), &p).is_ok())
            .unwrap_or(false);
        if ok {
            i.fails = 0;
            i.lockout_until = None;
            i.manual = !i.present;
            i.forced = false;
            self.seclog("desbloqueo con clave");
            Ok(())
        } else {
            i.fails += 1;
            self.seclog(&format!("clave incorrecta (intento {})", i.fails));
            if i.fails >= MAX_FAILS {
                let extra = 30u64 * (1u64 << (i.fails - MAX_FAILS).min(6));
                i.lockout_until = Some(Instant::now() + Duration::from_secs(extra));
                return Err(format!("Demasiados intentos. Espera {} s", extra));
            }
            Err(format!("Clave incorrecta ({} de {})", i.fails, MAX_FAILS))
        }
    }

    pub fn lock_now(&self) {
        let mut i = self.inner.lock().unwrap();
        i.manual = false;
        i.forced = true;
    }

    /// Levanta un bloqueo pedido por el usuario. Solo se llama desde una sesión autenticada del móvil, es decir, con el móvil presente.
    pub fn unlock_from_phone(&self) {
        let mut i = self.inner.lock().unwrap();
        if i.present {
            i.forced = false;
        }
    }

    pub fn is_locked(&self) -> bool {
        self.inner.lock().unwrap().locked()
    }
}
