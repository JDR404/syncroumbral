//! Copia de archivos y carpetas entre el PC y el móvil, por trozos, con progreso y cancelación.

use crate::core::Core;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use crate::ui::Ui;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Borra el archivo temporal si la transferencia no llega a terminar (error, cancelación o futuro descartado).
struct PartGuard {
    path: PathBuf,
    armed: bool,
}

impl Drop for PartGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

const CHUNK: usize = 1024 * 1024;
const PART: &str = ".umbral-part";

#[derive(Serialize, Deserialize, Clone)]
pub struct Item {
    pub path: String,
    pub name: String,
    pub dir: bool,
    pub size: u64,
}

#[derive(Serialize, Clone)]
struct Progress {
    job: u64,
    label: String,
    done: u64,
    total: u64,
    state: &'static str,
    msg: String,
}

struct FileTask {
    src: String,
    dst: String,
    size: u64,
}

/// Ejecuta `f` sobre cada elemento con hasta PARALLEL a la vez; se detiene en el primer error.
async fn run_parallel<'a, T, Fut, F>(items: &'a [T], f: F) -> Result<(), String>
where
    F: Fn(&'a T) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let mut running = FuturesUnordered::new();
    let mut next = items.iter();
    loop {
        while running.len() < PARALLEL {
            match next.next() {
                Some(item) => running.push(f(item)),
                None => break,
            }
        }
        match running.next().await {
            Some(r) => r?,
            None => return Ok(()),
        }
    }
}

/// Archivos que se transfieren a la vez. Con muchos archivos pequeños el tiempo lo dominan las esperas, no el ancho de banda.
const PARALLEL: usize = 6;

struct Job<'a, H: Ui> {
    h: &'a H,
    core: &'a Arc<Core>,
    id: u64,
    epoch: u64,
    label: String,
    done: AtomicU64,
    total: AtomicU64,
}

impl<H: Ui> Job<'_, H> {
    fn emit(&self, state: &'static str, msg: &str) {
        let _ = self.h.emit(
            "transfer",
            Progress {
                job: self.id,
                label: self.label.clone(),
                done: self.done.load(Ordering::Relaxed),
                total: self.total.load(Ordering::Relaxed),
                state,
                msg: msg.to_string(),
            },
        );
    }

    fn cancelled(&self) -> bool {
        self.core.epoch.load(Ordering::SeqCst) != self.epoch || self.core.is_locked()
    }
}

/// Nombre válido en Windows: el móvil no es de confianza para decidir rutas del PC.
pub fn safe_name(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c })
        .collect();
    while s.ends_with('.') || s.ends_with(' ') {
        s.pop();
    }
    if s.is_empty() || s == "." || s == ".." {
        s = "_".into();
    }
    let stem = s.split('.').next().unwrap_or("").to_uppercase();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
        "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        s.insert(0, '_');
    }
    s
}

/// Extensiones que Windows ejecuta o interpreta al abrirlas. Lo que llega solo (Compartir) con una de ellas se guarda con
/// el sufijo ".recibido" para que no se ejecute por descuido; el usuario decide si lo renombra.
pub fn is_risky(name: &str) -> bool {
    const RISKY: [&str; 30] = [
        "exe", "msi", "msp", "bat", "cmd", "com", "scr", "pif", "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta",
        "lnk", "url", "jar", "reg", "dll", "cpl", "msc", "appx", "msix", "iso", "vhd", "scf", "chm",
    ];
    name.rsplit('.').next().map_or(false, |e| name.contains('.') && RISKY.contains(&e.to_ascii_lowercase().as_str()))
}

/// Marca el archivo como llegado de fuera (Mark of the Web, zona 3 = Internet): SmartScreen y Office avisan antes de
/// ejecutarlo o abrirlo con macros. Se escribe en el flujo alterno NTFS "Zone.Identifier"; si falla (FAT, redes) no es grave.
pub fn mark_of_the_web(path: &std::path::Path) {
    let mut ads = path.as_os_str().to_os_string();
    ads.push(":Zone.Identifier");
    let _ = std::fs::write(ads, "[ZoneTransfer]\r\nZoneId=3\r\nHostUrl=about:internet\r\n");
}

/// "nombre (1).ext" si ya existe.
pub fn unique_name(name: &str, taken: &dyn Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (1..)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|c| !taken(c))
        .unwrap()
}

fn join_remote(base: &str, name: &str) -> String {
    if base.is_empty() { name.to_string() } else { format!("{base}/{name}") }
}

// ---------------------------------------------------------------- móvil -> PC

pub async fn run_download<H: Ui>(h: H, core: Arc<Core>, id: u64, items: Vec<Item>, local_dir: String) {
    let label = match items.as_slice() {
        [one] => one.name.clone(),
        many => format!("{} elementos", many.len()),
    };
    let job = Job { h: &h, core: &core, id, epoch: core.epoch.load(Ordering::SeqCst), label, done: AtomicU64::new(0), total: AtomicU64::new(0) };
    job.emit("planning", "");
    match download(&job, items, PathBuf::from(local_dir)).await {
        Ok(()) => job.emit("done", ""),
        Err(e) if e == "cancelado" => job.emit("cancelled", "Cancelado"),
        Err(e) => job.emit("error", &e),
    }
}

async fn download<H: Ui>(job: &Job<'_, H>, items: Vec<Item>, dest_dir: PathBuf) -> Result<(), String> {
    if !dest_dir.is_dir() {
        return Err("La carpeta de destino no existe".into());
    }
    let mut files: Vec<FileTask> = Vec::new();
    for it in &items {
        let name = unique_name(&safe_name(&it.name), &|n| dest_dir.join(n).exists());
        let dest = dest_dir.join(&name);
        if it.dir {
            std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
            let mut stack = vec![(it.path.clone(), dest)];
            while let Some((remote, local)) = stack.pop() {
                if job.cancelled() {
                    return Err("cancelado".into());
                }
                let r = job.core.call("ls", json!({"path": remote})).await?;
                for e in r["items"].as_array().cloned().unwrap_or_default() {
                    let child = safe_name(e["name"].as_str().unwrap_or("_"));
                    let remote_child = join_remote(&remote, e["name"].as_str().unwrap_or(""));
                    if e["dir"] == true {
                        let l = local.join(&child);
                        std::fs::create_dir_all(&l).map_err(|e| e.to_string())?;
                        stack.push((remote_child, l));
                    } else {
                        files.push(FileTask {
                            src: remote_child,
                            dst: local.join(&child).to_string_lossy().into_owned(),
                            size: e["size"].as_u64().unwrap_or(0),
                        });
                    }
                }
            }
        } else {
            files.push(FileTask { src: it.path.clone(), dst: dest.to_string_lossy().into_owned(), size: it.size });
        }
    }
    job.total.store(files.iter().map(|f| f.size).sum(), Ordering::Relaxed);
    job.emit("running", "");

    run_parallel(&files, |f| download_file(job, f)).await
}

async fn download_file<H: Ui>(job: &Job<'_, H>, f: &FileTask) -> Result<(), String> {
    let part = format!("{}{PART}", f.dst);
    // La guarda se declara antes que el archivo: al salir, primero se cierra el archivo y luego se borra el temporal.
    let mut guard = PartGuard { path: PathBuf::from(&part), armed: true };
    let mut out = tokio::fs::File::create(&part).await.map_err(|e| e.to_string())?;
    let result = async {
        let mut offset = 0u64;
        let mut expected: Option<u64> = None;
        loop {
            if job.cancelled() {
                return Err("cancelado".to_string());
            }
            let r = job.core.call("read", json!({"path": f.src, "offset": offset, "len": CHUNK})).await?;
            let data = B64.decode(r["data"].as_str().unwrap_or("")).map_err(|e| e.to_string())?;
            if data.len() as u64 != r["n"].as_u64().unwrap_or(0) {
                return Err("Datos corruptos recibidos del móvil".into());
            }
            out.write_all(&data).await.map_err(|e| e.to_string())?;
            offset += data.len() as u64;
            job.done.fetch_add(data.len() as u64, Ordering::Relaxed);
            expected = r["size"].as_u64().or(expected);
            job.emit("running", &f.src);
            if r["eof"] == true || data.is_empty() {
                // Un archivo incompleto nunca se da por bueno: debe tener el tamaño que dice el móvil.
                return match expected {
                    Some(size) if size != offset => Err(format!("Transferencia incompleta ({offset} de {size} bytes)")),
                    _ => Ok(()),
                };
            }
        }
    }
    .await;
    match result {
        Ok(()) => {
            out.flush().await.map_err(|e| e.to_string())?;
            drop(out);
            std::fs::rename(&part, &f.dst).map_err(|e| e.to_string())?;
            mark_of_the_web(std::path::Path::new(&f.dst));
            guard.armed = false;
            Ok(())
        }
        Err(e) => Err(e), // la guarda borra el temporal
    }
}

// ---------------------------------------------------------------- PC -> móvil

pub async fn run_upload<H: Ui>(h: H, core: Arc<Core>, id: u64, items: Vec<Item>, remote_dir: String) {
    let label = match items.as_slice() {
        [one] => one.name.clone(),
        many => format!("{} elementos", many.len()),
    };
    let job = Job { h: &h, core: &core, id, epoch: core.epoch.load(Ordering::SeqCst), label, done: AtomicU64::new(0), total: AtomicU64::new(0) };
    job.emit("planning", "");
    match upload(&job, items, remote_dir).await {
        Ok(()) => job.emit("done", ""),
        Err(e) if e == "cancelado" => job.emit("cancelled", "Cancelado"),
        Err(e) => job.emit("error", &e),
    }
}

async fn upload<H: Ui>(job: &Job<'_, H>, items: Vec<Item>, remote_dir: String) -> Result<(), String> {
    let existing = job.core.call("ls", json!({"path": remote_dir})).await?;
    let taken: HashSet<String> = existing["items"]
        .as_array()
        .map(|a| a.iter().filter_map(|e| e["name"].as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<FileTask> = Vec::new();
    for it in &items {
        let name = unique_name(&it.name, &|n| taken.contains(n));
        let remote = join_remote(&remote_dir, &name);
        if it.dir {
            dirs.push(remote.clone());
            let mut stack = vec![(PathBuf::from(&it.path), remote)];
            while let Some((local, remote)) = stack.pop() {
                if job.cancelled() {
                    return Err("cancelado".into());
                }
                for entry in std::fs::read_dir(&local).map_err(|e| e.to_string())?.flatten() {
                    let Ok(ft) = entry.file_type() else { continue };
                    if ft.is_symlink() {
                        continue;
                    }
                    let child = join_remote(&remote, &entry.file_name().to_string_lossy());
                    if ft.is_dir() {
                        dirs.push(child.clone());
                        stack.push((entry.path(), child));
                    } else if ft.is_file() {
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        files.push(FileTask { src: entry.path().to_string_lossy().into_owned(), dst: child, size });
                    }
                }
            }
        } else {
            files.push(FileTask { src: it.path.clone(), dst: remote, size: it.size });
        }
    }
    job.total.store(files.iter().map(|f| f.size).sum(), Ordering::Relaxed);
    job.emit("running", "");

    for d in dirs {
        job.core.call("mkdir", json!({"path": d})).await?;
    }

    // Varios archivos a la vez. Si uno falla (o se cancela), los demás se detienen y se borran
    // los temporales que hayan quedado en el móvil, también los de las subidas interrumpidas a medias.
    let in_flight: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    let in_flight_ref = &in_flight;
    let result = run_parallel(&files, |f| async move {
        in_flight_ref.lock().unwrap().insert(f.dst.clone());
        let r = upload_file(job, f).await;
        if r.is_ok() {
            in_flight_ref.lock().unwrap().remove(&f.dst);
        }
        r
    })
    .await;
    if result.is_err() {
        let leftovers: Vec<String> = in_flight.lock().unwrap().iter().cloned().collect();
        for dst in leftovers {
            job.core.abort_partial(&dst).await;
        }
    }
    result
}

async fn upload_file<H: Ui>(job: &Job<'_, H>, f: &FileTask) -> Result<(), String> {
    let mut input = tokio::fs::File::open(Path::new(&f.src)).await.map_err(|e| e.to_string())?;
    let mut offset = 0u64;
    let mut buf = vec![0u8; CHUNK];
    loop {
        if job.cancelled() {
            return Err("cancelado".into());
        }
        let mut n = 0;
        while n < buf.len() {
            let r = input.read(&mut buf[n..]).await.map_err(|e| e.to_string())?;
            if r == 0 {
                break;
            }
            n += r;
        }
        let last = n < buf.len();
        let r = job
            .core
            .call("write", json!({"path": f.dst, "offset": offset, "data": B64.encode(&buf[..n]), "final": last}))
            .await?;
        offset += n as u64;
        job.done.fetch_add(n as u64, Ordering::Relaxed);
        job.emit("running", &f.dst);
        if last {
            // El móvil informa del tamaño final guardado: debe coincidir con lo enviado.
            return match r["size"].as_u64() {
                Some(size) if size != offset => Err(format!("El móvil guardó {size} bytes de {offset}")),
                _ => Ok(()),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_the_phone_cannot_escape_or_break_windows() {
        assert_eq!(safe_name("..\\..\\x"), ".._.._x");
        assert!(!safe_name("..\\..\\Windows\\evil.exe").contains('\\'));
        assert!(!safe_name("a/b").contains('/'));
        assert_eq!(safe_name(".."), "_");
        assert_eq!(safe_name("foto?.jpg"), "foto_.jpg");
        assert_eq!(safe_name("nombre. "), "nombre");
        assert_eq!(safe_name("CON"), "_CON");
        assert_eq!(safe_name("nul.txt"), "_nul.txt");
        assert_eq!(safe_name("normal.txt"), "normal.txt");
    }

    #[test]
    fn unique_names_do_not_overwrite() {
        let taken: HashSet<&str> = ["a.txt", "a (1).txt", "carpeta"].into_iter().collect();
        let t = |n: &str| taken.contains(n);
        assert_eq!(unique_name("b.txt", &t), "b.txt");
        assert_eq!(unique_name("a.txt", &t), "a (2).txt");
        assert_eq!(unique_name("carpeta", &t), "carpeta (1)");
    }
}
