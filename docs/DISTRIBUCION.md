# Distribuir SyncroUmbral: que Android la instale sin bloqueos

SyncroUmbral es parte del portafolio de **SyncroApp** (en pruebas). Cuenta de desarrollador prevista: `syncroapp.dev@gmail.com`.

## Por qué hoy se bloquea la instalación manual

Al abrir el APK desde el gestor de archivos, Google Play Protect puede mostrar *"Se bloqueó la app para proteger tu dispositivo. Esta app puede
solicitar acceso a datos sensibles"*. No es un fallo del APK: es la protección de Android contra apps instaladas fuera de Play Store que piden
permisos que se usan en fraudes (acceso a notificaciones, superposición, SMS, accesibilidad). SyncroUmbral **necesita** el acceso a notificaciones
(es una de sus funciones). Permisos que declara el APK (hay un solo APK):

| Permiso | Para qué | Estado inicial |
|---|---|---|
| Acceso a notificaciones (servicio de escucha) | Mostrar y responder notificaciones en el PC | apagado, lo concede el usuario |
| Contestar llamadas (`ANSWER_PHONE_CALLS`) | Contestar y colgar llamadas normales | apagado, lo concede el usuario |
| Mostrar sobre otras apps (`SYSTEM_ALERT_WINDOW`) | Contestar llamadas de WhatsApp y similares desde segundo plano (comprobado: sin él Android lo bloquea) | apagado, lo concede el usuario |
| Todos los archivos, lista de apps | Explorador de archivos y pestaña Aplicaciones | apagado, lo concede el usuario |

Si más adelante se publica en Play, conviene reconsiderar los permisos más sensibles: la combinación de acceso a notificaciones y superposición es la
que más desconfianza despierta en las revisiones.

La instalación por `adb install -r` no pasa por ese filtro, y es la vía que usan las pruebas.

## Qué falta para instalar sin trabas (lo que depende de Google, no del código)

Lo siguiente es lo que se necesita; los detalles exactos cambian y deben confirmarse en la documentación oficial de Google antes de actuar.

1. **Identidad de desarrollador verificada.** Google anunció que desde septiembre de 2026 las apps instaladas en dispositivos Android certificados deberán
   proceder de desarrolladores verificados, también fuera de Play Store (hay una vía de "instalar sin verificar" para usuarios avanzados). El registro se hace
   con una cuenta de Google: aquí `syncroapp.dev@gmail.com`. Fuentes secundarias:
   [Bitdefender](https://www.bitdefender.com/en-au/blog/hotforsecurity/google-developer-verification-sideloaded-apps),
   [Notebookcheck](https://www.notebookcheck.net/Android-will-block-sideloading-of-unverified-apps-starting-next-year.1097806.0.html),
   [heise](https://heise.de/-11076901). Confirmar en el blog oficial de Android Developers.
2. **Identificador de aplicación definitivo.** `app.syncroapp.umbral`. En Play y en la verificación el id queda ligado a la app: no cambiarlo.
3. **Clave de firma propia y estable** (CN=SyncroUmbral, O=SyncroApp). Con la misma clave se actualizan todas las versiones. Guardarla fuera del repositorio
   y con copia de seguridad: si se pierde, hay que publicar una app nueva.
4. **Publicación en Play (opcional pero la vía más limpia):** pruebas internas o cerradas en Google Play Console permiten instalar sin el aviso de
   "app desconocida". Play revisará el uso de permisos sensibles (acceso a todos los archivos, lista de apps, notificaciones) y pedirá justificarlos.
5. **Declaración de permisos y política de privacidad.** Listado de qué datos toca la app (notificaciones, archivos, lista de apps), que todo se procesa
   en el dispositivo y el PC del usuario y que no hay servidores ni telemetría (ver `AUDITORIA_SEGURIDAD.md`, sección de privacidad).

## Pendiente de decidir por SyncroApp

- Licencia del código: **GPL-3.0-or-later** (archivo `LICENSE`), compatible con Slint bajo su licencia GPLv3.
- Si se publica en Play Store o solo por distribución directa.
- Icono y capturas definitivos de la ficha.
