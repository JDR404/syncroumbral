//! SyncroUmbral con interfaz nativa Slint (sin WebView). Comparte con la versión Tauri todo el agente
//! (servidor TLS, estado, transferencias, audio...), que vive en `desktop/src-tauri/src`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

#[path = "../../desktop/src-tauri/src/adb.rs"]
mod adb;
#[path = "../../desktop/src-tauri/src/api.rs"]
mod api;
#[path = "../../desktop/src-tauri/src/appwin.rs"]
mod appwin;
#[path = "../../desktop/src-tauri/src/audio.rs"]
mod audio;
#[path = "../../desktop/src-tauri/src/autostart.rs"]
mod autostart;
#[path = "../../desktop/src-tauri/src/boot.rs"]
mod boot;
#[path = "../../desktop/src-tauri/src/core.rs"]
mod core;
#[path = "../../desktop/src-tauri/src/discovery.rs"]
mod discovery;
#[path = "../../desktop/src-tauri/src/limits.rs"]
mod limits;
#[path = "../../desktop/src-tauri/src/places.rs"]
mod places;
#[path = "../../desktop/src-tauri/src/ring.rs"]
mod ring;
#[path = "../../desktop/src-tauri/src/server.rs"]
mod server;
#[path = "../../desktop/src-tauri/src/services.rs"]
mod services;
#[path = "../../desktop/src-tauri/src/share.rs"]
mod share;
#[path = "../../desktop/src-tauri/src/tools.rs"]
mod tools;
#[path = "../../desktop/src-tauri/src/transfer.rs"]
mod transfer;
#[path = "../../desktop/src-tauri/src/ui.rs"]
mod ui;
#[path = "../../desktop/src-tauri/src/vault.rs"]
mod vault;

mod glue;
#[cfg(test)]
mod glue_tests;
mod shell;

slint::include_modules!();

fn main() {
    shell::run();
}
