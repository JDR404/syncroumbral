# Publicar en Google Play

Preparación hecha en el código y textos para pegar en Play Console. Requisitos consultados el **2026-11-25**; Google los cambia: confirmar en
Play Console antes de enviar. **Nada de esto garantiza la aprobación**: la decide la revisión de Google.

## Estado

| Requisito | Estado |
|---|---|
| `targetSdk` 36 (Android 16) y paquete `.aab` | Hecho (`gradlew bundleRelease`) |
| Aviso destacado y consentimiento antes de cada permiso | Hecho (`Disclosures.kt`); el aviso general visto en un Pixel, el resto sin probar |
| Asistente de permisos y ayuda de «ajustes restringidos» | Hecho, **sin probar en un móvil** |
| Política de privacidad en la app y en la web | Hecho |
| Sin `QUERY_ALL_PACKAGES` ni petición directa de ignorar batería | Hecho |
| Servicio en primer plano `connectedDevice` | Hecho y probado en un Pixel 9 Pro (Android 17) |
| Cuenta de desarrollador, identidad y pago (25 USD) | Pendiente (solo el autor) |
| Prueba cerrada: 12 personas, 14 días seguidos (cuentas personales) | Pendiente |
| Video de declaración, capturas, icono 512 px y gráfico 1024×500 | Pendiente |

Al publicar: activar **Play App Signing** y subir el `.aab` firmado con la clave actual como clave de subida. La firma de la versión de Play será
distinta de la del APK de GitHub: quien tenga una debe desinstalarla para pasar a la otra.

## Ficha de la tienda

**Nombre:** SyncroUmbral: móvil y PC

**Descripción corta (máx. 80):** Sincronice su móvil con su PC: archivos, SMS, llamadas y avisos, sin nube.

**Descripción completa:**

> SyncroUmbral sincroniza su móvil Android con su PC con Windows por su red local.
>
> Desde el PC puede: leer y enviar SMS, ver el registro de llamadas y marcar, contestar y colgar llamadas, recibir y responder notificaciones,
> explorar y copiar los archivos y fotos del móvil, ver sus contactos y pasar texto del portapapeles.
>
> Además, su móvil funciona como llave de presencia: si se aleja, SyncroUmbral en el PC se bloquea.
>
> Privacidad: no hay cuentas, servidores ni publicidad. Los datos viajan cifrados, directamente entre su móvil y el PC que usted vincule.
> Software libre (GPL-3.0).
>
> Requiere el programa gratuito para Windows: https://www.syncroapp.dev/syncroumbral/

## Declaración de permisos (en inglés, que es como se revisa)

**SMS and Call Log** (`READ_SMS`, `SEND_SMS`, `READ_CALL_LOG`). Exception: *Cross-device synchronization or transfer of SMS or calls*.

> SyncroUmbral is a phone-to-PC companion. Its core, advertised feature is to let the user read and send their SMS and review their call log on
> their own Windows PC. Data is read only when the user opens Messages or Calls on the paired PC and is sent TLS-encrypted over the local network
> to that PC only. There is no developer server. There is no alternative API that provides SMS threads or call history to a paired computer.
> The user sees a prominent disclosure and must accept it before the runtime permission is requested, and can decline each one.

**All files access** (`MANAGE_EXTERNAL_STORAGE`). Use: *file management*.

> A core feature is a two-pane file manager on the paired PC: browse any folder of the phone's shared storage, copy files and folders both ways,
> create, rename and delete. The Storage Access Framework and MediaStore cannot provide this: SAF requires picking each folder or file on the
> phone and does not allow browsing the whole shared storage from the paired computer, and MediaStore only exposes media.

**Foreground service** (`connectedDevice`).

> While the phone is paired, a foreground service keeps an encrypted network connection with the user's PC, sends a signed presence heartbeat every
> 2 seconds (the PC app locks when it stops) and delivers notifications, calls and file requests. Deferring or stopping it locks the PC app and
> disconnects the sync. The user starts it by pairing with a QR code and sees a persistent notification; unpairing stops it.

**Notification access.**

> Core feature: show the phone's notifications on the paired PC and let the user reply to or dismiss them. Content goes only to the user's PC.

**Display over other apps** (`SYSTEM_ALERT_WINDOW`).

> Needed so that, when the user taps "Answer" on the PC, the incoming call screen of third-party calling apps can be opened from the background.
> The app draws nothing over other apps and reads nothing from the screen.

## Seguridad de los datos (formulario)

- Los datos que van del móvil al PC del usuario viajan cifrados (TLS con certificado fijado) y solo los leen esos dos equipos; no hay intermediario
  ni servidor. Según la definición de Google, lo cifrado de extremo a extremo no se declara como «recogido».
- **Excepción a revisar:** `play-services-code-scanner` (ML Kit) es un componente de Google que puede enviar datos de diagnóstico y uso. Declarar lo
  que indique la guía de «data disclosure» de ML Kit vigente, o sustituir el lector por uno sin telemetría antes de enviar.
- Cifrado en tránsito: sí. Borrado: desinstalar la app o desvincular. Cuentas: no hay.
- Política de privacidad: https://www.syncroapp.dev/syncroumbral/privacidad

## Instrucciones para el revisor (en inglés)

> The app pairs with the free open-source Windows program (https://github.com/JDR404/syncroumbral/releases/latest).
> 1. Install the Windows setup, open it and create a backup key. 2. On the PC open "Móviles y ajustes" and press "Generar código".
> 3. On the phone accept the data notice, tap "Escanear QR del PC" (both on the same Wi-Fi) and confirm the fingerprint.
> 4. On the phone open "Permisos" and tap "Configurar paso a paso". 5. On the PC open Messages, Calls, Files and Notifications.
> A demonstration video is attached: [enlace del video].

## Guion del video (2–3 minutos, con datos de prueba)

1. Abrir la app por primera vez: se ve el aviso general; mostrar «Ahora no» y volver a abrirlo; «Aceptar».
2. Vincular con el QR.
3. «Configurar paso a paso»: en cada permiso se lee el aviso, se acepta y se concede; omitir uno y mostrar que la app sigue.
4. En el PC: enviar un SMS de prueba, ver el registro de llamadas, explorar archivos, recibir la notificación de prueba.
5. Mostrar el aviso permanente del servicio y «Desvincular».

## Si Google rechaza un permiso

Leer el motivo, corregir la justificación o el aviso y reenviar. Si lo niega de forma definitiva, publicar en Play una versión sin ese permiso y
mantener la completa en GitHub. No reenviar sin cambios: las infracciones repetidas pueden suspender la cuenta.
