//! Silencia el resto del PC mientras suena una llamada, para que se oiga solo el timbre de SyncroUmbral.
//!
//! Usa Core Audio de Windows: recorre las sesiones de audio del dispositivo de salida predeterminado (cada programa que
//! reproduce sonido es una sesión, la misma lista que el mezclador de volumen) y silencia las que no son de SyncroUmbral ni del
//! "sonido del sistema". Solo toca las que estaban sonando: lo que el usuario ya tenía en silencio se deja como estaba, y al
//! terminar se restaura exactamente lo que se silenció aquí. No cambia el volumen general ni el de ninguna aplicación.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

/// Archivo donde se anotan las sesiones silenciadas aquí. Windows recuerda el silencio de cada programa aunque SyncroUmbral
/// se cierre o se detenga a la fuerza en plena llamada, así que al arrancar se restaura lo que quedó anotado.
static JOURNAL: OnceLock<std::path::PathBuf> = OnceLock::new();

/// Se llama al arrancar con la carpeta de datos: restaura lo que una ejecución anterior dejó silenciado y, una sola vez tras
/// actualizar desde versiones sin este registro, quita el silencio de todas las sesiones (el usuario las veía mudas).
pub fn init(dir: &std::path::Path) {
    let _ = JOURNAL.set(dir.join("audio-silenciadas.txt"));
    #[cfg(windows)]
    std::thread::spawn({
        let marker = dir.join("audio-rescate-1");
        move || imp::recover(!marker.exists(), &marker)
    });
}

/// Mientras `active` sea true, mantiene silenciado el resto del PC; al volverse false restaura todo.
pub fn duck_while(active: Arc<AtomicBool>) {
    #[cfg(windows)]
    std::thread::spawn(move || imp::run(&active));
    #[cfg(not(windows))]
    let _ = active;
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::time::Duration;
    use windows::core::Interface;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume,
        MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    fn session_id(ctl2: &IAudioSessionControl2) -> Option<String> {
        // SAFETY: la cadena la reserva COM y se libera aquí tras copiarla.
        unsafe {
            let p = ctl2.GetSessionInstanceIdentifier().ok()?;
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as *const _));
            s
        }
    }

    fn journal_write(ids: &[String]) {
        if let Some(p) = JOURNAL.get() {
            if ids.is_empty() {
                let _ = std::fs::remove_file(p);
            } else {
                let _ = std::fs::write(p, ids.join("\n"));
            }
        }
    }

    /// Quita el silencio de las sesiones anotadas (o de todas con `all`) en el dispositivo predeterminado.
    pub fn unmute_listed(ids: &[String], all: bool) {
        // SAFETY: llamadas COM sobre interfaces válidas dentro del hilo que inicializó COM.
        unsafe {
            let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return };
            let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) else { return };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return };
            let Ok(sessions) = manager.GetSessionEnumerator() else { return };
            for i in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(ctl) = sessions.GetSession(i) else { continue };
                let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else { continue };
                if ctl2.IsSystemSoundsSession().0 == 0 {
                    continue;
                }
                let listed = session_id(&ctl2).map_or(false, |s| ids.contains(&s));
                if all || listed {
                    if let Ok(vol) = ctl.cast::<ISimpleAudioVolume>() {
                        let _ = vol.SetMute(false, std::ptr::null());
                    }
                }
            }
        }
    }

    /// Al arrancar: restaura lo anotado por una ejecución anterior y, con `rescue`, todo lo que estuviera en silencio.
    pub fn recover(rescue: bool, marker: &std::path::Path) {
        let ids: Vec<String> = JOURNAL
            .get()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| t.lines().map(String::from).collect())
            .unwrap_or_default();
        if ids.is_empty() && !rescue {
            return;
        }
        // SAFETY: COM se inicializa y se libera en este hilo.
        unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                return;
            }
            unmute_listed(&ids, rescue);
            CoUninitialize();
        }
        journal_write(&[]);
        if rescue {
            let _ = std::fs::write(marker, "1");
        }
    }

    /// Un recorrido: silencia las sesiones ajenas que suenan y las añade a `muted`. Devuelve cuántas silenció.
    pub fn mute_pass(me: u32, muted: &mut Vec<ISimpleAudioVolume>) -> windows::core::Result<usize> {
        let mut ids = Vec::new();
        let r = mute_pass_ids(me, muted, &mut ids);
        if !ids.is_empty() {
            let mut all: Vec<String> = JOURNAL
                .get()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|t| t.lines().map(String::from).collect())
                .unwrap_or_default();
            all.extend(ids);
            journal_write(&all);
        }
        r
    }

    fn mute_pass_ids(me: u32, muted: &mut Vec<ISimpleAudioVolume>, ids: &mut Vec<String>) -> windows::core::Result<usize> {
        // SAFETY: llamadas COM sobre interfaces válidas dentro del hilo que inicializó COM.
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            let sessions = manager.GetSessionEnumerator()?;
            let mut n = 0;
            for i in 0..sessions.GetCount()? {
                let ctl = sessions.GetSession(i)?;
                let ctl2: IAudioSessionControl2 = ctl.cast()?;
                if ctl2.IsSystemSoundsSession().0 == 0 {
                    continue; // el sonido del sistema (incluye avisos) no se toca
                }
                if ctl2.GetProcessId().unwrap_or(0) == me {
                    continue; // el timbre es de SyncroUmbral
                }
                let vol: ISimpleAudioVolume = ctl.cast()?;
                if vol.GetMute()?.as_bool() {
                    continue; // ya estaba en silencio (por el usuario o por una pasada anterior)
                }
                vol.SetMute(true, std::ptr::null())?;
                if let Some(id) = session_id(&ctl2) {
                    ids.push(id);
                }
                muted.push(vol);
                n += 1;
            }
            Ok(n)
        }
    }

    pub fn restore(muted: &mut Vec<ISimpleAudioVolume>) {
        for v in muted.drain(..) {
            // SAFETY: `v` es una interfaz de sesión válida creada en este mismo hilo.
            unsafe {
                let _ = v.SetMute(false, std::ptr::null());
            }
        }
    }

    pub fn run(active: &AtomicBool) {
        // SAFETY: COM se inicializa y se libera en este hilo; las interfaces no salen de él.
        unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                return;
            }
            let mut muted: Vec<ISimpleAudioVolume> = Vec::new();
            let me = std::process::id();
            while active.load(Ordering::SeqCst) {
                let _ = mute_pass(me, &mut muted); // otra pasada por si alguien empieza a sonar durante la llamada
                std::thread::sleep(Duration::from_millis(400));
            }
            restore(&mut muted);
            journal_write(&[]);
            CoUninitialize();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Con un dispositivo de audio: tras una pasada todo lo ajeno que sonaba queda en silencio y se restaura al terminar.
        #[test]
        fn mutes_foreign_sessions_and_restores_them() {
            unsafe {
                if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                    return;
                }
            }
            let mut muted = Vec::new();
            match mute_pass(0, &mut muted) {
                Err(e) => {
                    eprintln!("sin dispositivo de audio utilizable: {e}");
                    return;
                }
                Ok(n) => {
                    eprintln!("sesiones ajenas silenciadas: {n}");
                    assert_eq!(n, muted.len());
                }
            }
            for v in &muted {
                assert!(unsafe { v.GetMute().unwrap().as_bool() }, "silenciada durante la llamada");
            }
            // una segunda pasada no vuelve a contar las ya silenciadas
            let mut again = Vec::new();
            assert_eq!(mute_pass(0, &mut again).unwrap(), 0);
            let snapshot: Vec<ISimpleAudioVolume> = muted.clone();
            restore(&mut muted);
            for v in &snapshot {
                assert!(!unsafe { v.GetMute().unwrap().as_bool() }, "restaurada al terminar");
            }
        }
    }
}
