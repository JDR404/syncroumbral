# 06 · Roadmap

**Supuestos**: una persona a tiempo parcial con ayuda de un asistente; esfuerzos en días de trabajo efectivo; no incluyen esperas (revisión de Apple, compra de certificados). Son estimaciones con incertidumbre de ±50 %.

**Camino crítico**: E0 (pruebas activas) → E1 (estabilidad Windows) → E2 (núcleo + plataforma) → E3 (Linux). macOS depende además de una decisión de gasto.

## E0 · Línea base y red de seguridad — P1, ~3 días
- **Objetivo**: que cualquier regresión se detecte antes de publicar.
- **Tareas**: arnés de pruebas del servidor sin Tauri (H-02); `ci.yml` en push/PR con fmt, clippy, test, audit y Gradle (H-04); corregir documentación desalineada (A-07); guion de medición de rendimiento (07).
- **Dependencias**: ninguna.
- **Riesgos**: las pruebas del servidor dependen de detalles de Tauri más de lo esperado.
- **Aceptación**: `cargo test` en desktop-slint ejecuta ≥ 55 pruebas (31 + 26 del servidor); la CI corre en cada PR y falla si alguna falla.
- **Terminado**: CI verde en `main` y en un PR de prueba con un fallo provocado que la CI detecta.

## E1 · Estabilización de Windows — P1, ~5 días
- **Tareas**: aviso «Actualizado a X» con novedades (H-01); rollback si la nueva versión no llega a «listo» (H-05); diagnóstico del inicio automático en Ajustes (H-06); limpiar `web` huérfano y mostrar tamaño de sesiones (H-08); registro general de errores con rotación y «Exportar diagnóstico» (F-03/F-05); revisar `let _ =` con efecto (A-04).
- **Dependencias**: E0.
- **Riesgos**: rollback mal hecho puede dejar dos versiones peleando por el puerto.
- **Aceptación**: casos CV-01…CV-12 de 07 pasan en Windows 10 y 11 reales.
- **Terminado**: publicación 0.8.0 instalada por el autor durante una semana sin incidencias.

## E2 · Núcleo y capa de plataforma — P2, ~8 días
- **Tareas**: workspace y `crates/umbral-core` (A-01); `umbral-platform` con Windows (A-03); dividir `glue.rs` (A-02); instancia única sin puerto fijo (A-05); `vault` sin *fallback* en claro (H-07).
- **Dependencias**: E0 (sin pruebas activas no se mueve código).
- **Riesgos**: regresiones sutiles en Windows; mitigado por mover código sin reescribirlo.
- **Aceptación**: cero `cfg(windows)` fuera de `umbral-platform`; misma batería de 07 en Windows.

## E3 · Primera versión Linux — P2, ~8 días
- **Tareas**: `linux.rs` (Secret Service, XDG autostart, `xdg-open`, `loginctl`, PipeWire opcional); bandeja con hilo GTK; AppImage; CI `ubuntu-latest` (compilar y pruebas sin interfaz); descarga de adb/scrcpy por sistema; guía de cortafuegos y Avahi.
- **Dependencias**: E2.
- **Riesgos**: bandeja en GNOME; Wayland y posición del aviso.
- **Aceptación**: en Ubuntu 24.04 (GNOME, Wayland) y Fedora (KDE): vincular, notificaciones, archivos, inicio automático, actualización; funciones no soportadas ocultas.
- **Terminado**: AppImage publicada como beta y probada en máquinas reales (no solo CI).

## E4 · macOS — P3 hasta decidir el gasto, ~8 días + esperas
- **Tareas**: `macos.rs` (Keychain, LaunchAgent/SMAppService, `open`, cuarentena); `.app` + DMG; firma Developer ID y notarización en CI (`macos-latest`); binario universal.
- **Dependencias**: E2; cuenta Apple Developer; acceso a un Mac para probar.
- **Aceptación**: instalar desde DMG en un Mac sin avisos de Gatekeeper; mismas funciones que Linux salvo las marcadas como no compatibles.

## E5 · Calidad y experiencia — P2, continuo
- Pruebas instrumentadas Android básicas, presupuesto de rendimiento en CI, reglas de notificaciones con horario (M3), búsqueda (M8).

## E6 · Consolidación y lanzamiento — P2
- Canal estable además de beta; firma de código Windows (H-03); documentación de usuario por sistema; política de soporte (qué versiones de cada sistema).

## Corto, mediano y largo plazo

| Plazo | Hito verificable | Entregable |
|---|---|---|
| Corto (2–3 semanas) | E0 + E1 | 0.8.0: CI completa, aviso de actualización con rollback, diagnóstico de inicio |
| Mediano (1–2 meses) | E2 + E3 | 0.9.0: núcleo separado, AppImage Linux beta |
| Largo (3+ meses) | E4 + E6 | 1.0: estable en Windows y Linux, macOS si se aprueba el gasto, firma de código |
