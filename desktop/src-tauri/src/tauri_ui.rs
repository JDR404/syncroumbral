//! La interfaz de Tauri (WebView2) como implementación de `Ui`.

use crate::ui::Ui;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

impl<R: Runtime> Ui for AppHandle<R> {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        let _ = Emitter::emit(self, event, payload);
    }
    fn toast_show(&self) {
        crate::toast::show(self);
    }
    fn toast_hide(&self) {
        crate::toast::hide(self);
    }
    fn close_web(&self) {
        crate::web::close_all(self);
    }
}
