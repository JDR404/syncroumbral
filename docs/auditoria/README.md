# Auditoría integral de SyncroUmbral (2026-11-22)

Diagnóstico y plan, sin cambios en el código de producción. Documentos:
[01 Estado actual](01_ESTADO_ACTUAL.md) · [02 Auditoría técnica](02_AUDITORIA_TECNICA.md) · [03 Errores y riesgos](03_ERRORES_Y_RIESGOS.md) ·
[04 Multiplataforma](04_COMPATIBILIDAD_MULTIPLATAFORMA.md) · [05 Arquitectura objetivo](05_ARQUITECTURA_OBJETIVO.md) · [06 Roadmap](06_ROADMAP.md) ·
[07 Plan de pruebas](07_PLAN_DE_PRUEBAS.md) · [08 Mejoras](08_MEJORAS_Y_FUNCIONALIDADES.md) · [09 Plan de implementación](09_PLAN_DE_IMPLEMENTACION.md)

**Límites**: sin acceso a Linux ni macOS; sin pruebas automáticas del móvil; mediciones en un solo PC (Windows 11). Hechos comprobados, inferencias y recomendaciones se distinguen en cada documento.

## A. Diagnóstico ejecutivo

1. **Estado real**: beta funcional en Windows, en uso diario, con un núcleo de seguridad sólido y consumo muy bajo. Lo débil es lo que rodea al producto: pruebas que no corren, CI mínima, observabilidad escasa y dependencia total de Windows.
2. **Porcentaje implementado**: denominador = las 33 entradas del registro `docs/FUNCIONES.md` (P1–P17, M0–M15) menos las 5 descartadas con motivo (P12, P13, P14, P16, P17) = **28**. Completas 20 (P1–P10, M0, M1, M2, M4, M5, M10, M11, M12, M14, M15), parciales 6 (P11, P15, M3, M6, M7, M9), no implementadas 2 (M8, M13). Contando parciales como media: (20 + 3) / 28 ≈ **82 %**. No incluye Linux ni macOS (0 %). Confianza media: varias «completas» lo son por prueba manual o según la documentación, no por prueba automática.
3. **Funciona y está comprobado**: vinculación, bloqueo por presencia, notificaciones, archivos, instalador, inicio automático (validado tras reinicio el 2026-11-21), actualización firmada (validada por el usuario el 2026-11-22). **Sin verificar aquí**: SMS enviados, copia automática de fotos, portapapeles móvil→PC, todo lo de Android de forma automática, Linux y macOS.
4. **Cinco problemas principales**: H-02 (26 pruebas del protocolo sin ejecutar), H-04 (CI mínima), H-01 (actualización sin aviso), H-03 (sin firma de código), A-03 (código de Windows mezclado con la lógica).
5. **Qué impide llamarla estable**: sin red de pruebas activa sobre el protocolo, sin rollback de actualizaciones, sin registro de errores general y sin pruebas del ciclo de vida en más de un PC.
6. **Linux**: preparada al ~60 % por bibliotecas (todo el núcleo y la interfaz son portables), 0 % en integración y empaquetado. Ningún bloqueo técnico.
7. **macOS**: técnicamente igual que Linux; el bloqueo es firma, notarización (cuenta de pago) y acceso a un Mac.
8. **Riesgo arquitectónico principal**: el agente vive en la carpeta de la versión Tauri retirada y se incluye por rutas; junto con el código de Windows disperso, es lo que más encarece portar y mantener.
9. **Corregir primero**: activar las pruebas del servidor y la CI en cada PR (E0).
10. **Ruta más eficiente a multiplataforma**: E0 → E1 → workspace + capa de plataforma (E2) → AppImage Linux (E3); macOS después, si se decide el gasto.

## B. Semáforo de madurez (0–100, estimación razonada)

| Área | Nota | Criterio y evidencia |
|---|---|---|
| Arquitectura | 60 | Núcleo separado de la interfaz (bien); agente incluido por `#[path]` desde carpeta retirada y SO mezclado (A-01, A-03) |
| Calidad del código | 60 | Comentado y legible; `glue.rs` de 2 451 líneas, 197 `unwrap`, 133 resultados descartados |
| Estabilidad | 65 | Uso diario sin caídas reportadas; arranque robusto desde 0.7.1; sin rollback ni log general |
| Seguridad | 78 | Modelo de amenazas, 14/19 hallazgos corregidos, actualizaciones firmadas; pruebas del protocolo inactivas, 1 aviso RUSTSEC |
| Rendimiento | 90 | 13 MB privados, 0,05 % CPU en reposo, 9 hilos (medido) |
| Experiencia de usuario | 70 | Interfaz cuidada y explicativa; actualización silenciosa, sin diagnóstico de inicio |
| Mantenibilidad | 55 | Documentación abundante pero desalineada en puntos; commits grandes; objeto dios |
| Automatización de pruebas | 35 | 31 pruebas activas, 26 inactivas, 0 en Android, CI solo en etiquetas |
| Distribución | 70 | Instalador por usuario, desinstalación limpia, actualizador firmado; sin firma de código |
| Multiplataforma | 20 | Bibliotecas portables; cero adaptadores, empaquetado o CI fuera de Windows |

## C. Top 20 de acciones

1. Arnés de pruebas del servidor sin Tauri (H-02).
2. CI en push/PR con fmt, clippy, test, audit y Gradle (H-04).
3. Aviso «Actualizado a X» con novedades (F-01).
4. Registro general de errores con rotación (F-03).
5. Corregir documentación desalineada (A-07).
6. Diagnóstico del inicio automático en Ajustes (F-04).
7. Rollback del actualizador (F-02).
8. Limpiar `web` huérfano y mostrar tamaño de sesiones (F-06).
9. Revisar `let _ =` con efectos y registrar errores (A-04).
10. Workspace y `crates/umbral-core` (A-01).
11. Capa `umbral-platform` con Windows (A-03).
12. Quitar el *fallback* en claro de `vault` (H-07).
13. Dividir `glue.rs` por dominio (A-02).
14. Instancia única sin puerto fijo (A-05).
15. Guion de rendimiento con presupuesto en CI.
16. Prueba de cierre limpio de `scrcpy`/`adb`/Edge (CV-09).
17. Firma de código Windows cuando haya presupuesto (H-03).
18. `linux.rs` + AppImage + CI Ubuntu (E3).
19. Pruebas instrumentadas básicas de Android.
20. Decisión documentada sobre macOS (gasto y hardware).

## D. Roadmap

Ver [06](06_ROADMAP.md): corto plazo 0.8.0 (E0+E1), mediano 0.9.0 (E2+E3), largo 1.0 (E4+E6).

## E. Recomendación arquitectónica

**Refactorizar parcialmente y modularizar, con migración gradual.** No sustituir tecnologías: Slint, tokio, rustls y el protocolo son portables y eficientes. Mover el agente a una biblioteca propia, aislar el sistema operativo tras un trait y dividir `glue.rs`; todo moviendo código antes de cambiarlo, con las pruebas del servidor activas como red de seguridad.

## F. Próxima acción recomendada

**Activar las pruebas del servidor en la versión Slint (H-02).**
- **Cómo**: en `desktop/src-tauri/src/server.rs:297-300` quitar la condición `not(feature = "slint-ui")` y sustituir en `server_tests.rs` el uso del entorno de pruebas de Tauri por una implementación falsa del trait `Ui` (`desktop/src-tauri/src/ui.rs`), como hace `desktop-slint/src/glue_tests.rs`.
- **Archivos**: `server.rs`, `server_tests.rs`, quizá `ui.rs` (si falta algún método para observar eventos).
- **Verificación**: `cargo test --release -- --list` en `desktop-slint` muestra las pruebas `server::tests::*`; `cargo test --release` pasa; romper a propósito la comprobación del código de un solo uso hace fallar al menos una prueba.
