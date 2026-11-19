# Interfaz nativa con Slint (prototipo)

Carpeta: [`desktop-slint/`](../desktop-slint). Es un crate **independiente**: no toca la app actual (`desktop/`), que sigue siendo la que se instala.

## Por qué

La interfaz actual es HTML dentro de WebView2 (Edge). Cada vez que se abre la ventana aparecen varios procesos de Edge. Slint dibuja la
interfaz nativamente, en un solo proceso, **por software** (sin OpenGL ni GPU), así que debería funcionar en cualquier PC con poca memoria.

## Medición (2026-11-17, Windows 11, mismo PC)

| | Procesos | Memoria privada | Memoria de trabajo* | Ejecutable |
|---|---|---|---|---|
| App actual (Tauri + WebView2), solo agente en la bandeja | 1 | 7–8 MB | 44 MB | 5,4 MB |
| App actual con la **ventana abierta** | 7 | **230 MB** | **419 MB** | — |
| Prototipo Slint con la ventana abierta | 1 | **5–6 MB** | **19–22 MB** | 8,1 MB |

\* La memoria de trabajo cuenta páginas compartidas entre los procesos de WebView2, así que la cifra de la app actual está inflada; la
**memoria privada** es la comparación más justa. Medido con `Process.WorkingSet64` y `PrivateMemorySize64`.

**Ojo:** el prototipo es solo la interfaz. Al conectarle el agente (servidor TLS, estado, transferencias, tokio) subirá. Una estimación
razonable es que la app completa quede en unas decenas de MB (hoy el agente solo ocupa 44 MB de trabajo, de los que una parte es el motor
de Tauri que se sustituye), pero **eso hay que medirlo cuando esté conectado**.

## Qué hay

- Pantalla de bloqueo, panel lateral (se pliega solo por debajo de 980 px), **Archivos** (dos paneles; "Este PC" lista carpetas reales), **Notificaciones**
  (descartar y limpiar) y **Ajustes** (interruptores).
- Misma paleta teal oscura que la app actual.
- Todo lo del móvil (archivos, notificaciones) y la clave (`demo`) son datos de demostración.
- Una prueba automática sin ventana (`cargo test`) comprueba desbloquear, bloquear, seleccionar y descartar.

## Cómo probarlo

```
cd desktop-slint
cargo run --release                          # arranca bloqueado (clave de prueba: demo)
cargo run --release -- --open --page 1       # abierto, en Notificaciones (0 archivos, 1 notificaciones, 2 ajustes)
cargo run --release -- --open --small        # tamaño mínimo 860x560, para ver el panel plegado
cargo test
```

## Qué falta para sustituir a la interfaz actual

| # | Tarea | Nota |
|---|---|---|
| 1 | **Separar el agente de Tauri.** Hoy `core.rs`, `toast.rs`, `shell.rs`, `web.rs`, `appwin.rs` usan `AppHandle` de Tauri para emitir eventos y abrir ventanas | Extraer una biblioteca (`syncroumbral-core`) con un `trait` de eventos; Tauri y Slint serían dos "frontends" |
| 2 | Conectar los comandos (`commands.rs`) y los eventos (`state`, `notif`, `transfer`, `mode`...) a Slint | `slint::invoke_from_event_loop` para actualizar desde otros hilos |
| 3 | Bandeja del sistema, inicio con Windows, una sola instancia | Hoy los da Tauri (`tray-icon` integrado y el plugin de instancia única): habría que usar el crate `tray-icon` directamente y reescribir esa parte de `shell.rs` |
| 4 | Ventana emergente de notificaciones y pantalla de llamada | Ventanas Slint pequeñas, sin bordes |
| 5 | Arrastrar y soltar archivos desde el Explorador | Eventos de `winit` |
| 6 | QR de vinculación | El agente ya genera el QR con el crate `qrcode` (como SVG); habría que dibujarlo como imagen |
| 7 | **Mensajería web** (WhatsApp, Telegram...) | Necesita un motor de navegador: abrirla bajo demanda en una ventana aislada de Edge (`msedge --app=URL --user-data-dir=...`) y cerrarla al bloquear. Es la única pieza que seguiría usando WebView2, y solo mientras se use |
| 8 | Aplicaciones del móvil y herramientas (adb, scrcpy) | Solo hay que dibujar las listas; la lógica ya existe |
| 9 | Tipografía e iconos, interruptores con la paleta de la app | Hoy los interruptores usan el estilo por defecto de Slint |
| 10 | Revisar la **licencia de Slint** antes de distribuir | GPLv3, o gratuita para escritorio con atribución, o comercial |

## Decisión: por qué Slint y no egui

- Renderizado por software: funciona sin GPU y en máquinas virtuales (egui necesita OpenGL o wgpu).
- Interfaz declarativa y accesible (IME, lectores de pantalla); egui dibuja en modo inmediato y repinta más.
- Se parece más a la interfaz Material actual con poco código (este prototipo son unas 425 líneas de `.slint` y 197 de Rust).
- Coste: licencia más restrictiva que la de egui (MIT/Apache).
