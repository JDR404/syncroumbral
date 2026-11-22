# 07 · Plan de pruebas

## Estado actual [H]

| Nivel | Hoy | Dónde corre |
|---|---|---|
| Unitarias Rust | 31 (28 pasan, 3 ignoradas) | Local y CI en etiquetas |
| Servidor/protocolo | 26 escritas, **0 ejecutadas** (H-02) | Ninguno |
| Integración interfaz | 1 (`glue_tests.rs`: agente real a través de Slint) | Local y CI en etiquetas |
| Android | 0 | — |
| Ciclo de vida (instalar, reiniciar, actualizar, desinstalar) | Manual, ad hoc | PC del autor |
| Rendimiento | Medición puntual | PC del autor |

## Estrategia

1. **Unitarias**: lógica pura (versiones, `repair_plan`, límites, validaciones, sellos). Objetivo: cada módulo de `umbral-core` con pruebas de sus funciones puras.
2. **Integración**: servidor TLS real con un cliente de prueba (el arnés de H-02): vinculación, código de un solo uso, límites antes de autenticar, bloqueo tras fallos, transferencia con cancelación.
3. **Sistema**: el ejecutable con `--data-dir` temporal y las opciones `selftest` existentes (`--test-unlock`, `--test-page`, `--test-fake`) para capturas y flujos sin móvil.
4. **Regresión**: toda corrección de bug añade una prueba que fallaba antes.
5. **Ciclo de vida**: tabla CV, manual en máquinas reales, obligatoria antes de cada publicación que toque arranque, instalador o actualizador.
6. **Multiplataforma**: CI compila y ejecuta unitarias/integración en `windows-latest`, `ubuntu-latest` y (E4) `macos-latest`; la interfaz y el ciclo de vida se prueban en máquinas reales.
7. **Seguridad**: `cargo audit` en CI; revisión de `Command::new` con datos externos; prueba de que un `devices.json` editado a mano se ignora (existe).
8. **Rendimiento**: guion que lanza la app con perfil de prueba, espera 60 s y registra memoria privada, hilos y CPU; presupuesto: ≤ 25 MB privados y ≤ 0,5 % CPU en reposo; falla la CI si se supera en un 50 %.

## Casos de ciclo de vida (CV)

| ID | Caso | Resultado esperado | Automatizable |
|---|---|---|---|
| CV-01 | Instalación limpia con doble clic | En Aplicaciones instaladas, menú Inicio, entrada Run con comillas | Parcial (instalación silenciosa en CI; el doble clic es manual) |
| CV-02 | Reiniciar Windows | La app arranca sola; `startup.log` con «inicio en segundo plano … listo» | No (máquina real) |
| CV-03 | Apagar y encender (con inicio rápido activo) | Igual que CV-02 | No |
| CV-04 | Cerrar y abrir sesión | Igual que CV-02 | No |
| CV-05 | Cuenta estándar (sin administrador) | Instala y arranca sin UAC | No |
| CV-06 | Segunda instancia (abrir dos veces) | Se muestra la ventana existente; no hay dos procesos | Sí |
| CV-07 | Actualizar desde la app | Versión nueva, aviso «Actualizado», versión correcta en Aplicaciones instaladas | Parcial (release de prueba) |
| CV-08 | Actualización con versión rota | Vuelve a la anterior (cuando exista H-05) | Sí con release de prueba |
| CV-09 | Salir desde la bandeja con apps y mensajería abiertas | No quedan procesos `scrcpy`, `adb` ni Edge del perfil | Sí |
| CV-10 | Desactivar inicio en la app y reinstalar | El instalador respeta la decisión | Sí (probado 2026-11-21) |
| CV-11 | Desinstalar | Sin carpeta, Run, acceso directo ni menú contextual; datos según la respuesta | Sí (probado 2026-11-21) |
| CV-12 | Arranque sin red | Arranca, muestra «Móvil ausente», reconecta al volver la red | No |
| CV-13 | Puerto 47600 ocupado por otro programa | Mensaje claro (hoy) o puerto alternativo (tras A-05) | Sí |

## Matriz de sistemas

| Sistema | CI | Máquina real |
|---|---|---|
| Windows 11 x64 | Sí | PC del autor |
| Windows 10 x64 | Sí (mismo runner) | Pendiente: máquina virtual |
| Ubuntu 24.04 GNOME Wayland | E3 | Pendiente |
| Fedora KDE | — | Pendiente |
| macOS Apple Silicon | E4 | Pendiente (requiere Mac) |

## Criterios de salida de una publicación

- CI verde (fmt, clippy, pruebas, audit sin vulnerabilidades nuevas, Gradle).
- CV-01, CV-02, CV-06, CV-07 y CV-11 en Windows real si la entrega toca arranque, instalador o actualizador.
- CHANGELOG, FUNCIONES y README actualizados en el mismo PR.
