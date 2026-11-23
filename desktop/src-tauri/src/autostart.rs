//! Inicio con Windows (clave Run del registro). Sin dependencias de interfaz.

use std::path::{Path, PathBuf};
use std::process::Command;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "SyncroUmbral";
/// Archivo en la carpeta de datos que indica que el usuario desactivó el inicio con Windows (se respeta siempre).
const OFF_MARKER: &str = "autostart-off";

fn reg(args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new("reg");
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.output()
}

fn data_dir() -> PathBuf {
    std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir()).join("app.syncroapp.umbral")
}

/// Línea de comando exacta que debe quedar en la clave Run: ruta entre comillas (válida aunque tenga espacios) y `--background`.
pub fn command_for(exe: &Path) -> String {
    format!("\"{}\" --background", exe.display())
}

/// Ruta del ejecutable dentro de una línea de comando de la clave Run (con o sin comillas).
pub fn exe_of(cmd: &str) -> String {
    let c = cmd.trim();
    if let Some(rest) = c.strip_prefix('"') {
        return rest.split('"').next().unwrap_or("").to_string();
    }
    match c.to_ascii_lowercase().find(".exe") {
        Some(i) => c[..i + 4].to_string(),
        None => c.split(' ').next().unwrap_or("").to_string(),
    }
}

/// Valor actual de la entrada en la clave Run (None si no existe).
pub fn autostart_value() -> Option<String> {
    let out = reg(&["query", RUN_KEY, "/v", RUN_VALUE]).ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    // Línea: "    SyncroUmbral    REG_SZ    <valor>"
    text.lines().find_map(|l| l.split_once("REG_SZ").map(|(_, v)| v.trim().to_string()))
}

pub fn autostart_enabled() -> bool {
    reg(&["query", RUN_KEY, "/v", RUN_VALUE]).map(|o| o.status.success()).unwrap_or(false)
}

/// Migración desde el nombre anterior de la aplicación: si el usuario tenía el inicio con Windows activado con la entrada
/// vieja ("Umbral", que apunta al ejecutable antiguo), se quita y se crea la nueva apuntando a este ejecutable.
pub fn migrate_legacy_autostart() {
    const OLD: &str = "Umbral";
    if reg(&["query", RUN_KEY, "/v", OLD]).map(|o| o.status.success()).unwrap_or(false) {
        let _ = reg(&["delete", RUN_KEY, "/v", OLD, "/f"]);
        let _ = set_autostart(true);
    }
}

/// Activa o desactiva el inicio con Windows (en segundo plano, sin abrir la ventana). Recuerda la decisión del usuario.
pub fn set_autostart(on: bool) -> Result<(), String> {
    let marker = data_dir().join(OFF_MARKER);
    let out = if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let value = command_for(&exe);
        reg(&["add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &value, "/f"])
    } else {
        reg(&["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
    }
    .map_err(|e| e.to_string())?;
    if out.status.success() || !on {
        if on {
            let _ = std::fs::remove_file(&marker);
        } else {
            let _ = std::fs::create_dir_all(data_dir());
            let _ = std::fs::write(&marker, "1");
        }
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Decide qué hacer con una entrada existente (función pura, probada). `existing` es el valor actual de la clave Run,
/// `exe` el ejecutable en marcha, `installed` la ruta de instalación habitual y `exists` indica si un archivo existe.
/// Devuelve el valor que debe quedar escrito, o None si no hay que tocar nada.
pub fn repair_plan(existing: Option<&str>, exe: &Path, installed: Option<&Path>, exists: impl Fn(&str) -> bool) -> Option<String> {
    let cur = existing?; // sin entrada = el usuario la quitó (o nunca existió): no se recrea
    let desired = command_for(exe);
    if cur == desired {
        return None;
    }
    let target = exe_of(cur);
    let same_exe = target.eq_ignore_ascii_case(&exe.display().to_string());
    let stale = !exists(&target);
    let running_installed = installed.map(|p| p.display().to_string().eq_ignore_ascii_case(&exe.display().to_string())).unwrap_or(false);
    if same_exe || (stale && running_installed) {
        Some(desired)
    } else {
        None // apunta a otra copia que sí existe: no se le quita
    }
}

/// Comprueba al arrancar que la entrada de inicio es correcta (comillas, ruta existente) y la repara si no.
/// Nunca la crea si el usuario la desactivó. Devuelve un texto para el registro de arranque.
pub fn repair_autostart() -> String {
    if data_dir().join(OFF_MARKER).exists() {
        return "inicio con Windows desactivado por el usuario: no se toca".into();
    }
    let existing = autostart_value();
    let Ok(exe) = std::env::current_exe() else {
        return "no se pudo conocer la ruta del ejecutable".into();
    };
    let installed = std::env::var("LOCALAPPDATA").ok().map(|b| PathBuf::from(b).join("SyncroUmbral").join("syncroumbral.exe"));
    match repair_plan(existing.as_deref(), &exe, installed.as_deref(), |p| Path::new(p).exists()) {
        Some(_) => match set_autostart(true) {
            Ok(()) => format!("entrada de inicio reparada (antes: {:?})", existing.unwrap_or_default()),
            Err(e) => format!("no se pudo reparar la entrada de inicio: {e}"),
        },
        None => format!("entrada de inicio correcta: {:?}", existing.unwrap_or_else(|| "(sin entrada)".into())),
    }
}

/// Estado del inicio con Windows, explicado al usuario.
#[derive(Debug, PartialEq, Eq)]
pub enum Estado {
    /// Todo bien: arrancará solo al iniciar sesión.
    Activo,
    /// El usuario lo apagó en SyncroUmbral (se respeta).
    ApagadoPorUsuario,
    /// No hay entrada: no arrancará solo.
    SinEntrada,
    /// Windows lo tiene desactivado (Configuración > Aplicaciones > Inicio).
    DesactivadoEnWindows,
    /// La entrada apunta a un archivo que ya no existe.
    RutaInexistente(String),
    /// La entrada apunta a otra copia del programa que sí existe.
    OtraCopia(String),
}

impl Estado {
    /// Texto para Ajustes.
    pub fn texto(&self) -> String {
        match self {
            Estado::Activo => "Activo: SyncroUmbral arrancará solo al iniciar sesión en Windows.".into(),
            Estado::ApagadoPorUsuario => "Desactivado: lo apagaste aquí, así que no arrancará solo.".into(),
            Estado::SinEntrada => "Sin entrada de inicio: no arrancará solo. Activa «Iniciar con Windows».".into(),
            Estado::DesactivadoEnWindows => {
                "Windows lo tiene desactivado y no lo abrirá al iniciar. Actívalo en Configuración > Aplicaciones > Inicio.".into()
            }
            Estado::RutaInexistente(p) => format!("La entrada de inicio apunta a un archivo que ya no existe ({p}). Se repara al abrir SyncroUmbral desde su carpeta instalada."),
            Estado::OtraCopia(p) => format!("El inicio abre otra copia de SyncroUmbral ({p}), no esta."),
        }
    }
    /// ¿Es un problema que el usuario debería mirar?
    pub fn es_problema(&self) -> bool {
        !matches!(self, Estado::Activo | Estado::ApagadoPorUsuario)
    }
}

/// Decide el estado a partir de lo leído del sistema (función pura, probada).
/// `approved_off`: Some(true) si `StartupApproved` marca la entrada como desactivada.
pub fn classify(value: Option<&str>, user_off: bool, approved_off: Option<bool>, exe: &Path, exists: impl Fn(&str) -> bool) -> Estado {
    let Some(cur) = value else {
        return if user_off { Estado::ApagadoPorUsuario } else { Estado::SinEntrada };
    };
    if approved_off == Some(true) {
        return Estado::DesactivadoEnWindows;
    }
    let target = exe_of(cur);
    if !exists(&target) {
        return Estado::RutaInexistente(target);
    }
    if !target.eq_ignore_ascii_case(&exe.display().to_string()) {
        return Estado::OtraCopia(target);
    }
    Estado::Activo
}

/// ¿`StartupApproved\Run` marca esta entrada como desactivada? (primer byte impar: 03, 07...). None si no hay marca.
fn approved_off() -> Option<bool> {
    let out = reg(&["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run", "/v", RUN_VALUE]).ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let hex = text.lines().find_map(|l| l.split_once("REG_BINARY").map(|(_, v)| v.trim().to_string()))?;
    let first = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
    Some(first & 1 == 1)
}

/// Estado actual del inicio con Windows para esta copia del programa.
pub fn estado() -> Estado {
    let exe = std::env::current_exe().unwrap_or_default();
    classify(autostart_value().as_deref(), data_dir().join(OFF_MARKER).exists(), approved_off(), &exe, |p| Path::new(p).exists())
}

/// Ejecuta `reg` con esos argumentos; true si salió bien.
pub fn reg_run(args: &[&str]) -> bool {
    reg(args).map(|o| o.status.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = r"C:\Users\ana maria\AppData\Local\SyncroUmbral\syncroumbral.exe";

    #[test]
    fn estado_del_inicio() {
        let exe = Path::new(r"C:\a\b.exe");
        let ok = |_: &str| true;
        let cmd = command_for(exe);
        assert_eq!(classify(Some(&cmd), false, None, exe, ok), Estado::Activo);
        assert_eq!(classify(Some(&cmd), false, Some(false), exe, ok), Estado::Activo);
        assert_eq!(classify(Some(&cmd), false, Some(true), exe, ok), Estado::DesactivadoEnWindows);
        assert_eq!(classify(None, true, None, exe, ok), Estado::ApagadoPorUsuario);
        assert_eq!(classify(None, false, None, exe, ok), Estado::SinEntrada);
        assert_eq!(classify(Some(&cmd), false, None, exe, |_| false), Estado::RutaInexistente(r"C:\a\b.exe".into()));
        assert_eq!(
            classify(Some("\"C:\\otra\\x.exe\" --background"), false, None, exe, ok),
            Estado::OtraCopia(r"C:\otra\x.exe".into())
        );
        assert!(Estado::SinEntrada.es_problema() && !Estado::Activo.es_problema() && !Estado::ApagadoPorUsuario.es_problema());
    }

    #[test]
    fn comando_con_comillas() {
        assert_eq!(command_for(Path::new(EXE)), format!("\"{EXE}\" --background"));
    }

    #[test]
    fn extrae_ruta_con_y_sin_comillas() {
        assert_eq!(exe_of(&format!("\"{EXE}\" --background")), EXE);
        assert_eq!(exe_of(r"C:\a\b.exe --background"), r"C:\a\b.exe");
        assert_eq!(exe_of(r"C:\A\B.EXE"), r"C:\A\B.EXE");
    }

    #[test]
    fn repara_valor_sin_comillas_de_la_misma_copia() {
        let exe = Path::new(r"C:\a\b.exe");
        let plan = repair_plan(Some(r"C:\a\b.exe --background"), exe, None, |_| true);
        assert_eq!(plan.as_deref(), Some("\"C:\\a\\b.exe\" --background"));
    }

    #[test]
    fn no_toca_valor_correcto_ni_recrea_entrada_quitada() {
        let exe = Path::new(r"C:\a\b.exe");
        assert_eq!(repair_plan(Some(&command_for(exe)), exe, None, |_| true), None);
        assert_eq!(repair_plan(None, exe, None, |_| true), None);
    }

    #[test]
    fn no_quita_otra_copia_que_existe() {
        let exe = Path::new(r"C:\dev\prueba.exe");
        let inst = Path::new(r"C:\x\inst.exe");
        assert_eq!(repair_plan(Some("\"C:\\x\\inst.exe\" --background"), exe, Some(inst), |_| true), None);
    }

    #[test]
    fn repara_ruta_inexistente_solo_desde_la_instalada() {
        let inst = Path::new(r"C:\x\inst.exe");
        let vieja = "\"C:\\viejo\\app.exe\" --background";
        assert!(repair_plan(Some(vieja), inst, Some(inst), |_| false).is_some());
        assert_eq!(repair_plan(Some(vieja), Path::new(r"C:\dev\t.exe"), Some(inst), |_| false), None);
    }
}
