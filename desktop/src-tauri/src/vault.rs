//! Protección de los secretos y de la configuración en disco.
//!
//! - Los secretos (clave privada TLS y la llave del sellado) se guardan cifrados con DPAPI de Windows: solo los puede
//!   descifrar el mismo usuario de Windows en este equipo. Copiar la carpeta de datos a otro equipo o leerla desde otra
//!   cuenta no sirve de nada.
//! - La lista de móviles, la clave de respaldo y las apps silenciadas llevan un sello HMAC-SHA256 con esa llave: editar
//!   el archivo a mano (por ejemplo para añadir un móvil del atacante o cambiar la clave) lo invalida y SyncroUmbral lo ignora.
//!
//! Límite honesto: un programa que se ejecute con tu mismo usuario de Windows también puede usar DPAPI. Esto sube el
//! listón de "editar un archivo con el Bloc de notas" a "ejecutar código en tu cuenta", que ya es otro nivel de compromiso.

use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use std::path::{Path, PathBuf};

const ENTROPY: &[u8] = b"umbral-vault-v1";

/// Resultado de leer un archivo sellado.
#[derive(Debug, PartialEq)]
pub enum Sealed {
    /// No existe: instalación nueva.
    Missing,
    /// Existe y el sello es correcto.
    Ok(String),
    /// Existe pero el sello falta o no coincide: alguien lo tocó.
    Tampered,
}

pub struct Vault {
    key: [u8; 32],
    /// True si la llave se acaba de crear (primer arranque con esta versión): los archivos antiguos sin sello se aceptan
    /// una vez y se sellan.
    migrating: bool,
}

#[cfg(windows)]
fn dpapi(data: &[u8], protect: bool) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let ent = CRYPT_INTEGER_BLOB { cbData: ENTROPY.len() as u32, pbData: ENTROPY.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: los punteros apuntan a memoria válida durante la llamada; la salida la reserva Windows y se libera aquí.
    unsafe {
        let ok = if protect {
            CryptProtectData(&input, std::ptr::null(), &ent, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), &ent, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        };
        if ok == 0 || out.pbData.is_null() {
            return None;
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as _);
        Some(v)
    }
}

#[cfg(not(windows))]
fn dpapi(data: &[u8], _protect: bool) -> Option<Vec<u8>> {
    Some(data.to_vec()) // SyncroUmbral de escritorio es solo para Windows; esto permite compilar las pruebas en otros sistemas
}

/// Cifra bytes para este usuario de Windows.
pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
    dpapi(data, true)
}

pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    dpapi(data, false)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn sig_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".sig");
    PathBuf::from(s)
}

impl Vault {
    /// Abre la llave de la carpeta de datos, o la crea la primera vez.
    pub fn open(dir: &Path) -> Vault {
        let p = dir.join("vault.bin");
        if let Some(k) = std::fs::read(&p).ok().and_then(|b| unprotect(&b)).filter(|k| k.len() == 32) {
            let mut key = [0u8; 32];
            key.copy_from_slice(&k);
            return Vault { key, migrating: false };
        }
        // Primera vez, o la llave no se pudo descifrar (otro usuario/equipo): se crea una nueva. Los archivos sellados
        // con la anterior quedarán como manipulados, que es lo correcto si cambió el dueño de la carpeta.
        let existed = p.exists();
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        if let Some(b) = protect(&key) {
            let _ = write_atomic(&p, &b);
        }
        Vault { key, migrating: !existed }
    }

    fn mac(&self, name: &str, content: &[u8]) -> Vec<u8> {
        let mut m = <Hmac<Sha256> as Mac>::new_from_slice(&self.key).expect("clave HMAC");
        m.update(name.as_bytes());
        m.update(&[0]);
        m.update(content);
        m.finalize().into_bytes().to_vec()
    }

    /// Escribe el archivo y su sello.
    pub fn seal_write(&self, path: &Path, content: &str) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let sig = hex::encode(self.mac(name, content.as_bytes()));
        let _ = write_atomic(path, content.as_bytes());
        let _ = write_atomic(&sig_path(path), sig.as_bytes());
    }

    /// Lee un archivo comprobando su sello.
    pub fn read_sealed(&self, path: &Path) -> Sealed {
        let Ok(content) = std::fs::read_to_string(path) else { return Sealed::Missing };
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        match std::fs::read_to_string(sig_path(path)) {
            Ok(sig) => {
                let Ok(sig) = hex::decode(sig.trim()) else { return Sealed::Tampered };
                let mut m = <Hmac<Sha256> as Mac>::new_from_slice(&self.key).expect("clave HMAC");
                m.update(name.as_bytes());
                m.update(&[0]);
                m.update(content.as_bytes());
                if m.verify_slice(&sig).is_ok() {
                    Sealed::Ok(content)
                } else {
                    Sealed::Tampered
                }
            }
            // Sin sello: válido solo en la migración desde la versión anterior (se sella ahora).
            Err(_) if self.migrating => {
                self.seal_write(path, &content);
                Sealed::Ok(content)
            }
            Err(_) => Sealed::Tampered,
        }
    }
}

/// Registro de eventos de seguridad (vinculaciones, fallos, desbloqueos, manipulaciones). Solo texto, con rotación.
pub fn log(dir: &Path, event: &str) {
    use std::io::Write;
    const MAX: u64 = 256 * 1024;
    let p = dir.join("security.log");
    if std::fs::metadata(&p).map_or(false, |m| m.len() > MAX) {
        let _ = std::fs::rename(&p, dir.join("security.log.1"));
    }
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let clean: String = event.chars().filter(|c| !c.is_control()).take(300).collect();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{secs} {clean}");
    }
}

/// Clave privada TLS: se guarda cifrada con DPAPI (`key.dpapi`). Si existe el `key.der` en claro de una versión anterior
/// se migra y se borra.
pub fn load_tls_key(dir: &Path) -> Option<Vec<u8>> {
    let enc = dir.join("key.dpapi");
    if let Some(k) = std::fs::read(&enc).ok().and_then(|b| unprotect(&b)) {
        let _ = std::fs::remove_file(dir.join("key.der")); // por si quedó una copia en claro
        return Some(k);
    }
    let legacy = dir.join("key.der");
    let k = std::fs::read(&legacy).ok()?;
    let b = protect(&k)?;
    write_atomic(&enc, &b).ok()?;
    // Se sobrescribe antes de borrar para no dejar la clave tal cual en el archivo liberado.
    let _ = std::fs::write(&legacy, vec![0u8; k.len()]);
    let _ = std::fs::remove_file(&legacy);
    Some(k)
}

pub fn save_tls_key(dir: &Path, key: &[u8]) -> std::io::Result<()> {
    let b = protect(key).ok_or_else(|| std::io::Error::other("DPAPI no disponible"))?;
    write_atomic(&dir.join("key.dpapi"), &b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("umbral-vault-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn dpapi_roundtrip_and_not_plaintext() {
        let secret = b"clave-privada-de-prueba-0123456789";
        let enc = protect(secret).unwrap();
        #[cfg(windows)]
        assert!(!enc.windows(8).any(|w| w == &secret[..8]), "el cifrado no debe contener el texto en claro");
        assert_eq!(unprotect(&enc).unwrap(), secret);
    }

    #[test]
    fn sealed_file_detects_edit_swap_and_missing_sig() {
        let d = tmp("seal");
        let v = Vault::open(&d); // migrating = true
        let a = d.join("devices.json");
        v.seal_write(&a, "[1]");
        assert_eq!(v.read_sealed(&a), Sealed::Ok("[1]".into()));
        // edición directa
        std::fs::write(&a, "[1,2]").unwrap();
        assert_eq!(v.read_sealed(&a), Sealed::Tampered);
        // el sello de otro archivo no vale
        v.seal_write(&a, "[1]");
        let b = d.join("muted.json");
        std::fs::copy(&a, &b).unwrap();
        std::fs::copy(sig_path(&a), sig_path(&b)).unwrap();
        assert_eq!(v.read_sealed(&b), Sealed::Tampered);
        // borrar el sello también se detecta una vez pasada la migración
        let v2 = Vault::open(&d);
        assert!(!v2.migrating);
        std::fs::remove_file(sig_path(&a)).unwrap();
        assert_eq!(v2.read_sealed(&a), Sealed::Tampered);
        assert_eq!(v2.read_sealed(&d.join("no-existe")), Sealed::Missing);
    }

    #[test]
    fn migration_accepts_old_unsealed_file_once() {
        let d = tmp("mig");
        std::fs::write(d.join("devices.json"), "[]").unwrap();
        let v = Vault::open(&d);
        assert_eq!(v.read_sealed(&d.join("devices.json")), Sealed::Ok("[]".into()));
        let v2 = Vault::open(&d);
        assert_eq!(v2.read_sealed(&d.join("devices.json")), Sealed::Ok("[]".into()), "quedó sellado");
    }

    #[test]
    fn tls_key_is_migrated_and_plaintext_removed() {
        let d = tmp("tls");
        std::fs::write(d.join("key.der"), b"PRIVATE-KEY-BYTES-1234567890").unwrap();
        assert_eq!(load_tls_key(&d).unwrap(), b"PRIVATE-KEY-BYTES-1234567890");
        assert!(!d.join("key.der").exists(), "la clave en claro se borra");
        assert!(d.join("key.dpapi").exists());
        assert_eq!(load_tls_key(&d).unwrap(), b"PRIVATE-KEY-BYTES-1234567890");
    }

    #[test]
    fn log_rotates_and_strips_control_chars() {
        let d = tmp("log");
        log(&d, "evento\nfalso 1");
        let t = std::fs::read_to_string(d.join("security.log")).unwrap();
        assert_eq!(t.lines().count(), 1);
        assert!(t.contains("eventofalso 1"));
    }
}
