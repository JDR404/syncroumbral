//! Apps del móvil en ventanas del PC, con scrcpy en una pantalla virtual: la app corre aparte de la
//! pantalla del teléfono, que puede seguir bloqueado. Una ventana por app.

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub type Windows = Mutex<HashMap<String, Child>>;

const NO_WINDOW: u32 = 0x0800_0000;

fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(NO_WINDOW); // sin ventana de consola; la de scrcpy es aparte
    }
    cmd
}

fn works(bin: &str) -> bool {
    quiet(Command::new(bin).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// scrcpy en el PATH, o el que instala winget (`winget install Genymobile.scrcpy`).
pub fn find_scrcpy() -> Option<PathBuf> {
    if works("scrcpy") {
        return Some("scrcpy".into());
    }
    if let Some(p) = crate::tools::scrcpy_in_tools() {
        return Some(p); // el que descargó SyncroUmbral
    }
    let base = PathBuf::from(std::env::var("LOCALAPPDATA").ok()?).join("Microsoft\\WinGet\\Packages");
    for pkg in std::fs::read_dir(base).ok()?.flatten() {
        if pkg.file_name().to_string_lossy().starts_with("Genymobile.scrcpy") {
            for sub in std::fs::read_dir(pkg.path()).ok()?.flatten() {
                let exe = sub.path().join("scrcpy.exe");
                if exe.exists() {
                    return Some(exe);
                }
            }
        }
    }
    None
}

fn last_lines(path: &PathBuf) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let v: Vec<&str> = text.lines().rev().take(4).collect();
    v.into_iter().rev().collect::<Vec<_>>().join(" | ")
}

pub fn open(windows: &Windows, serial: &str, pkg: &str, label: &str) -> Result<String, String> {
    let bin = find_scrcpy().ok_or("No se encontró scrcpy. Instálalo con: winget install Genymobile.scrcpy")?;
    let log_dir = std::env::temp_dir().join("umbral");
    std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let log_path = log_dir.join(format!("scrcpy-{pkg}.log"));

    {
        let mut map = windows.lock().unwrap();
        if let Some(c) = map.get_mut(pkg) {
            if matches!(c.try_wait(), Ok(None)) {
                return Err("Esa app ya está abierta en el PC".into());
            }
            map.remove(pkg);
        }
        let log = File::create(&log_path).map_err(|e| e.to_string())?;
        let log2 = log.try_clone().map_err(|e| e.to_string())?;
        let title: String = label.chars().filter(|c| !c.is_control()).take(60).collect();
        let mut cmd = Command::new(bin);
        // Sin la barra de tareas del sistema: en la pantalla virtual se dibujaba encima de la app y tapaba su parte inferior.
        cmd.args(["-s", serial, "--new-display=1080x2160/420", "--no-vd-system-decorations"])
            .arg(format!("--start-app={pkg}"))
            .arg(format!("--window-title={title}"))
            .stdout(log)
            .stderr(log2);
        let child = quiet(&mut cmd).spawn().map_err(|e| format!("No se pudo iniciar scrcpy: {e}"))?;
        map.insert(pkg.to_string(), child);
    }

    // Si scrcpy falla (app sin icono de lanzador, depuración apagada...) se cierra enseguida: se informa el motivo.
    std::thread::sleep(Duration::from_millis(2000));
    let mut map = windows.lock().unwrap();
    if let Some(c) = map.get_mut(pkg) {
        if let Ok(Some(status)) = c.try_wait() {
            map.remove(pkg);
            return Err(format!("scrcpy se cerró ({status}): {}", last_lines(&log_path)));
        }
    }
    Ok("Abierta en una ventana del PC".into())
}

/// Mientras haya ventanas de apps abiertas mantiene el móvil despierto con el panel apagado: con el teléfono bloqueado
/// Android lo duerme a los 10 s y, al dormirse, apaga también la pantalla virtual (la app se ve negra).
pub async fn awake_keeper(core: std::sync::Arc<crate::core::Core>) {
    {
        let mut held = false;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let active = !running(&core.app_windows).is_empty();
            if active && !held {
                // Se anota si el teléfono estaba dormido ANTES de despertarlo para mantenerlo: entonces nadie lo está usando.
                let ip = core.phone_ip();
                let ip2 = ip.clone();
                let asleep = tokio::task::spawn_blocking(move || crate::adb::is_asleep(ip2.as_deref()))
                    .await
                    .unwrap_or(false);
                if core.call("keep_awake", serde_json::json!({"on": true})).await.is_ok() {
                    held = true;
                    let _ = tokio::task::spawn_blocking(move || crate::adb::panel_off_if_unused(ip.as_deref(), asleep)).await;
                }
            } else if !active && held {
                held = false;
                core.cleanup_call("keep_awake", serde_json::json!({"on": false})).await;
                let ip = core.phone_ip();
                let _ = tokio::task::spawn_blocking(move || crate::adb::sleep_if_locked(ip.as_deref())).await;
            }
        }
    }
}

/// Paquetes con ventana abierta (descarta las que el usuario ya cerró).
pub fn running(windows: &Windows) -> Vec<String> {
    let mut map = windows.lock().unwrap();
    map.retain(|_, c| matches!(c.try_wait(), Ok(None)));
    map.keys().cloned().collect()
}

/// Cierra la ventana de una app (al detenerla o desinstalarla): sin la app, la ventana solo mostraría una pantalla vacía.
pub fn close_one(windows: &Windows, pkg: &str) {
    let child = windows.lock().unwrap().remove(pkg);
    if let Some(mut c) = child {
        std::thread::spawn(move || {
            let _ = quiet(Command::new("taskkill").args(["/PID", &c.id().to_string()])).status();
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_secs(3) {
                if matches!(c.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = c.kill();
            let _ = c.wait();
            crate::adb::cleanup_scrcpy_server();
        });
    }
}

/// Cierra todas las ventanas. Primero con un cierre normal (para que el móvil libere la pantalla virtual);
/// si alguna no responde en 3 s se fuerza y se limpia el servidor que quedó en el móvil.
pub fn close_all(windows: &Windows) {
    let children: Vec<Child> = windows.lock().unwrap().drain().map(|(_, c)| c).collect();
    if children.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let mut forced = false;
        for mut c in children {
            let _ = quiet(Command::new("taskkill").args(["/PID", &c.id().to_string()])).status();
            let t0 = Instant::now();
            loop {
                if matches!(c.try_wait(), Ok(Some(_))) {
                    break;
                }
                if t0.elapsed() > Duration::from_secs(3) {
                    let _ = c.kill();
                    let _ = c.wait();
                    forced = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        if forced {
            crate::adb::cleanup_scrcpy_server();
        }
    });
}
