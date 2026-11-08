//! Acciones sobre el móvil que Android restringe a las apps (abrir, detener, desinstalar...) y se hacen por adb.

use std::path::Path;
use std::process::Command;

/// adb: primero el del SDK de Android (ANDROID_HOME), luego el que descargó SyncroUmbral, y por último el del PATH.
fn adb_bin() -> String {
    std::env::var("ANDROID_HOME")
        .ok()
        .map(|p| format!("{p}\\platform-tools\\adb.exe"))
        .filter(|p| Path::new(p).exists())
        .or_else(|| crate::tools::adb_in_tools().map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "adb".into())
}

/// ¿Se puede ejecutar adb? Devuelve su versión.
pub fn adb_version() -> Option<String> {
    run(&["version"]).ok().and_then(|v| v.lines().next().map(String::from))
}

/// Direcciones IP:puerto que los móviles ofrecen para emparejar la depuración inalámbrica.
pub fn pairing_services() -> Vec<String> {
    run(&["mdns", "services"])
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("_adb-tls-pairing"))
        .filter_map(|l| l.split_whitespace().last().map(String::from))
        .collect()
}

/// `adb pair IP:PUERTO CODIGO`. Se valida el formato antes de ejecutar nada.
pub fn pair(addr: &str, code: &str) -> Result<String, String> {
    let ok_addr = {
        let mut it = addr.split(':');
        match (it.next(), it.next(), it.next()) {
            (Some(ip), Some(port), None) => {
                ip.split('.').count() == 4
                    && ip.split('.').all(|o| !o.is_empty() && o.len() <= 3 && o.chars().all(|c| c.is_ascii_digit()))
                    && !port.is_empty()
                    && port.len() <= 5
                    && port.chars().all(|c| c.is_ascii_digit())
            }
            _ => false,
        }
    };
    if !ok_addr {
        return Err("La dirección debe tener el formato 192.168.1.20:37000".into());
    }
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err("El código tiene 6 dígitos".into());
    }
    run(&["pair", addr, code])
}

fn run(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(adb_bin());
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // sin ventana de consola
    }
    let out = cmd.output().map_err(|e| format!("No se pudo ejecutar adb: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(text.trim().to_string())
    } else {
        Err(text.trim().to_string())
    }
}

/// Un nombre de paquete de Android: letras, dígitos, puntos y guiones bajos. Se valida antes de pasarlo
/// a adb porque adb une los argumentos en una sola orden de shell del móvil (evita inyección).
pub fn valid_pkg(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 255
        && !p.starts_with('.')
        && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

fn keyguard_showing(serial: &str) -> bool {
    shell(serial, &["dumpsys", "window"]).map(|t| t.contains("isKeyguardShowing=true")).unwrap_or(false)
}

/// ¿Está el teléfono dormido (pantalla apagada)? Si lo está, nadie lo está usando.
pub fn is_asleep(phone_ip: Option<&str>) -> bool {
    match find_device(phone_ip) {
        Ok(serial) => shell(&serial, &["dumpsys", "power"])
            .map(|t| t.contains("mWakefulness=Dozing") || t.contains("mWakefulness=Asleep"))
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Apaga solo el panel físico del móvil (el sistema sigue despierto) si nadie lo está usando: o estaba dormido antes de
/// empezar, o está bloqueado. Si lo estás usando desbloqueado no se toca. Mejor esfuerzo.
pub fn panel_off_if_unused(phone_ip: Option<&str>, was_asleep: bool) {
    let Ok(serial) = find_device(phone_ip) else { return };
    let mut unused = was_asleep;
    // Al despertar el sistema tarda un instante en marcar el bloqueo: se reintenta unas veces.
    for _ in 0..4 {
        if unused || keyguard_showing(&serial) {
            unused = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(700));
    }
    if !unused {
        return;
    }
    // Al despertar el teléfono el sistema enciende el panel un momento después y anula la orden: se repite hasta que quede apagado.
    for _ in 0..6 {
        let _ = shell(&serial, &["cmd", "display", "power-off", "0"]);
        std::thread::sleep(std::time::Duration::from_millis(1500));
        if !panel_is_on(&serial) {
            return;
        }
    }
}

/// ¿Está encendido el panel físico (pantalla 0)?
fn panel_is_on(serial: &str) -> bool {
    shell(serial, &["dumpsys", "display"])
        .map(|t| {
            t.lines()
                .find(|l| l.contains("DisplayInfo{") && l.contains("displayId 0,"))
                .map(|l| l.contains(", state ON"))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Al terminar: si el teléfono sigue bloqueado se deja dormido y con el panel apagado (no hay forma de "encender" solo el panel;
/// se restablece al dormir y despertar). Si lo estás usando desbloqueado no se toca.
pub fn sleep_if_locked(phone_ip: Option<&str>) {
    if let Ok(serial) = find_device(phone_ip) {
        if keyguard_showing(&serial) {
            let _ = shell(&serial, &["input", "keyevent", "KEYCODE_SLEEP"]);
        }
    }
}

/// Mata el servidor de scrcpy que haya quedado en el móvil tras un cierre forzado (best effort).
pub fn cleanup_scrcpy_server() {
    let _ = run(&["shell", "pkill", "-f", "scrcpy"]);
}

fn devices() -> Vec<String> {
    run(&["devices"])
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            match (it.next(), it.next()) {
                (Some(s), Some("device")) => Some(s.to_string()),
                _ => None,
            }
        })
        .collect()
}

/// Busca el móvil: primero adb ya conectado (USB o inalámbrico); si no, intenta conectar por la
/// depuración inalámbrica usando la IP desde la que se conectó SyncroUmbral.
pub fn find_device(phone_ip: Option<&str>) -> Result<String, String> {
    let pick = |list: Vec<String>| -> Option<String> {
        if let Some(ip) = phone_ip {
            if let Some(s) = list.iter().find(|s| s.starts_with(&format!("{ip}:"))) {
                return Some(s.clone());
            }
        }
        if list.len() == 1 { list.into_iter().next() } else { None }
    };
    if let Some(s) = pick(devices()) {
        return Ok(s);
    }
    if let Some(ip) = phone_ip {
        let services = run(&["mdns", "services"]).unwrap_or_default();
        for l in services.lines().filter(|l| l.contains("_adb-tls-connect")) {
            if let Some(addr) = l.split_whitespace().last().filter(|a| a.starts_with(&format!("{ip}:"))) {
                let _ = run(&["connect", addr]);
                if let Some(s) = pick(devices()) {
                    return Ok(s);
                }
            }
        }
    }
    Err("No hay acceso por adb al móvil. Activa la depuración inalámbrica (o USB) en el móvil.".into())
}

fn shell(serial: &str, args: &[&str]) -> Result<String, String> {
    let mut all = vec!["-s", serial, "shell"];
    all.extend_from_slice(args);
    run(&all)
}

pub fn launch(serial: &str, pkg: &str) -> Result<String, String> {
    shell(serial, &["monkey", "-p", pkg, "-c", "android.intent.category.LAUNCHER", "1"]).map(|_| "Abierta en el móvil".into())
}

pub fn force_stop(serial: &str, pkg: &str) -> Result<String, String> {
    shell(serial, &["am", "force-stop", pkg]).map(|_| "Detenida".into())
}

pub fn app_settings(serial: &str, pkg: &str) -> Result<String, String> {
    shell(serial, &["am", "start", "-a", "android.settings.APPLICATION_DETAILS_SETTINGS", "-d", &format!("package:{pkg}")])
        .map(|_| "Ajustes de la app abiertos en el móvil".into())
}

pub fn clear_data(serial: &str, pkg: &str) -> Result<String, String> {
    let r = shell(serial, &["pm", "clear", pkg])?;
    if r.contains("Success") { Ok("Datos borrados".into()) } else { Err(r) }
}

pub fn uninstall(serial: &str, pkg: &str) -> Result<String, String> {
    let r = shell(serial, &["pm", "uninstall", pkg])?;
    if r.contains("Success") { Ok("Desinstalada".into()) } else { Err(r) }
}

/// Copia el/los APK de la app a `dest_dir`. Devuelve la ruta creada.
pub fn pull_apk(serial: &str, pkg: &str, dest_dir: &Path) -> Result<String, String> {
    let paths: Vec<String> = shell(serial, &["pm", "path", pkg])?
        .lines()
        .filter_map(|l| l.trim().strip_prefix("package:").map(String::from))
        .collect();
    if paths.is_empty() {
        return Err("No se encontró el APK de la app".into());
    }
    let target = if paths.len() == 1 {
        dest_dir.join(format!("{pkg}.apk"))
    } else {
        let d = dest_dir.join(pkg);
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        d
    };
    for p in &paths {
        let dst = if paths.len() == 1 {
            target.clone()
        } else {
            target.join(p.rsplit('/').next().unwrap_or("base.apk"))
        };
        run(&["-s", serial, "pull", p, &dst.to_string_lossy()])?;
    }
    Ok(target.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{pair, valid_pkg};

    #[test]
    fn pairing_input_is_validated_before_running_adb() {
        for bad in ["", "192.168.1.5", "192.168.1.5:", "192.168.1.5:99999999", "a.b.c.d:1", "1.2.3.4:5 && calc", "1.2.3.4:5:6", "$(id):1"] {
            assert!(pair(bad, "123456").is_err(), "debía rechazar {bad:?}");
        }
        for bad_code in ["", "12345", "1234567", "12345a", "123 56", "; ls"] {
            assert!(pair("192.168.1.5:37000", bad_code).is_err(), "debía rechazar el código {bad_code:?}");
        }
    }

    #[test]
    fn package_names_cannot_inject_shell_commands() {
        assert!(valid_pkg("com.google.android.apps.maps"));
        assert!(valid_pkg("app.syncroapp.umbral"));
        assert!(!valid_pkg(""));
        assert!(!valid_pkg("com.x; rm -rf /sdcard"));
        assert!(!valid_pkg("com.x && reboot"));
        assert!(!valid_pkg("a b"));
        assert!(!valid_pkg("$(id)"));
        assert!(!valid_pkg("a`id`"));
        assert!(!valid_pkg("../x"));
        assert!(!valid_pkg(".hidden"));
    }
}
