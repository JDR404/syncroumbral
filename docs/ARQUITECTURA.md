# Arquitectura de SyncroUmbral

Dos programas que hablan por un canal seguro en la red local:

```
 MÓVIL (Android, Kotlin)                                   PC (Windows, Tauri 2 + Rust)
 ───────────────────────                                   ────────────────────────────
 LinkService ── LinkClient ──── wss (TLS, huella fijada) ──▶ server.rs ── core.rs (estado)
   │              │  latido cada 2 s, peticiones del PC        │            │
   │              └── Link (ws activo para el resto)           │            ├─ commands.rs ◀── interfaz (desktop/ui)
   ├─ NotifService (lee notificaciones)                        │            ├─ shell.rs (bandeja, ventanas)
   ├─ ShareActivity (menú Compartir)                           │            ├─ toast.rs (ventana emergente)
   ├─ ActionReceiver (botones Bloquear/Desbloquear)            │            └─ transfer / share / web / appwin / adb / tools
   └─ MainActivity (permisos, ajustes, vinculación)            └─ discovery.rs (mDNS), vault.rs (DPAPI), limits.rs
```

El protocolo de mensajes está en [PROTOCOLO.md](PROTOCOLO.md). La seguridad, en [AUDITORIA_SEGURIDAD.md](AUDITORIA_SEGURIDAD.md).

## Idea central

La **presencia** del móvil decide si la aplicación del PC está abierta. `core.rs` guarda tres banderas:

| Bandera | Significado |
|---|---|
| `present` | Hay una sesión autenticada del móvil con latidos recientes |
| `manual` | Se abrió con la clave de respaldo (vale aunque el móvil no esté) |
| `forced` | El usuario pidió "Bloquear ahora": gana sobre la presencia hasta usar la clave, o hasta que el móvil pida `unlock_pc` |

La aplicación está bloqueada si `forced` o si no hay `present` ni `manual`. Al bloquear se borra la interfaz, se cierran ventanas,
transferencias y apps abiertas, y se ignoran los mensajes del móvil (excepto `unlock_pc`, cuya función es abrir).

## PC (`desktop/`)

### Interfaz: `desktop/ui/`

| Archivo | Para qué |
|---|---|
| `index.html` | Todas las pantallas: crear clave, bloqueo, principal (pestañas), diálogo y pantalla de llamada |
| `style.css` | Tema oscuro Material; el panel lateral se pliega solo por debajo de 980 px |
| `main.js` | Lógica de la ventana: paneles de archivos, apps, mensajería, ajustes, bloqueo |
| `notif.js` | Tarjetas de notificación, compartidas con la ventana emergente |
| `toast.html`, `toast.js` | Ventana emergente de notificaciones |

La interfaz solo habla con el agente por comandos (`invoke`) y eventos (`listen`). Todo texto que viene del móvil se inserta con
`textContent`, nunca como HTML. Hay una CSP estricta (`default-src 'self'`).

### Agente: `desktop/src-tauri/src/`

| Módulo | Responsabilidad |
|---|---|
| `main.rs` | Arranque, registro de comandos, una sola instancia |
| `core.rs` | Estado compartido: presencia, bloqueo, dispositivos, notificaciones, ajustes, clave de respaldo (Argon2id) |
| `server.rs` | Servidor wss en el puerto 47600: vinculación, reto-respuesta, latidos, peticiones al móvil |
| `limits.rs` | Límites antes de autenticar (tamaño, conexiones simultáneas, bloqueo por fallos) |
| `commands.rs` | Comandos que llama la interfaz (todos exigen la aplicación desbloqueada, salvo los inofensivos) |
| `vault.rs` | Secretos con DPAPI de Windows, sellos HMAC de la configuración y `security.log` con rotación |
| `shell.rs` | Bandeja del sistema, ventana principal bajo demanda, inicio con Windows |
| `toast.rs` | Ventana emergente: se crea al llegar una notificación y se destruye después |
| `transfer.rs` | Copia de archivos y carpetas por trozos, con verificación de tamaño y cancelación |
| `share.rs` | Recibe lo que el móvil comparte (texto y archivos, con Mark of the Web) |
| `web.rs` | Clientes web de mensajería dentro de la ventana, con sesión guardada y navegación limitada a sus dominios |
| `appwin.rs`, `adb.rs`, `tools.rs` | Apps del móvil en ventanas del PC (scrcpy) y acciones por adb; descarga de `adb` y `scrcpy` con hash fijo |
| `discovery.rs` | Anuncio mDNS (`_umbral._tcp`) para que el móvil reencuentre al PC si cambia de IP |
| `ring.rs`, `audio.rs` | Timbre de llamadas y silenciado temporal del resto de sonidos (no toca mejoradores como FxSound) |
| `places.rs` | Carpetas conocidas de Windows (Escritorio, Descargas...) |
| `server_tests.rs` | Pruebas de punta a punta: un cliente que imita a la app Android contra el servidor real |

### Interfaz nativa alternativa: `desktop-slint/`

Comparte el agente (sin Tauri: trait `Ui` en `ui.rs`) y dibuja con Slint, sin WebView. Detalles en [SLINT.md](SLINT.md).

## Móvil (`android/`)

| Archivo | Responsabilidad |
|---|---|
| `LinkService.kt` | Servicio en primer plano y notificación permanente con los botones **Bloquear** / **Desbloquear** |
| `ActionReceiver.kt` | Recibe esos botones y envía `lock_pc` / `unlock_pc` al PC (Desbloquear exige el teléfono desbloqueado) |
| `LinkClient.kt` | Conexión con el PC: TLS fijado, firma del reto, latidos, reintentos con espera creciente |
| `Link.kt`, `Status.kt` | Conexión activa y estado visible, compartidos |
| `KeyManager.kt`, `PinnedTls.kt`, `Store.kt` | Clave en el Keystore, confianza por huella, datos del PC vinculado |
| `Discovery.kt` | Busca al PC por mDNS cuando las direcciones guardadas no responden |
| `NotifService.kt`, `NotifOps.kt` | Lee notificaciones y las reenvía; responder, pulsar botones, descartar |
| `FileOps.kt`, `AppOps.kt`, `ClipOps.kt`, `CallOps.kt`, `PowerOps.kt`, `KeepAlive.kt` | Operaciones que pide el PC |
| `ShareActivity.kt` | Menú Compartir → PC (con confirmación y límites) |
| `MainActivity.kt` | Pantalla de gestión: Inicio, Permisos, Ajustes, Info |
| `TestNotification.kt` | Notificaciones de prueba (solo en la versión de depuración) |

## Decisiones de diseño que conviene conocer

- **Sin servidor propio ni telemetría.** Todo ocurre entre tu móvil y tu PC.
- **Huella fijada en vez de autoridades de certificación.** El móvil solo confía en el certificado cuya huella vio en el QR.
- **El móvil es la llave, no Windows.** SyncroUmbral bloquea su propia aplicación, no la sesión de Windows.
- **Residente ligero.** Cerrar la ventana destruye su WebView; el agente sigue recibiendo notificaciones con pocos MB.
- **Todo lo que viene del móvil es hostil.** Se valida, se sanea y se muestra como texto.
- **Pruebas que no tocan el audio real.** El silenciado de llamadas está desactivado en `cargo test`; las dos pruebas que lo usan de verdad
  son `#[ignore]` (ver el README).

## Dirección futura

La interfaz HTML dentro de WebView2 es lo que más memoria cuesta (unos 160–190 MB con la ventana abierta). La hoja de ruta prevé una interfaz
nativa con Slint, que reutilizaría `core.rs`, `server.rs` y el resto del agente sin cambios: solo se sustituirían `desktop/ui/` y la capa de
`commands.rs`/eventos por enlaces de Slint. Ver "Hoja de ruta" en el [README](../README.md).
