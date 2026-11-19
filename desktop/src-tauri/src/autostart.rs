//! Inicio con Windows (clave Run del registro). Sin dependencias de interfaz.

use std::process::Command;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "SyncroUmbral";

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

/// Activa o desactiva el inicio con Windows (en segundo plano, sin abrir la ventana).
pub fn set_autostart(on: bool) -> Result<(), String> {
    let out = if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let value = format!("\"{}\" --background", exe.display());
        reg(&["add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &value, "/f"])
    } else {
        reg(&["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
    }
    .map_err(|e| e.to_string())?;
    if out.status.success() || !on {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
