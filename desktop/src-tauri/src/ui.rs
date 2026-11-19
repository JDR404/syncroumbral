//! Lo que el agente necesita de la interfaz, sin saber cuál es (Tauri/WebView o Slint nativa).
//! `core`, `server`, `share` y `transfer` solo hablan con este trait.

use serde::Serialize;

pub trait Ui: Clone + Send + Sync + 'static {
    /// Avisa a la interfaz de un cambio (`state`, `notif`, `notif_gone`, `notif_sync`, `notif_clear`, `mode`, `transfer`...).
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S);
    /// Muestra (o refresca) la ventana emergente de notificaciones.
    fn toast_show(&self);
    fn toast_hide(&self);
    /// Cierra las ventanas web (mensajería) al bloquear.
    fn close_web(&self);
}
