//! Arranque del agente sin interfaz: carpeta de datos, certificado, TLS y estado. Lo usan Tauri y la interfaz Slint.

use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio_rustls::rustls::{
    self,
    pki_types::{CertificateDer, PrivatePkcs8KeyDer},
};

/// Certificado autofirmado del PC: se crea una vez y se reutiliza (la huella va en el QR).
/// La clave privada se guarda cifrada con DPAPI (ver vault.rs); una `key.der` en claro de la versión anterior se migra.
pub fn load_or_create_cert(dir: &std::path::Path) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let cp = dir.join("cert.der");
    if let (Ok(c), Some(k)) = (std::fs::read(&cp), crate::vault::load_tls_key(dir)) {
        return Ok((c, k));
    }
    let ck = rcgen::generate_simple_self_signed(vec!["umbral.local".to_string()])?;
    let (c, k) = (ck.cert.der().to_vec(), ck.key_pair.serialize_der());
    crate::vault::save_tls_key(dir, &k)?;
    std::fs::write(&cp, &c)?;
    Ok((c, k))
}

/// Migración desde el nombre anterior (Umbral, identificador `app.umbral.desktop`): si la carpeta de datos nueva aún no existe y
/// la antigua sí, se mueve entera (claves, móviles vinculados, sesiones web). Las claves van cifradas con DPAPI por usuario
/// y los sellos dependen del nombre de cada archivo, no de la ruta, así que siguen siendo válidos.
pub fn migrate_legacy_data(new_dir: &std::path::Path) {
    let Some(parent) = new_dir.parent() else { return };
    let old = parent.join("app.umbral.desktop");
    if new_dir.exists() || !old.is_dir() {
        return;
    }
    if std::fs::rename(&old, new_dir).is_err() {
        // Otra unidad o un archivo en uso: se copia y, si salió bien, se deja la antigua intacta como respaldo.
        let _ = copy_dir(&old, new_dir);
    }
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Carpeta de datos: `--data-dir=RUTA` (perfil aparte o modo portátil) o la de la aplicación. No se lee del entorno:
/// una variable de entorno heredada no debe poder redirigir dónde SyncroUmbral guarda sus claves.
pub fn data_dir_arg() -> Option<std::path::PathBuf> {
    std::env::args().find_map(|a| a.strip_prefix("--data-dir=").map(std::path::PathBuf::from))
}

pub fn tls_config(cert: Vec<u8>, key: Vec<u8>) -> anyhow::Result<rustls::ServerConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Ok(rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(vec![CertificateDer::from(cert)], PrivatePkcs8KeyDer::from(key).into())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_data_folder_is_moved_once_and_never_overwrites_the_new_one() {
        let base = std::env::temp_dir().join(format!("syncroumbral-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (old, new) = (base.join("app.umbral.desktop"), base.join("app.syncroapp.umbral"));
        std::fs::create_dir_all(old.join("web").join("whatsapp")).unwrap();
        std::fs::write(old.join("devices.json"), "[1]").unwrap();
        std::fs::write(old.join("web").join("whatsapp").join("cookie"), "sesion").unwrap();

        migrate_legacy_data(&new);
        assert_eq!(std::fs::read_to_string(new.join("devices.json")).unwrap(), "[1]");
        assert_eq!(std::fs::read_to_string(new.join("web").join("whatsapp").join("cookie")).unwrap(), "sesion");
        assert!(!old.exists(), "la antigua se movió");

        // Si la carpeta nueva ya existe no se toca (ni se mezcla con otra antigua que reaparezca).
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("devices.json"), "[999]").unwrap();
        migrate_legacy_data(&new);
        assert_eq!(std::fs::read_to_string(new.join("devices.json")).unwrap(), "[1]");
        let _ = std::fs::remove_dir_all(&base);
    }
}


/// Prepara todo lo del agente (herramientas, audio, certificado, estado) en la carpeta de datos.
pub fn init(dir: &std::path::Path) -> anyhow::Result<(Arc<crate::core::Core>, Arc<rustls::ServerConfig>)> {
    std::fs::create_dir_all(dir)?;
    crate::tools::init(dir);
    crate::audio::init(dir);
    let (cert, key) = load_or_create_cert(dir)?;
    let fp = hex::encode(Sha256::digest(&cert));
    let tls = Arc::new(tls_config(cert, key)?);
    let mut core = crate::core::Core::load(dir.to_path_buf(), fp);
    core.sync_notifs = true;
    Ok((Arc::new(core), tls))
}
