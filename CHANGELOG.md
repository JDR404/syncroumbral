# Cambios

## Sin publicar

## 0.7.1 — 2026-11-20

### Arranque con Windows
- El arranque ya no se rinde en silencio: si al iniciar sesión el puerto de «una sola copia» está ocupado, o el agente o la ventana tardan en poder crearse, **reintenta unos segundos** y, si aun así no puede comprobar copias duplicadas, arranca igualmente.
- Nuevo registro de arranque `startup.log` (en `%APPDATA%pp.syncroapp.umbral`, máx. ~64 KB) con el motivo si la app no llega a abrirse. Corrige y diagnostica el caso de que, tras encender el PC, la app no apareciera en la bandeja.

## 0.7.0 — 2026-11-20

### Interfaz del PC
- **Archivos con vista previa**: las imágenes (y, del móvil, también los vídeos) muestran su miniatura en ambos paneles, que van apareciendo poco a poco. Botón para alternar entre **lista** y **cuadrícula** en cada panel; en cuadrícula el nombre completo (y tamaño y fecha) aparece al pasar el ratón, y la casilla de la esquina marca el elemento.
- **Zona de arrastre** en el panel derecho (reemplaza el «Consejo»): se ilumina al arrastrar un archivo sobre la ventana y dice claramente dónde queda cada cosa (carpeta abierta del móvil, `Descargas\SyncroUmbral` en el PC y destino de «Traer al PC»).
- **Notificaciones en columnas**: las tarjetas se reparten por el ancho de la ventana en vez de ocupar una fila entera cada una.
- Galería: los botones de álbum quedan siempre por encima de las fotos al hacer scroll.

### Distribución y licencia
- Licencia **GPL-3.0-or-later** (`LICENSE`), compatible con Slint bajo GPLv3.
- Página del proyecto: https://www.syncroapp.dev/umbral/, con política de firma (`#firma`) y **política de privacidad propia** (https://www.syncroapp.dev/umbral/privacidad).
- Ficha del repositorio en GitHub actualizada: página web, descripción y temas.
- La compilación del `.exe` en GitHub Actions se probó con éxito (pruebas + compilación + artefacto sin firmar).
- Compilación del `.exe` en **GitHub Actions** (`.github/workflows/build-windows.yml`): pruebas, compilación y huella SHA-256 del ejecutable.
- Firma de Windows con **SignPath Foundation**: solicitud enviada el 2026-11-19; preparada pero inactiva hasta que aprueben el proyecto (`.signpath/artifact-configuration.xml`, trabajo `firmar` condicionado a variables del repositorio). Guía en `docs/FIRMA.md`. Mientras tanto los ejecutables siguen **sin firma de Windows**.

## 0.6.0 — 2026-11-19

### Correcciones importantes
- **Quitar un móvil lo desvincula de verdad.** Antes, al quitarlo de la lista del PC, la sesión ya abierta seguía conectada y el móvil seguía creyéndose vinculado.
  Ahora el PC corta la sesión en el siguiente latido (≤ 2 s) y el móvil, al recibir «dispositivo no vinculado», borra su vinculación y deja de reconectar.
- **Ajustes de la 0.4.0 y 0.5.0 pisaban los del agente** (ambos usaban `settings.json`): el agente registraba «MANIPULADO» y descartaba sus ajustes (modo completo, silenciar en llamadas, apps silenciadas). Los de la interfaz pasan a `ui-settings.json`; al arrancar se migra el archivo viejo.
  Si usabas «Modo completo» o tenías apps silenciadas, vuelve a activarlos.
- **Portapapeles del móvil al PC fiable**: el botón «Copiar al PC» reintenta leer el portapapeles (Android da el foco con retraso) y el PC deja el texto directamente en su portapapeles.

### Archivos
- Una carpeta se abre con **un clic**; una casilla marca cualquier elemento. Antes un clic marcaba la carpeta y no se podía entrar.
- **Filtro por nombre** y «Marcar todo» en cada panel; se ve a qué carpeta irá cada copia («Enviar al móvil a: ...», «Traer al PC a: ...»).
- La barra de desplazamiento ya no tapa botones ni mensajes en Archivos, Llamadas y Mensajes.

### Mensajería
- Diseño en tarjetas con varias columnas; más servicios (Mensajes de Google con SMS y RCS, Discord, Slack, X, LinkedIn, Gmail, Outlook, Teams).
- **Añade tus propios servicios**: nombre y dirección https; se abren aislados en Edge con la sesión guardada. Se pueden quitar.

### Seguridad (ver [docs/SEGURIDAD.md](docs/SEGURIDAD.md))
- El PC solo atiende conexiones de la red local (privadas, enlace local, Tailscale o el propio equipo): se cortan las de direcciones públicas.
- **Actualizaciones firmadas**: la lista de huellas debe llevar la firma ECDSA P-256 del autor; una cuenta de GitHub comprometida ya no basta para colar un ejecutable.
- El móvil limita las llamadas (6 cada 10 min) y los SMS (15 cada 10 min) que puede pedir el PC.
- Nueva prueba: quitar un móvil corta su sesión.
- Botón «Copiar el código» de vinculación (para vincular sin cámara).

## 0.5.0 — 2026-11-19

### Rediseño de la interfaz
- **Notificaciones**: cada una en una sola línea (los correos y resúmenes ya no se montan unos sobre otros).
- **Pantalla completa**: las páginas usan todo el ancho; en pantallas grandes aparece un **panel derecho** con la batería del móvil y **acciones rápidas** (portapapeles al móvil, hacer sonar, ir a Archivos o a la galería).
- **Acciones rápidas siempre visibles** en la cabecera de cada página (antes el portapapeles estaba escondido en Ajustes).
- **Galería por álbumes**: todas las carpetas con fotos o vídeos (cámara, capturas, WhatsApp, descargas...), no solo la cámara. La cuadrícula se ajusta al ancho.
- **Aplicaciones**: mosaico con los **iconos reales** de las apps y búsqueda.
- **Tipografía Inter** incrustada (más nítida que la del sistema) e **iconos vectoriales** en menú y botones; menú plegado solo con iconos.
- **Ajustes**: cada tarjeta explica lo que hace; en pantalla ancha se reparten en dos columnas; las explicaciones ya no se cortan.
- Móvil (0.5.0): la galería consulta MediaStore para ver todos los álbumes.
- Licencias de terceros: fuente Inter (SIL OFL 1.1) e iconos Material Design (Apache 2.0); ver `docs/SLINT.md`.

## 0.4.0 — 2026-11-19

### Paridad con Phone Link
- **Galería** con miniaturas y vídeos, "cargar más" y traer lo seleccionado al PC.
- **Contactos, llamadas y historial**: marcar desde el PC, llamar a un contacto o devolver una llamada.
- **Mensajes SMS**: conversaciones y mensajes; enviar. Solo SMS (ver [docs/FUNCIONES.md](docs/FUNCIONES.md)).
- **Arrastrar y soltar** archivos desde el Explorador sobre la ventana, y **«Enviar al móvil con SyncroUmbral»** en el clic derecho.
- **Portapapeles** en ambos sentidos: botón «Enviar portapapeles al móvil» en el PC y «Copiar al PC» en la notificación del móvil.

### Mejoras propias
- **Códigos de verificación**: botón «Copiar código» en notificaciones y SMS.
- **Buscar el móvil**: lo hace sonar al máximo hasta un minuto.
- **Bloquear Windows al alejarte** (opcional), **ocultar el contenido de los avisos** y **silenciar avisos por palabras**.
- **Copia automática de fotos nuevas** al PC (opcional).
- **Actualización automática**: aviso de versión nueva y reemplazo con verificación de huella.

### Interfaz y agente
- Nuevas páginas Galería, Llamadas y Mensajes; ajustes en tarjetas (Seguridad y avisos, Copias y envío rápido, Acciones del móvil).
- Icono de la ventana y del ejecutable.
- Móvil (0.4.0): permisos opcionales de contactos, llamar, registro de llamadas y SMS en la pestaña Permisos.

## 0.3.0 — 2026-11-19

### Interfaz nativa con Slint (sin WebView)
- **SyncroUmbral nativo** en [`desktop-slint/`](desktop-slint): un proceso y ~8 MB privados con la ventana abierta y el móvil conectado
  (la versión Tauri usa ~230 MB en 7 procesos). Misma carpeta de datos y mismos móviles vinculados. Ver [docs/SLINT.md](docs/SLINT.md).
- Archivos, notificaciones con respuesta y controles del reproductor, llamadas, aplicaciones del móvil en el PC, mensajería en una ventana
  aislada de Edge, vinculación por QR, ajustes, aviso emergente, bandeja e instancia única.
- Instalador de usuario: `desktop-slint/instalar.ps1`.

### Agente
- El agente ya no depende de Tauri: nuevo trait `Ui` (`ui.rs`) y módulos compartidos (`api`, `boot`, `autostart`, `services`). La versión
  Tauri lo implementa con `tauri_ui.rs`. Sin cambios de comportamiento: las 43 pruebas siguen pasando.

## 0.2.0 — 2026-11-17

### Nuevo
- **Botones Bloquear y Desbloquear en la notificación del móvil** (y en "Acciones rápidas" de la app): se bloquea o desbloquea SyncroUmbral
  del PC sin abrir la app. Desbloquear exige tener el teléfono desbloqueado. Nuevo mensaje `unlock_pc` en el [protocolo](docs/PROTOCOLO.md),
  atendido solo dentro de una sesión autenticada y con el móvil presente.
- Listas de archivos y apps manejables con teclado (flechas, Espacio, Intro); Escape cierra los diálogos; roles ARIA y foco visible.

### Interfaz del PC
- El panel lateral se pliega solo en ventanas estrechas (860 px ya era inutilizable en Archivos).
- Nombres de archivo legibles: tamaño y fecha en dos líneas, nombre completo al pasar el ratón.
- Barras de portapapeles y de Aplicaciones que se ajustan en vez de partir el texto; marca "SyncroUmbral" ya no se corta.
- El aviso "El móvil no está conectado" se lee entero dentro del panel.
- La pantalla de llamada respeta el panel plegado.
- Los avisos periódicos (mensajería web, apps abiertas) se pausan con la ventana oculta; se respeta "reducir movimiento".

### App Android
- Los márgenes respetan las barras del sistema (Android 15 dibuja a pantalla completa): la barra inferior ya no choca con los gestos.
- Se conserva la sección abierta al girar el móvil.
- Si el PC no aparece, la espera entre reintentos crece de 3 s a 30 s (antes, cada 3 s sin parar: gastaba batería con el PC apagado).
- Los hilos de peticiones se cierran al detener la conexión.

### Audio (corrección importante)
- El silenciado del resto de sonidos durante una llamada ya **no toca mejoradores o mezcladores de audio** (FxSound, Voicemeeter...): por
  ellos pasa todo el audio del PC y silenciarlos lo dejaba mudo, también el timbre.
- Las pruebas automáticas ya no pueden silenciar el audio real (antes, al terminar el programa de pruebas, el hilo se cortaba antes de
  restaurar y dejaba sesiones mudas). Las dos pruebas que lo usan de verdad son `#[ignore]`.
- Nueva prueba de reparación: `cargo test --manifest-path desktop/src-tauri/Cargo.toml repair_audio -- --ignored`.

### Documentación
- README reescrito (cómo funciona, estructura, compilar y probar, hoja de ruta) y nuevo [docs/ARQUITECTURA.md](docs/ARQUITECTURA.md).

### Verificación
- 43 pruebas automáticas del PC pasan (3 ignoradas a propósito por tocar el audio real).
- App Android de producción firmada, no depurable, probada en un Pixel 9 Pro con Android 17: vinculación, conexión estable y los dos botones
  de la notificación contra el PC.

## 0.1.0 — 2026-11-07

Primera versión: presencia y bloqueo, archivos, compartir, notificaciones con respuesta, llamadas, apps del móvil en el PC,
mensajería web, modos básico y completo, y la auditoría de seguridad inicial.
