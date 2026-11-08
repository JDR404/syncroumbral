//! Timbre de las llamadas entrantes: un sonido repetido en el PC hasta que la llamada se contesta, se rechaza o se corta.
//! El estado vive en cada instancia (un `Arc<AtomicBool>`), no en una variable global.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[cfg(windows)]
extern "system" {
    fn Beep(freq: u32, ms: u32) -> i32;
}

fn tone(freq: u32, ms: u32) {
    #[cfg(windows)]
    unsafe {
        Beep(freq, ms);
    }
    #[cfg(not(windows))]
    {
        let _ = (freq, ms);
    }
}

/// `mute_others`: mientras suena, el resto de sonidos del PC se silencia (ver audio.rs) y se restaura al parar.
pub fn start(flag: &Arc<AtomicBool>, mute_others: bool) {
    if flag.swap(true, Ordering::SeqCst) {
        return; // ya está sonando
    }
    if mute_others {
        crate::audio::duck_while(flag.clone());
    }
    let flag = flag.clone();
    std::thread::spawn(move || {
        // Como máximo ~45 s: una llamada que nadie atiende no debe sonar para siempre.
        for _ in 0..30 {
            if !flag.load(Ordering::SeqCst) {
                return;
            }
            tone(880, 220);
            tone(660, 220);
            std::thread::sleep(Duration::from_millis(1200));
        }
        flag.store(false, Ordering::SeqCst);
    });
}

pub fn stop(flag: &Arc<AtomicBool>) {
    flag.store(false, Ordering::SeqCst);
}
