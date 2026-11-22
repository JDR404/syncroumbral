# 01 · Estado actual de SyncroUmbral

Auditoría del 2026-11-22 sobre `main` tras publicar `v0.7.2`. Solo diagnóstico: no se modificó código de producción.

## Método y límites del entorno

- Se leyó el código, la configuración, los scripts, la CI, la documentación y el historial de Git (45 commits, 7 publicaciones).
- Se ejecutaron: `cargo test --release` (desktop-slint), `cargo audit`, `cargo test -- --list` y la medición del proceso instalado en el PC del autor (Windows 11 Pro 10.0.26300, x64).
- **No se pudo**: ejecutar en Linux ni macOS, compilar para esos sistemas (no hay *toolchains* instaladas y la auditoría no instala dependencias), probar el móvil Android de forma automática, ni lanzar procesos fuera del contenedor MSIX de la app de Claude (ver 03, hallazgo H-12).
- Notación: **[H]** hecho comprobado, **[I]** inferencia del código, **[D]** solo lo afirma la documentación.

## Qué es el producto

Una pareja de aplicaciones que hace del móvil Android la «llave» del PC y lo integra con él, en red local y sin nube:

- **PC (Windows)**: agente en Rust (servidor TLS con certificado fijado, protocolo propio por WebSocket, mDNS) con interfaz nativa **Slint** (renderizado por software, sin WebView). Bloquea la app (y opcionalmente Windows) cuando el móvil se va.
- **Móvil (Android 8+, `minSdk 26`, `targetSdk 35`)**: app Kotlin que mantiene la conexión, reenvía notificaciones, llamadas, SMS, archivos, galería y portapapeles.
- Referente explícito: Microsoft Phone Link (docs/FUNCIONES.md).
- Usuario objetivo: persona con un PC Windows y un Android en la misma red que quiere privacidad local y ligereza.

## Stack real

| Capa | Tecnología | Evidencia |
|---|---|---|
| Interfaz PC | Slint 1.19 (winit, `renderer-software`) | `desktop-slint/Cargo.toml` |
| Bandeja | `tray-icon` 0.21 | `desktop-slint/src/shell.rs:292-302` |
| Agente | Rust 2021, tokio, rustls 0.23, tokio-tungstenite, mdns-sd, p256, argon2, hmac | `desktop/src-tauri/Cargo.toml`, módulos incluidos con `#[path]` en `desktop-slint/src/main.rs` |
| APIs de Windows | `windows`/`windows-sys`: DPAPI, Core Audio, `LockWorkStation`, `SHGetKnownFolderPath`, área de trabajo | `vault.rs`, `audio.rs`, `glue.rs:1101,1808`, `places.rs` |
| Procesos externos | `reg`, `taskkill`, `explorer`, `rundll32`, Edge (`--app`), adb, scrcpy | grep `Command::new` |
| Móvil | Kotlin, Gradle, R8, firmado | `android/app/build.gradle.kts` |
| Versión Tauri | `desktop/src-tauri` (WebView2): **ya no se publica**, sigue en el repo y aloja el código del agente | README, SLINT.md |
| Distribución | GitHub Releases (pre-release), instalador NSIS por usuario, actualizador propio con firma ECDSA | `instalador/`, `glue.rs:1930-2010`, `tools/release-sign` |
| CI | GitHub Actions, solo Windows, solo en etiquetas `v*` | `.github/workflows/build-windows.yml` |

Tamaño: 181 archivos versionados, ~13 400 líneas de Rust/Slint/Kotlin/scripts. Los mayores: `glue.rs` 2 451, `app.slint` 1 051, `server_tests.rs` 853, `core.rs` 826, `MainActivity.kt` 756.

## Inventario funcional (matriz)

Estados: **Completa**, **Parcial**, **Defectuosa**, **No implementada**, **No verificable** (en este entorno). Prioridad P0–P3 del trabajo pendiente.

| Funcionalidad | Estado | Evidencia | Problemas | Prioridad |
|---|---|---|---|---|
| Vinculación por QR / código con huella | Completa | `core.rs`, `server.rs`; pruebas de servidor **no corren** en Slint (H-02) | Sin pruebas activas del protocolo | P1 |
| Bloqueo/desbloqueo por presencia | Completa [H en uso] | `glue.rs on_presence`, pantalla de bloqueo | Presencia = misma red, no cercanía (S-03) | P2 |
| Bloquear Windows al alejarse | Parcial | `glue.rs:1800-1812` | No probado el bloqueo real; solo Windows | P2 |
| Clave de respaldo (Argon2 + sello HMAC) | Completa | `vault.rs` (5 pruebas) | DPAPI solo Windows | — |
| Notificaciones con respuesta, acciones, 2FA | Completa [D+I] | `notif.slint`, `NotifOps.kt` | Sin pruebas automáticas Android | P2 |
| Llamadas: contestar, colgar, timbre, silenciar audio del PC | Completa [D] | `ring.rs`, `audio.rs` (4 pruebas), `CallOps.kt` | Silenciado solo Windows | — |
| Marcar llamadas y contactos | Completa [D] | `PhoneOps.kt` | — | — |
| SMS leer/escribir | Parcial | `SmsOps.kt`; FUNCIONES P11 | Envío no probado con destinatario real; sin MMS | P3 |
| Archivos (dos paneles, enviar/traer, vista previa) | Completa | `transfer.rs` (2 pruebas), `glue.rs load_thumbs` | Miniaturas PC solo jpg/png | P3 |
| Galería por álbumes | Completa [D] | `MediaOps.kt` | — | — |
| Portapapeles PC→móvil y móvil→PC | Completa / Parcial | FUNCIONES P6/P7 | Viaje móvil→PC no verificado automáticamente | P3 |
| Apps del móvil en ventanas (adb + scrcpy) | Completa | `appwin.rs`, `adb.rs` (2), `tools.rs` (2) | Requiere depuración USB/inalámbrica | — |
| Mensajería web en ventanas Edge | Completa | `glue.rs:1016-1040` | Depende de Edge; perfiles crecen sin límite (1 GB medido) | P2 |
| Menú «Enviar al móvil» del Explorador | Completa | `glue.rs context_menu` | Solo Windows | — |
| Copia automática de fotos | Parcial | FUNCIONES M6 | No probado en ronda real | P3 |
| Reglas de notificaciones / modo concentración | Parcial | FUNCIONES M3 | Faltan horarios | P3 |
| Búsqueda global | No implementada | FUNCIONES M8 | — | P3 |
| Varios PC / varios móviles | Parcial | FUNCIONES M9 | — | P3 |
| Inicio con Windows (clave Run, reparación) | Completa [H 2026-11-21] | `autostart.rs` (6 pruebas), `shell.rs` | Solo Windows; sin diagnóstico en la interfaz | P2 |
| Instancia única | Completa | `shell.rs:48-75` (puerto fijo 47601) | Puerto fijo puede chocar | P3 |
| Registro de arranque `startup.log` y `security.log` | Completa | `shell.rs startlog`, `core.seclog` | No hay registro general de errores | P2 |
| Actualización automática firmada | Completa con defectos | `glue.rs:1898-2010` | Sin aviso tras actualizar, sin rollback tras arranque fallido (H-01, H-05) | P1 |
| Instalador por usuario y desinstalación | Completa [H 2026-11-21] | `instalador/syncroumbral.nsi` | Sin firma de código | P1 |
| Tarjeta «Acerca de» | Completa | `app.slint` | — | — |
| Firma de código Windows | No implementada | `docs/FIRMA.md`; SignPath rechazó | SmartScreen avisa | P1 |
| Linux / macOS | No implementada | `vault.rs:62` declara «solo Windows» | Ver 04 | P2 |
| Pruebas Android | No implementada | 0 archivos de prueba | — | P2 |

## Madurez en una frase

Beta funcional y usada a diario en un PC con Windows; núcleo de seguridad bien pensado; falla en lo que rodea al producto: pruebas que no corren, CI mínima, código del agente alojado en la carpeta de una versión retirada, observabilidad escasa y dependencia total de Windows.
