# Cambios

## 0.3.0 — 2026-11-19

### Interfaz nativa con Slint (sin WebView)
- **SyncroUmbral nativo** en [`desktop-slint/`](desktop-slint): un proceso y ~8 MB privados con la ventana abierta y el móvil conectado
  (la versión Tauri usa ~230 MB en 7 procesos). Misma carpeta de datos y mismos móviles vinculados. Ver [docs/SLINT.md](docs/SLINT.md).
- Archivos, notificaciones con respuesta y controles del reproductor, llamadas, aplicaciones del móvil en el PC, mensajería en una ventana
  aislada de Edge, vinculación por QR, ajustes, aviso emergente, bandeja e instancia única.
- Instalador de usuario: `desktop-slint/instalar.ps1`.

### Agente
- El agente ya no depende de Tauri: nuevo trait `Ui` (`ui.rs`) y módulos compartidos (`api`, `boot`, `autostart`, `services`). La versión
  Tauri lo implementa con `tauri_ui.rs`. Sin cambios de comportamiento: las 43 pruebas siguen pasando.

## 0.2.0 — 2026-11-17

### Nuevo
- **Botones Bloquear y Desbloquear en la notificación del móvil** (y en "Acciones rápidas" de la app): se bloquea o desbloquea SyncroUmbral
  del PC sin abrir la app. Desbloquear exige tener el teléfono desbloqueado. Nuevo mensaje `unlock_pc` en el [protocolo](docs/PROTOCOLO.md),
  atendido solo dentro de una sesión autenticada y con el móvil presente.
- Listas de archivos y apps manejables con teclado (flechas, Espacio, Intro); Escape cierra los diálogos; roles ARIA y foco visible.

### Interfaz del PC
- El panel lateral se pliega solo en ventanas estrechas (860 px ya era inutilizable en Archivos).
- Nombres de archivo legibles: tamaño y fecha en dos líneas, nombre completo al pasar el ratón.
- Barras de portapapeles y de Aplicaciones que se ajustan en vez de partir el texto; marca "SyncroUmbral" ya no se corta.
- El aviso "El móvil no está conectado" se lee entero dentro del panel.
- La pantalla de llamada respeta el panel plegado.
- Los avisos periódicos (mensajería web, apps abiertas) se pausan con la ventana oculta; se respeta "reducir movimiento".

### App Android
- Los márgenes respetan las barras del sistema (Android 15 dibuja a pantalla completa): la barra inferior ya no choca con los gestos.
- Se conserva la sección abierta al girar el móvil.
- Si el PC no aparece, la espera entre reintentos crece de 3 s a 30 s (antes, cada 3 s sin parar: gastaba batería con el PC apagado).
- Los hilos de peticiones se cierran al detener la conexión.

### Audio (corrección importante)
- El silenciado del resto de sonidos durante una llamada ya **no toca mejoradores o mezcladores de audio** (FxSound, Voicemeeter...): por
  ellos pasa todo el audio del PC y silenciarlos lo dejaba mudo, también el timbre.
- Las pruebas automáticas ya no pueden silenciar el audio real (antes, al terminar el programa de pruebas, el hilo se cortaba antes de
  restaurar y dejaba sesiones mudas). Las dos pruebas que lo usan de verdad son `#[ignore]`.
- Nueva prueba de reparación: `cargo test --manifest-path desktop/src-tauri/Cargo.toml repair_audio -- --ignored`.

### Documentación
- README reescrito (cómo funciona, estructura, compilar y probar, hoja de ruta) y nuevo [docs/ARQUITECTURA.md](docs/ARQUITECTURA.md).

### Verificación
- 43 pruebas automáticas del PC pasan (3 ignoradas a propósito por tocar el audio real).
- App Android de producción firmada, no depurable, probada en un Pixel 9 Pro con Android 17: vinculación, conexión estable y los dos botones
  de la notificación contra el PC.

## 0.1.0 — 2026-11-07

Primera versión: presencia y bloqueo, archivos, compartir, notificaciones con respuesta, llamadas, apps del móvil en el PC,
mensajería web, modos básico y completo, y la auditoría de seguridad inicial.
