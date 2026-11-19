//! "Compartir con el PC": lo que el usuario comparte desde cualquier app del móvil llega aquí.
//! El texto aparece como notificación con botón Copiar; los archivos se guardan en Descargas\SyncroUmbral.

use crate::core::Core;
use crate::transfer::{is_risky, mark_of_the_web, safe_name, unique_name};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use crate::ui::Ui;

const MAX_TEXT: usize = 100_000;
/// Límites de lo que el móvil puede dejar en el PC sin que nadie lo pida desde el PC.
const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;
const MAX_SESSION: u64 = 4 * 1024 * 1024 * 1024;
const MAX_OPEN: usize = 4;
const PART: &str = ".umbral-part";

/// Bytes anunciados por el móvil en esta sesión (se reinicia al irse el móvil o bloquearse).
static SESSION_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub struct Incoming {
    file: File,
    path: PathBuf,
    final_path: PathBuf,
    name: String,
    size: u64,
    written: u64,
}

/// Carpeta donde se guardan los archivos recibidos.
pub fn inbox_dir() -> PathBuf {
    // Solo en compilaciones de depuración (pruebas): en la versión instalada el entorno no puede redirigir lo recibido.
    #[cfg(debug_assertions)]
    if let Some(p) = std::env::var_os("UMBRAL_INBOX_DIR") {
        return PathBuf::from(p);
    }
    PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into())).join("Downloads").join("SyncroUmbral")
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn item(key: String, title: &str, text: String, extra: Value) -> Value {
    let mut v = json!({
        "key": key, "pkg": "umbral.share", "app": "Compartido desde el móvil", "title": title, "text": text,
        "sub": "", "when": now_ms(), "quiet": false, "clearable": false, "actions": [],
    });
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        o.extend(e.clone());
    }
    v
}

fn fmt_size(n: u64) -> String {
    let units = ["B", "KB", "MB", "GB"];
    let (mut x, mut i) = (n as f64, 0);
    while x >= 1024.0 && i < units.len() - 1 {
        x /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", x, units[i])
}

pub fn handle<H: Ui>(core: &Core, h: &H, kind: &str, v: &Value) {
    match kind {
        "share_text" => {
            let text: String = v["text"].as_str().unwrap_or("").chars().take(MAX_TEXT).collect();
            if !text.trim().is_empty() {
                core.push_notif(h, &item(format!("share-{}", now_ms()), "Texto", text, json!({"kind": "text"})));
            }
        }
        "share_begin" => begin(core, h, v),
        "share_chunk" => chunk(core, v),
        "share_end" => end(core, h, v),
        _ => {}
    }
}

fn begin<H: Ui>(core: &Core, h: &H, v: &Value) {
    let (Some(id), Some(size)) = (v["id"].as_u64(), v["size"].as_u64()) else { return };
    if size > MAX_FILE {
        core.push_notif(h, &item(format!("share-{id}"), "Archivo demasiado grande", "El límite es 2 GB.".into(), json!({})));
        return;
    }
    if SESSION_BYTES.load(std::sync::atomic::Ordering::SeqCst).saturating_add(size) > MAX_SESSION {
        core.push_notif(
            h,
            &item(format!("share-{id}"), "Límite de recepción alcanzado", "El móvil ya envió 4 GB en esta sesión.".into(), json!({})),
        );
        return;
    }
    let mut open = core.shares.lock().unwrap();
    if open.len() >= MAX_OPEN {
        return;
    }
    let dir = inbox_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    SESSION_BYTES.fetch_add(size, std::sync::atomic::Ordering::SeqCst);
    let mut safe = safe_name(v["name"].as_str().unwrap_or("archivo"));
    if is_risky(&safe) {
        safe.push_str(".recibido"); // un ejecutable o acceso directo no debe poder lanzarse por descuido
    }
    let name = unique_name(&safe, &|n| dir.join(n).exists());
    let final_path = dir.join(&name);
    let path = dir.join(format!("{name}{PART}"));
    if let Ok(file) = File::create(&path) {
        open.insert(id, Incoming { file, path, final_path, name, size, written: 0 });
    }
}

fn chunk(core: &Core, v: &Value) {
    let Some(id) = v["id"].as_u64() else { return };
    let Ok(data) = B64.decode(v["data"].as_str().unwrap_or("")) else { return };
    let mut open = core.shares.lock().unwrap();
    let failed = match open.get_mut(&id) {
        Some(inc) if inc.written + data.len() as u64 <= inc.size => {
            inc.written += data.len() as u64;
            inc.file.write_all(&data).is_err()
        }
        Some(_) => true, // más datos de los anunciados
        None => return,
    };
    if failed {
        if let Some(inc) = open.remove(&id) {
            drop(inc.file);
            let _ = std::fs::remove_file(&inc.path);
        }
    }
}

fn end<H: Ui>(core: &Core, h: &H, v: &Value) {
    let Some(id) = v["id"].as_u64() else { return };
    let Some(inc) = core.shares.lock().unwrap().remove(&id) else { return };
    let Incoming { mut file, path, final_path, name, size, written } = inc;
    let _ = file.flush();
    drop(file);
    // Un archivo incompleto nunca se deja con su nombre final.
    if written != size || std::fs::rename(&path, &final_path).is_err() {
        let _ = std::fs::remove_file(&path);
        core.push_notif(h, &item(format!("share-{id}"), &name, format!("No se recibió completo ({written} de {size} bytes)."), json!({})));
        return;
    }
    mark_of_the_web(&final_path);
    core.push_notif(
        h,
        &item(
            format!("share-{id}"),
            &name,
            format!("{} guardado en Descargas\\SyncroUmbral", fmt_size(size)),
            json!({"kind": "file", "path": final_path.to_string_lossy()}),
        ),
    );
}

/// Descarta los archivos a medias (al bloquear o irse el móvil).
pub fn abort_all(core: &Core) {
    SESSION_BYTES.store(0, std::sync::atomic::Ordering::SeqCst);
    for (_, inc) in core.shares.lock().unwrap().drain() {
        drop(inc.file);
        let _ = std::fs::remove_file(&inc.path);
    }
}
