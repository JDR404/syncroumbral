//! Herramientas externas que SyncroUmbral necesita para algunas funciones: `adb` (Google) y `scrcpy` (GitHub).
//! Se pueden descargar desde la propia aplicación a la carpeta de datos, sin permisos de administrador.
//! Solo se usan estas dos direcciones https, con versión fija y hash SHA-256 verificado antes de instalar.

use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

/// Versiones fijas y probadas, con el SHA-256 del archivo incrustado en SyncroUmbral: si el archivo descargado no coincide
/// (descarga manipulada o corrupta) no se instala. El SHA-1 de adb coincide además con el que publica Google en
/// repository2-3.xml (d7ee482b3874f1e0d6d5dca135f6bd58f5716db0).
const ADB_URL: &str = "https://dl.google.com/android/repository/platform-tools_r37.0.2-win.zip";
const ADB_SHA256: &str = "6d93a5e63bc1654b70b7b94bec5c6ab46db113df0265beb314055dd253772b58";
const SCRCPY_URL: &str = "https://github.com/Genymobile/scrcpy/releases/download/v4.1/scrcpy-win64-v4.1.zip";
const SCRCPY_SHA256: &str = "5b12172b3264b2889f4583ee64752ce832e29bc8b1089dca81093459697165db";
const MAX_DOWNLOAD: u64 = 120 * 1024 * 1024;

fn verify_sha256(bytes: &[u8], expected: &str) -> Result<(), String> {
    let got = hex::encode(Sha256::digest(bytes));
    if crate::core::ct_eq(got.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err("La descarga no coincide con el hash esperado; no se instaló".into())
    }
}

static DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init(data_dir: &Path) {
    let _ = DIR.set(data_dir.join("tools"));
}

fn dir() -> Option<&'static PathBuf> {
    DIR.get()
}

pub fn adb_in_tools() -> Option<PathBuf> {
    let p = dir()?.join("platform-tools").join("adb.exe");
    p.exists().then_some(p)
}

pub fn scrcpy_in_tools() -> Option<PathBuf> {
    for sub in std::fs::read_dir(dir()?.join("scrcpy")).ok()?.flatten() {
        let exe = sub.path().join("scrcpy.exe");
        if exe.exists() {
            return Some(exe);
        }
    }
    None
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(90))
        .user_agent("SyncroUmbral")
        .build()
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err("Solo se permiten descargas https".into());
    }
    let resp = agent().get(url).call().map_err(|e| format!("No se pudo descargar: {e}"))?;
    let mut buf = Vec::new();
    resp.into_reader().take(MAX_DOWNLOAD + 1).read_to_end(&mut buf).map_err(|e| format!("Descarga interrumpida: {e}"))?;
    if buf.len() as u64 > MAX_DOWNLOAD {
        return Err("La descarga es demasiado grande".into());
    }
    Ok(buf)
}

/// Descomprime un zip dentro de `dest`. Ignora cualquier entrada que intente salir de esa carpeta.
fn extract(bytes: Vec<u8>, dest: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Archivo zip no válido: {e}"))?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = f.enclosed_name() else { continue };
        let out = dest.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut file = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut file).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn install_adb() -> Result<PathBuf, String> {
    let root = dir().ok_or("Carpeta de herramientas no inicializada")?;
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let bytes = download(ADB_URL)?;
    verify_sha256(&bytes, ADB_SHA256)?;
    let target = root.join("platform-tools");
    let _ = std::fs::remove_dir_all(&target);
    extract(bytes, root)?;
    adb_in_tools().ok_or_else(|| "El paquete descargado no contiene adb".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downloads_must_match_the_pinned_hash() {
        let data = b"contenido";
        let ok = hex::encode(Sha256::digest(data));
        assert!(verify_sha256(data, &ok).is_ok());
        assert!(verify_sha256(b"contenido alterado", &ok).is_err(), "una descarga distinta no se instala");
        assert!(verify_sha256(data, &"0".repeat(64)).is_err());
    }

    #[test]
    fn pinned_hashes_are_well_formed_and_urls_are_https_with_fixed_versions() {
        for h in [ADB_SHA256, SCRCPY_SHA256] {
            assert_eq!(h.len(), 64);
            assert!(h.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }
        assert!(ADB_URL.starts_with("https://dl.google.com/") && !ADB_URL.contains("latest"));
        assert!(SCRCPY_URL.starts_with("https://github.com/Genymobile/scrcpy/releases/download/v"));
    }
}

pub fn install_scrcpy() -> Result<PathBuf, String> {
    let root = dir().ok_or("Carpeta de herramientas no inicializada")?;
    let bytes = download(SCRCPY_URL)?;
    verify_sha256(&bytes, SCRCPY_SHA256)?;

    let target = root.join("scrcpy");
    let _ = std::fs::remove_dir_all(&target);
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    extract(bytes, &target)?;
    scrcpy_in_tools().ok_or_else(|| "El paquete descargado no contiene scrcpy".into())
}
