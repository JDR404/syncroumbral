//! Carpetas conocidas de Windows (Escritorio, Descargas, Documentos, Imágenes) para acceder a ellas con un clic.
//!
//! Se piden a Windows (`SHGetKnownFolderPath`) en vez de armar la ruta a mano: con OneDrive activado el Escritorio real es
//! `C:\Users\<usuario>\OneDrive\Escritorio` y `C:\Users\<usuario>\Desktop` es una carpeta antigua casi vacía.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Place {
    pub name: &'static str,
    pub path: String,
}

#[cfg(windows)]
fn known(id: &windows::core::GUID) -> Option<String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    // SAFETY: Windows devuelve una cadena propia que se copia y se libera con CoTaskMemFree.
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

/// Las carpetas que existen, en un orden fijo.
pub fn places() -> Vec<Place> {
    #[cfg(windows)]
    let list: Vec<(&'static str, Option<String>)> = {
        use windows::Win32::UI::Shell::{
            FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Pictures, FOLDERID_Profile,
        };
        vec![
            ("Inicio", known(&FOLDERID_Profile)),
            ("Escritorio", known(&FOLDERID_Desktop)),
            ("Descargas", known(&FOLDERID_Downloads)),
            ("Documentos", known(&FOLDERID_Documents)),
            ("Imágenes", known(&FOLDERID_Pictures)),
        ]
    };
    #[cfg(not(windows))]
    let list: Vec<(&'static str, Option<String>)> = vec![("Inicio", std::env::var("HOME").ok())];

    list.into_iter()
        .filter_map(|(name, path)| path.map(|p| Place { name, path: p }))
        .filter(|p| std::path::Path::new(&p.path).is_dir())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_folders_exist_and_desktop_follows_onedrive_if_redirected() {
        let p = places();
        assert!(p.iter().any(|x| x.name == "Inicio"), "al menos la carpeta del usuario: {p:?}");
        assert!(p.iter().all(|x| std::path::Path::new(&x.path).is_dir()));
        #[cfg(windows)]
        if let Some(d) = p.iter().find(|x| x.name == "Escritorio") {
            // Si el Escritorio está en OneDrive, la ruta devuelta es la de OneDrive y no `...\Desktop`.
            let onedrive = std::env::var("OneDrive").unwrap_or_default();
            if !onedrive.is_empty() && d.path.starts_with(&onedrive) {
                assert!(!d.path.ends_with("\\Desktop") || d.path.starts_with(&onedrive));
            }
        }
    }
}
