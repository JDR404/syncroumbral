# Interfaz nativa con Slint

Carpeta: [`desktop-slint/`](../desktop-slint). Es SyncroUmbral completo con interfaz nativa y **sin WebView**: un solo proceso, dibujado por
software (sin OpenGL ni GPU). Comparte con la versión Tauri (`desktop/`) **todo el agente**: servidor TLS, estado, transferencias, audio,
adb/scrcpy, claves y móviles vinculados (misma carpeta de datos, así que se puede pasar de una a otra sin volver a vincular).

## Medición (2026-11-19, Windows 11, mismo PC, Pixel 9 Pro conectado)

| | Procesos | Memoria privada | Memoria de trabajo* |
|---|---|---|---|
| Versión Tauri con la **ventana abierta** | 7 | **230 MB** | 419 MB |
| Versión Slint con la ventana abierta y el móvil conectado | 1 | **8 MB** | 34 MB |
| Versión Slint con la ventana cerrada (bandeja) | 1 | 8 MB | 28 MB |

\* La memoria de trabajo cuenta páginas compartidas entre procesos de WebView2; la privada es la comparación justa.

## Cómo está hecho

```
desktop/src-tauri/src/   agente (sin Tauri):  core, server, share, transfer, api, boot, autostart, services, audio, adb...
        ui.rs            trait `Ui`: lo único que el agente necesita de la interfaz (emit, toast_show, toast_hide, close_web)
        tauri_ui.rs      implementación para Tauri (WebView2)
desktop-slint/src/
        main.rs          incluye los módulos del agente con #[path] (no se copia código)
        glue.rs          implementación `Ui` para Slint y todos los callbacks de la interfaz
        shell.rs         arranque: instancia única, bandeja, ventana oculta al cerrar
desktop-slint/ui/        app.slint (ventana), notif.slint (tarjetas), toast.slint (aviso emergente), common.slint (paleta, botones, interruptores)
```

Si cambias algo del agente, las dos interfaces lo reciben: las 43 pruebas del agente corren con Tauri y la de `glue_tests.rs` ejercita el
agente real a través de la interfaz Slint (clave, archivos, notificaciones, QR, bloqueo).

## Qué tiene

- Pantalla de bloqueo (clave o móvil presente), crear clave la primera vez.
- **Archivos**: dos paneles (PC y móvil), enviar y traer con progreso y cancelar.
- **Notificaciones**: tarjetas con respuesta, botones del reproductor, llamadas (contestar y colgar), descartar, limpiar todas.
- **Aviso emergente** abajo a la derecha (sin robar el foco, sin icono en la barra de tareas, se cierra solo salvo en llamadas).
- **Aplicaciones del móvil**: lista, abrir en una ventana del PC (scrcpy), detener, ajustes de la app.
- **Mensajería** (WhatsApp, Instagram, Facebook, Messenger, Telegram): cada servicio en una ventana propia de Edge (`--app`) con perfil
  aislado y la sesión guardada; se cierra al bloquear. Es la única parte que usa un navegador, y solo mientras la abres.
- **Móviles y ajustes**: móviles vinculados, **QR de vinculación** con huella, inicio con Windows, silenciado en llamadas, modo completo,
  instalar adb y scrcpy.
- Bandeja del sistema (abrir, bloquear, salir), **una sola instancia** y cerrar la ventana la oculta.

## Instalar

```
cd desktop-slint
cargo build --release
powershell -ExecutionPolicy Bypass -File instalar.ps1
```

`instalar.ps1` sustituye a `syncroumbral.exe` en `%LOCALAPPDATA%\SyncroUmbral`, activa el inicio con Windows, crea el acceso directo y lo abre.

## Probar

```
cargo test --release                       # 18 pruebas (agente + interfaz sin ventana)
cargo build --release --features selftest  # añade --test-unlock=CLAVE --test-page=N --test-notif --test-qr para capturas
```

## Pendiente

- Arrastrar archivos desde el Explorador (winit lo permite; falta conectarlo).
- Iconos de las aplicaciones y fotos de los contactos en las tarjetas.
- Carátula del reproductor y cronómetro de la llamada en curso.
- Revisar la **licencia de Slint** antes de distribuir: GPLv3, o gratuita para escritorio con atribución, o comercial.

## Por qué Slint y no egui

- Renderizado por software: funciona sin GPU y en máquinas virtuales (egui necesita OpenGL o wgpu).
- Interfaz declarativa y accesible (IME, lectores de pantalla); egui dibuja en modo inmediato y repinta más.
- Se parece más a la interfaz Material actual con poco código.
- Coste: licencia más restrictiva que la de egui (MIT/Apache).
