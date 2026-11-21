# Firma de código de Windows (SignPath Foundation)

**Estado: aún no firmado. SignPath Foundation respondió el 2026-11-20 que por ahora no puede aprobar el proyecto** (su programa exige señales de adopción pública: estrellas, forks, colaboradores, artículos o menciones independientes; no es un juicio sobre la calidad). Se puede volver a solicitar cuando el proyecto tenga más visibilidad, o usar SignPath con suscripción de pago. Los `.exe` publicados no llevan firma de Windows (SmartScreen puede avisar). Esta guía deja todo listo por si se aprueba más adelante. Nada de aquí afirma que ya esté firmado.

## Qué está preparado en el repositorio
| Pieza | Archivo |
|---|---|
| Licencia de código abierto (OSI) | `LICENSE` (GPL-3.0-or-later) |
| Compilación pública y repetible | `.github/workflows/build-windows.yml` (trabajo `build`) |
| Qué se firma | `.signpath/artifact-configuration.xml` (solo `syncroumbral.exe`) |
| Trabajo de firma, inactivo hasta configurar | `.github/workflows/build-windows.yml` (trabajo `firmar`) |
| Política de firma | https://www.syncroapp.dev/umbral/#firma y README |
| Política de privacidad | https://www.syncroapp.dev/umbral/privacidad |

## Datos para la solicitud (https://signpath.org/foundation)
| Campo | Valor |
|---|---|
| Project Name | SyncroUmbral |
| Repository URL | https://github.com/juandr404/syncroumbral |
| Homepage URL | https://www.syncroapp.dev/umbral/ |
| Download URL | https://www.syncroapp.dev/umbral/ |
| Privacy Policy URL | https://www.syncroapp.dev/umbral/privacidad |
| Wikipedia URL | (vacío) |
| Build System | GitHub Actions |
| Maintainer Type | Individual |

**Tagline:** Turns your Android phone into a presence key that locks and unlocks your Windows PC, and shares files, notifications, calls and SMS over your local network.

**Description:** SyncroUmbral locks a Windows PC when you walk away with your Android phone and unlocks it when you return. It also shares files, photos, notifications, calls and SMS between phone and PC over the local network, with no cloud or accounts. Native Rust/Slint application with TLS and certificate pinning, licensed under GPL-3.0.

**Reputation:** escribir solo lo verdadero: repositorio público con versiones publicadas, pruebas automáticas, análisis de seguridad (`docs/SEGURIDAD.md`), página del proyecto. No inventar descargas ni estrellas. Al ser un proyecto nuevo, pueden rechazarlo o pedir más tiempo.

Los campos personales (nombre, correo), el reCAPTCHA y las casillas de aceptación los completa y envía el propietario.

## Cuando aprueben el proyecto
1. En el repositorio: *Settings → Secrets and variables → Actions*.
   - Variables: `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_SIGNING_POLICY_SLUG`.
   - Secreto: `SIGNPATH_API_TOKEN` (no compartirlo con nadie).
2. En SignPath: asociar la configuración de artefacto `.signpath/artifact-configuration.xml` y la política de firma.
3. Publicar una etiqueta `v*` o lanzar el flujo a mano: el trabajo `firmar` devuelve el artefacto `syncroumbral-windows-x64-firmado`.
4. Subir ese `.exe` a la publicación, regenerar `SHA256SUMS.txt`, volver a firmarlo con `tools/release-sign` y entonces sí actualizar README, la página y `docs/FUNCIONES.md` diciendo que está firmado.

## Nota
La firma de Windows es independiente de la firma propia del actualizador (ECDSA P-256 sobre `SHA256SUMS.txt`, ver `docs/SEGURIDAD.md`) y de la firma del APK (clave propia de Android).
