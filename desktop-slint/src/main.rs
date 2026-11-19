//! Prototipo de la interfaz nativa de SyncroUmbral con Slint (sin WebView).
//!
//! Solo es la interfaz: el panel "Este PC" lista carpetas reales; el móvil, las notificaciones y la clave son de demostración
//! (la clave de prueba es `demo`). Sirve para medir memoria y probar el diseño antes de conectarla al agente de Rust.
//!
//! Opciones: `--open` arranca desbloqueado, `--page N` abre la pestaña N (0 archivos, 1 notificaciones, 2 ajustes).

use slint::{Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

slint::include_modules!();

fn fmt_size(n: u64) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    let units = ["KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    loop {
        v /= 1024.0;
        if v < 1024.0 || i == units.len() - 1 {
            break;
        }
        i += 1;
    }
    if v >= 100.0 { format!("{v:.0} {}", units[i]) } else { format!("{v:.1} {}", units[i]) }
}

/// Carpetas primero, luego archivos, por nombre; sin los que empiezan por punto.
fn list_dir(path: &PathBuf) -> Vec<FileItem> {
    let mut items: Vec<FileItem> = std::fs::read_dir(path)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        return None;
                    }
                    let md = e.metadata().ok()?;
                    Some(FileItem {
                        name: name.into(),
                        meta: if md.is_dir() { SharedString::new() } else { fmt_size(md.len()).into() },
                        dir: md.is_dir(),
                        selected: false,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    items.sort_by(|a, b| b.dir.cmp(&a.dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    items
}

fn phone_demo() -> Vec<FileItem> {
    let f = |name: &str, meta: &str, dir: bool| FileItem { name: name.into(), meta: meta.into(), dir, selected: false };
    vec![
        f("DCIM", "", true),
        f("Download", "", true),
        f("Pictures", "", true),
        f("un_nombre_de_archivo_extremadamente_largo_para_probar_el_recorte_de_texto.pdf", "118 MB", false),
        f("app.apk", "42.9 MB", false),
    ]
}

fn notifs_demo() -> Vec<Notif> {
    let n = |app: &str, title: &str, text: &str, time: &str| Notif { app: app.into(), title: title.into(), text: text.into(), time: time.into() };
    vec![
        n("WhatsApp", "Ana", "Hola, ¿vienes hoy?", "hace 2 min"),
        n("Telegram", "Equipo", "La versión 0.2.0 ya está en GitHub", "hace 9 min"),
        n("Calendario", "Reunión a las 10:00", "Sala principal", "hace 1 h"),
    ]
}

/// Construye la ventana con sus datos y sus acciones (separada de `main` para poder probarla sin abrir nada).
fn build_ui(open: bool, page: i32, small: bool) -> Result<AppWindow, slint::PlatformError> {
    let ui = AppWindow::new()?;

    let home = PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".into()));
    let cwd = Rc::new(RefCell::new(home.clone()));
    let pc = Rc::new(VecModel::from(list_dir(&home)));
    let phone = Rc::new(VecModel::from(phone_demo()));
    let notifs = Rc::new(VecModel::from(notifs_demo()));

    ui.set_pc_files(ModelRc::from(pc.clone()));
    ui.set_phone_files(ModelRc::from(phone.clone()));
    ui.set_notifs(ModelRc::from(notifs.clone()));
    ui.set_pc_footer(format!("{} elementos", pc.row_count()).into());
    ui.set_phone_footer("46.6 GB libres".into());
    ui.set_locked(!open);
    ui.set_page(page);
    if small {
        ui.window().set_size(slint::LogicalSize::new(860.0, 560.0)); // tamaño mínimo: el panel lateral se pliega solo
    }

    ui.on_unlock({
        let ui = ui.as_weak();
        move |pw| {
            let ui = ui.unwrap();
            if pw.as_str() == "demo" {
                ui.set_lock_error(SharedString::new());
                ui.set_locked(false);
            } else {
                ui.set_lock_error("Clave incorrecta (la de prueba es: demo)".into());
            }
        }
    });
    ui.on_lock_now({
        let ui = ui.as_weak();
        move || ui.unwrap().set_locked(true)
    });
    ui.on_toggle_file({
        let (pc, phone) = (pc.clone(), phone.clone());
        move |is_phone, i| {
            let model: &VecModel<FileItem> = if is_phone { &phone } else { &pc };
            if let Some(mut it) = model.row_data(i as usize) {
                it.selected = !it.selected;
                model.set_row_data(i as usize, it);
            }
        }
    });
    ui.on_open_file({
        let (pc, cwd, ui) = (pc.clone(), cwd.clone(), ui.as_weak());
        move |is_phone, i| {
            if is_phone {
                return;
            }
            let Some(it) = pc.row_data(i as usize) else { return };
            if !it.dir {
                return;
            }
            let next = cwd.borrow().join(it.name.as_str());
            let items = list_dir(&next);
            *cwd.borrow_mut() = next;
            ui.unwrap().set_pc_footer(format!("{} elementos", items.len()).into());
            pc.set_vec(items);
        }
    });
    ui.on_dismiss_notif({
        let notifs = notifs.clone();
        move |i| {
            notifs.remove(i as usize);
        }
    });
    ui.on_clear_notifs({
        let notifs = notifs.clone();
        move || notifs.set_vec(Vec::<Notif>::new())
    });

    Ok(ui)
}

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().collect();
    let open = args.iter().any(|a| a == "--open");
    let small = args.iter().any(|a| a == "--small");
    let page = args.iter().position(|a| a == "--page").and_then(|i| args.get(i + 1)).and_then(|p| p.parse::<i32>().ok()).unwrap_or(0);
    build_ui(open, page, small)?.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_lock_select_and_dismiss() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = build_ui(false, 0, false).unwrap();
        assert!(ui.get_locked(), "arranca bloqueada");

        ui.invoke_unlock("mala".into());
        assert!(ui.get_locked());
        assert!(!ui.get_lock_error().is_empty(), "una clave mala muestra el error");

        ui.invoke_unlock("demo".into());
        assert!(!ui.get_locked());
        assert!(ui.get_lock_error().is_empty());

        // Seleccionar y deseleccionar un archivo del PC.
        ui.invoke_toggle_file(false, 0);
        assert!(ui.get_pc_files().row_data(0).unwrap().selected);
        ui.invoke_toggle_file(false, 0);
        assert!(!ui.get_pc_files().row_data(0).unwrap().selected);

        // Notificaciones: descartar una y limpiar el resto.
        assert_eq!(ui.get_notifs().row_count(), 3);
        ui.invoke_dismiss_notif(0);
        assert_eq!(ui.get_notifs().row_count(), 2);
        ui.invoke_clear_notifs();
        assert_eq!(ui.get_notifs().row_count(), 0);

        ui.invoke_lock_now();
        assert!(ui.get_locked(), "Bloquear ahora vuelve a la pantalla de bloqueo");
    }
}
