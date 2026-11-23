# 03 · Errores y riesgos

Severidad: **P0** crítico · **P1** alto · **P2** medio · **P3** bajo. Tipo: Bug, Deuda, Riesgo, Mejora, Investigación. Estado de la evidencia: [H] comprobado, [I] inferido del código.

| ID | Prioridad | Tipo | Hallazgo |
|---|---|---|---|
| H-01 | P1 | Bug (UX) | Tras actualizar, la app se reabre oculta y no avisa — **resuelto en `main` (2026-11-22), sin publicar** |
| H-02 | P1 | Riesgo | 26 pruebas del servidor y del protocolo no se ejecutan en ningún sitio — **resuelto (2026-11-22): 54 pruebas activas** |
| H-03 | P1 | Riesgo | Ejecutables e instalador sin firma de código |
| H-04 | P1 | Deuda | CI mínima: solo en etiquetas, solo Windows, sin Android, sin `cargo audit` |
| H-05 | P2 | Bug | El actualizador no vuelve atrás si la versión nueva no arranca |
| H-06 | P2 | Riesgo | Sin diagnóstico del inicio automático en la interfaz |
| H-07 | P2 | Riesgo | `dpapi()` fuera de Windows guarda los secretos sin cifrar |
| H-08 | P2 | Deuda | 2 GB de datos (1 GB huérfano de la versión Tauri) |
| H-09 | P2 | Riesgo | Vulnerabilidad RUSTSEC-2026-0387 (`crypto-bigint` vía `p256`) |
| H-10 | P3 | Riesgo | 8 dependencias sin mantenimiento (avisos) |
| H-11 | P3 | Bug | Puertos fijos 47600/47601 |
| H-12 | P3 | Riesgo (proceso) | Instalar desde una terminal de una app MSIX deja la instalación virtualizada |
| H-13 | P3 | Deuda | Documentación desalineada (ver 02, A-07) — **resuelto (2026-11-22)** |

## Detalle

### H-01 · Sin aviso tras actualizar — P1
- **Evidencia [H]**: `glue.rs` `do_update`: tras escribir el exe nuevo ejecuta `Command::new(&exe).arg("--background").spawn()` y `slint::quit_event_loop()`. Con `--background` la ventana no se muestra (`shell.rs`). Nada guarda que hubo una actualización. Reproducido por el usuario el 2026-11-22: la app «se cierra y queda en la bandeja».
- **Impacto**: parece un cierre inesperado; el usuario no sabe si funcionó ni qué cambió.
- **Solución**: lanzar la nueva versión con `--updated-from=<versión>` (sin `--background` si la ventana estaba abierta); al arrancar, mostrar un aviso «Actualizado a X.Y.Z» con botón «Ver novedades», y la sección de esa versión del CHANGELOG/notas (ya descargadas de la API de GitHub, campo `body`).
- **Esfuerzo**: S–M.

### H-02 · Pruebas del protocolo desactivadas — P1
- **Evidencia [H]**: `desktop/src-tauri/src/server.rs:297-300`: `#[cfg(all(test, not(feature = "slint-ui")))] mod tests;` («usan el entorno de pruebas de Tauri»). `cargo test -- --list` en desktop-slint: 31 pruebas, ninguna de `server::tests`. La versión Tauri ya no se compila en CI ni se publica.
- **Impacto**: las garantías de seguridad del protocolo (código de vinculación de un solo uso, límites antes de autenticar, bloqueo tras 8 fallos: S-04, S-08) pueden romperse sin que nada lo detecte.
- **Solución**: reescribir el arnés de esas pruebas sobre el trait `Ui` (una implementación falsa) en vez de `tauri::test`, para que corran con la interfaz Slint y en CI.
- **Esfuerzo**: M.

### H-03 · Sin firma de código — P1
- **Evidencia [H]**: SignPath Foundation rechazó la solicitud (2026-11-20, falta de visibilidad pública). SmartScreen avisa en instalador y exe.
- **Impacto**: fricción de instalación y riesgo de falsos positivos de antivirus; el actualizador mitiga la integridad (firma ECDSA propia), no la reputación.
- **Solución**: certificado de pago (OV/estándar o servicio en la nube) cuando haya usuarios; mientras, enviar cada versión al portal de análisis de Microsoft y publicar las huellas. Reintentar SignPath con más tracción.
- **Esfuerzo**: S técnico; coste económico.

### H-04 · CI mínima — P1
- **Evidencia [H]**: `.github/workflows/build-windows.yml` se dispara solo con etiquetas `v*` y a mano; ejecuta pruebas, compila exe e instalador; no hay flujo para Android, Linux, `cargo audit`, `clippy`, ni para PR o `main`.
- **Impacto**: los fallos se descubren al publicar.
- **Solución**: flujo `ci.yml` en `push`/`pull_request`: `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `cargo audit`, `./gradlew assembleRelease lint`. El repo es público: los minutos de Actions son gratis.
- **Esfuerzo**: S.

### H-05 · Sin rollback tras arranque fallido — P2
- **Evidencia [I]**: `do_update` solo restaura `syncroumbral.old` si falla la **escritura**. Si la versión nueva arranca y muere, el `.old` queda en disco pero nada lo usa.
- **Impacto**: una publicación defectuosa deja la app sin funcionar hasta reinstalar a mano.
- **Solución**: la versión nueva, al llegar a «listo», escribe una marca `update-ok`; la versión vieja (antes de salir) lanza la nueva y espera esa marca N segundos; si no aparece, restaura `.old` y relanza. Alternativa más simple: el instalador NSIS como mecanismo de actualización (`setup.exe /S`), que es atómico respecto al registro.
- **Esfuerzo**: M.

### H-06 · Inicio automático sin diagnóstico visible — P2
- **Evidencia [H]**: el estado real solo se ve en `startup.log` y en Configuración > Inicio de Windows. El 2026-11-21 hicieron falta 5 reinicios para descubrir la causa (virtualización MSIX del entorno de desarrollo).
- **Solución**: en Ajustes, línea «Inicio con Windows: activo / desactivado en Windows (StartupApproved = 03) / entrada apunta a otra copia / sin entrada», con botón «Abrir Inicio de Windows» (`ms-settings:startupapps`).
- **Esfuerzo**: S.

### H-07 · Secretos sin cifrar fuera de Windows — P2
- **Evidencia [H]**: `vault.rs:62-65`.
- **Impacto**: nulo hoy (no hay build para Linux/macOS); alto en cuanto exista.
- **Solución**: parte de A-03; en Linux usar Secret Service (`keyring`), en macOS Keychain; si no hay servicio disponible, pedir la clave de respaldo para derivar la llave (Argon2) en vez de guardar en claro.
- **Esfuerzo**: M (con la capa de plataforma).

### H-08 · 2 GB de datos — P2
Ver 02, A-06. Medido el 2026-11-22.

### H-09 · RUSTSEC-2026-0387 — P2 (riesgo bajo en la práctica)
- **Evidencia [H]**: `cargo audit`: `crypto-bigint 0.5.5` ← `elliptic-curve 0.13.8` ← `p256 0.13.2`. Variaciones de tiempo en operaciones que deberían ser de tiempo constante.
- **Impacto**: `p256` se usa para verificar firmas (datos públicos) y en la vinculación; explotar canales laterales de tiempo exige medir con precisión desde la red local. La auditoría anterior (S-13) ya lo consideró no explotable.
- **Solución**: subir `p256` cuando haya versión con `crypto-bigint >= 0.7.6`; añadir `cargo audit` a la CI con esta excepción documentada.

### H-10 · Dependencias sin mantenimiento — P3
`bincode`, `chrono`, `pin-utils`, `proc-macro-error`, `ttf-parser`, `zbus_names`, `zvariant` y otras (9 avisos permitidos). Llegan por Slint/winit/zbus; revisar al subir Slint.

### H-11 · Puertos fijos — P3
Ver 02, A-05.

### H-12 · Instalaciones virtualizadas por MSIX — P3 (proceso de desarrollo)
- **Evidencia [H]**: lo instalado desde la terminal de la app de Claude quedó en `%LOCALAPPDATA%\Packages\Claude_*\LocalCache` y `Helium\User.dat`; Explorer no lo veía. Documentado en `docs/SLINT.md` e `INSTALACION.md`.
- **Solución**: instalar siempre con doble clic o desde una PowerShell normal. No afecta a usuarios finales.

## Rendimiento medido [H]

Proceso instalado (0.7.2), ventana cerrada, móvil conectado, Windows 11:

| Métrica | Valor |
|---|---|
| Memoria privada | 13,4 MB |
| Conjunto de trabajo | 36,6 MB |
| Hilos | 9 |
| Identificadores | 294 |
| CPU en 30 s de reposo | 0,016 s (0,05 % de un núcleo) |
| Datos en disco | 2 062 MB (ver H-08) |

Sin medir (requiere guion de benchmark, ver 07): tiempo de arranque en frío, CPU durante transferencias grandes, memoria con la galería abierta (miniaturas en memoria), crecimiento tras 24 h.

## Seguridad: resumen

La auditoría de seguridad propia (`docs/AUDITORIA_SEGURIDAD.md`) sigue siendo válida en lo esencial; 14 de 19 hallazgos corregidos. Pendientes: S-03 (presencia ≠ proximidad, por diseño), S-10 (permisos amplios del móvil), S-11 (firma; el actualizador ya existe), S-17 (permisos de las webs integradas). Esta auditoría añade H-02, H-07 y H-09.

No se encontraron: secretos en el repositorio (la clave de firma está fuera, `.gitignore` cubre `keystore.properties` y `*.jks`), ejecución de comandos con texto controlado por el móvil sin validar (`adb.rs` valida la entrada de vinculación, prueba `pairing_input_is_validated_before_running_adb`), ni registros con contenido de notificaciones [I: revisión por muestreo de `seclog`].
