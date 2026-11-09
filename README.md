<p align="center"><img src="docs/brand/logo-512.png" alt="SyncroUmbral" width="128"></p>

# SyncroUmbral

> Parte del portafolio de **SyncroApp** (en pruebas). Desarrollador: SyncroApp, `syncroapp.dev@gmail.com`.
> Nombre anterior del proyecto: Umbral. Estado de seguridad: [docs/AUDITORIA_SEGURIDAD.md](docs/AUDITORIA_SEGURIDAD.md).
> Cómo distribuirla sin bloqueos de Android: [docs/DISTRIBUCION.md](docs/DISTRIBUCION.md).

Tu móvil Android como llave de tu PC: mientras el móvil esté en la red (o conectado por cable), la app del PC
está abierta. Si se va, la app se bloquea y oculta todo lo que había en pantalla. Sin móvil, solo se abre con clave.
Además trae al PC tus notificaciones, tus archivos y tus apps.

## Qué hace

- **Presencia y bloqueo:** latido firmado cada 2 s; sin latido durante 10 s la app se bloquea (pantalla borrosa, contenido fuera de la
  interfaz) y se cierran las ventanas, transferencias y apps abiertas.
- **Residente y ligero:** al cerrar la ventana queda en la bandeja del sistema con unos 5 MB y sigue recibiendo notificaciones.
  Se puede iniciar con Windows. Una sola copia a la vez.
- **Archivos:** explorador de dos paneles (PC y móvil), arrastrar y soltar desde el Explorador, copia de archivos y carpetas en ambos
  sentidos con varios archivos a la vez, progreso y cancelación. Cada archivo se verifica por tamaño y nunca queda a medias.
  Renombrar, crear y eliminar en el móvil.
- **Compartir con el PC:** desde el menú Compartir de cualquier app del móvil, el texto o los archivos llegan al PC.
- **Notificaciones:** llegan al PC en una ventana emergente y en su pestaña. Se responde desde el PC (WhatsApp, Telegram, SMS y cualquier
  app con respuesta rápida), se pulsan sus botones, se descartan o se silencia la app. Con la aplicación bloqueada no se muestra ni guarda nada.
- **Apps del móvil en el PC:** "Abrir en el PC" abre cualquier app en su propia ventana (scrcpy, pantalla virtual: el móvil puede seguir
  bloqueado y dormido; SyncroUmbral lo mantiene despierto con el panel apagado y lo deja dormir al terminar). La sesión vive en el móvil, así que no
  hay que volver a iniciar sesión. Detener o desinstalar la app cierra su ventana.
- **Mensajería web:** WhatsApp, Instagram, Facebook, Messenger y Telegram en sus clientes web oficiales, **dentro de la propia ventana de SyncroUmbral**,
  con la sesión guardada en el PC (sin escanear el QR cada vez, y funcionan con el móvil apagado). Las páginas web no pueden usar ninguna
  función interna de SyncroUmbral.
- **Llamadas:** aviso con sonido en el PC y botones Contestar y Rechazar para llamadas normales y de WhatsApp (requiere dos permisos en el móvil);
  silencia el resto de sonidos del PC mientras suena (opcional). El audio de la conversación sigue en el móvil, ver limitaciones.
- **Aplicaciones:** lista con icono, versión, tamaño y fecha; abrir, detener, ajustes, extraer APK, borrar datos y desinstalar (por adb).
- **Utilidades:** batería del móvil en la cabecera, texto del PC al portapapeles del móvil, el móvil reencuentra al PC si cambia de IP.
- **Portabilidad:** SyncroUmbral descarga por sí mismo `adb` (Google) y `scrcpy` (GitHub) en versiones fijas, con su SHA-256 incrustado y verificado,
  y ayuda a emparejar la depuración inalámbrica desde la interfaz.
- **Dos modos:** básico (sin depuración: bloqueo, archivos, notificaciones, compartir, mensajería web) y completo (con depuración: además abrir
  apps del móvil en el PC y gestionarlas). Ver [docs/INSTALACION.md](docs/INSTALACION.md).
- **Seguridad:** TLS con huella fijada, vinculación por QR de un solo uso, clave privada no exportable (Android Keystore),
  reto-respuesta en cada conexión, nombres de archivo del móvil saneados para Windows, nombres de paquete y entradas validados.

## Frente a las alternativas

| | SyncroUmbral | Phone Link (Microsoft) | KDE Connect |
|---|---|---|---|
| Apps del móvil en ventanas del PC | Cualquier Android con depuración; el móvil puede estar bloqueado | Según Microsoft, solo dispositivos seleccionados (sobre todo Samsung) | No |
| Bloqueo del PC según presencia del móvil | Sí, firmado y con clave de respaldo | No | No |
| Notificaciones con respuesta | Sí, más silenciar apps | Sí, con filtro por app | Sí |
| Transferencias verificadas | Tamaño comprobado, sin archivos a medias | - | Una reseña critica que una transferencia grande puede darse por buena sin completarse |
| Portapapeles compartido | PC a móvil; móvil a PC con Compartir (límite de Android) | Sí | Sí |
| Llamadas, SMS | No | Sí (llamadas por Bluetooth) | Notificaciones de llamadas y mensajes |
| Control remoto, multimedia | No | - | Sí (panel táctil, control multimedia) |
| Sistema | Android | Android 10 o superior; iPhone con menos funciones | Android, varios sistemas de escritorio |

Fuentes: [Microsoft Phone Link](https://support.microsoft.com/en-US/phone-link?fromAR=3), [comparativa de requisitos de Phone Link](https://www.sikich.com/insight/microsoft-phone-link-why-you-should-care-and-how-to-get-started/),
[KDE Connect en F-Droid](https://f-droid.org/en/packages/org.kde.kdeconnect_tp/), [reseña de KDE Connect](https://www.ctrl.blog/entry/review-kde-connect/),
[scrcpy v3.0](https://github.com/Genymobile/scrcpy/releases/tag/v3.0). Las comparaciones reflejan lo que encontré en esas páginas; revisa la versión actual de cada producto.

## Estructura

```
desktop/   App de escritorio (Tauri 2: agente Rust + interfaz HTML/JS)
android/   App del móvil (Kotlin, servicio en primer plano)
docs/      PROTOCOLO.md, INSTALACION.md
```

Para instalar en otro PC o móvil, ver [docs/INSTALACION.md](docs/INSTALACION.md). Estado de seguridad y hoja de ruta en
[docs/AUDITORIA_SEGURIDAD.md](docs/AUDITORIA_SEGURIDAD.md).

## Compilar

PC (requiere Rust, Node y las Build Tools de MSVC):
```
cd desktop
npm install
npx tauri build                  # instalador: src-tauri/target/release/bundle/nsis/SyncroUmbral_0.1.0_x64-setup.exe
npx tauri build --no-bundle      # solo el ejecutable: src-tauri/target/release/syncroumbral.exe
cargo test --manifest-path src-tauri/Cargo.toml
```

Android (requiere JDK 17 y el Android SDK; no hace falta Android Studio):
```
cd android
.\gradlew.bat assembleRelease    # app/build/outputs/apk/release/app-release.apk (no depurable, firmado con tu clave: ver docs/INSTALACION.md)
adb install -r app\build\outputs\apk\release\app-release.apk
# Solo para desarrollo: .\gradlew.bat assembleDebug (depurable, con intents de prueba)
```

## Usar
1. Abre SyncroUmbral en el PC y crea la clave de respaldo.
2. En el móvil abre SyncroUmbral y concede archivos, notificaciones, acceso a notificaciones y uso en segundo plano.
3. En el PC: Móviles y ajustes, "Mostrar QR de vinculación". En el móvil: "Escanear QR del PC".
4. Para la pestaña Aplicaciones: en Móviles y ajustes, tarjeta Herramientas, instala adb y scrcpy, y activa la depuración inalámbrica (o USB) en el móvil.

El firewall de Windows pedirá permitir `syncroumbral.exe` en redes privadas (puerto TCP 47600 y mDNS).

## Limitaciones conocidas
- Con la ventana abierta SyncroUmbral usa unos 160 a 190 MB (WebView2); cerrada, unos 5 MB. La ventana emergente de notificaciones sube a unos 160 MB solo mientras se ve.
- Solo Android. En iPhone no es posible listar ni gestionar apps.
- Las llamadas se pueden contestar o rechazar desde el PC, pero **el audio no se puentea**: Windows no ofrece a las apps de terceros el papel de
  auricular Bluetooth (Phone Link de Microsoft sí lo hace) y Android no permite capturar el audio de las llamadas normales. Sin SMS ni control remoto.
- Android no deja a una app en segundo plano leer el portapapeles: del móvil al PC se usa Compartir.
- La depuración inalámbrica de Android se apaga al cambiar de red o reiniciar el móvil; hay que volver a activarla (sin emparejar otra vez).
- Las apps con captura de pantalla bloqueada (bancos) se ven en negro en "Abrir en el PC".
- El móvil recuerda un solo PC; un PC puede tener varios móviles.
- La compilación con `--features webdebug` es solo para pruebas (abre un puerto de depuración); no usarla para instalar.
