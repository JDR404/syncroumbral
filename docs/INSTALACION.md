# Instalar SyncroUmbral en otro PC (y en otro móvil)

## Qué necesitas

| En el PC | En el móvil |
|---|---|
| Windows 10 u 11 de 64 bits | Android 8 o superior (probado en Android 17, Pixel 9 Pro) |
| El instalador `SyncroUmbral_0.1.0_x64-setup.exe` | El archivo `app-release.apk` (versión de producción: no depurable, firmada con tu clave) |
| Conexión a Internet solo si falta WebView2 (Windows 11 ya lo trae) | Estar en la **misma red WiFi** que el PC |
| `adb` y `scrcpy` solo para el **modo completo** (SyncroUmbral los descarga por ti, ver paso 5) | Depuración inalámbrica o USB, solo para el **modo completo** |

No hace falta instalar Rust, Node ni Android Studio: eso es solo para compilar (ver el README).

## Dos modos de uso: básico y completo

SyncroUmbral funciona **sin activar nada especial en el móvil** (modo básico). La depuración solo se necesita para el modo completo y es opcional.

| | Básico (sin depuración) | Completo (con depuración inalámbrica o USB) |
|---|---|---|
| Bloqueo por presencia, clave de respaldo | Sí | Sí |
| Archivos, compartir, portapapeles, batería | Sí | Sí |
| Notificaciones con respuesta, avisos de llamada | Sí | Sí |
| Mensajería web con sesión guardada | Sí | Sí |
| Lista de apps con iconos | Sí | Sí |
| Abrir apps del móvil en ventanas del PC | No | Sí |
| Detener, borrar datos, desinstalar, extraer APK | No | Sí |

Conviene saber que **la depuración da control total del móvil al PC emparejado** (no solo a SyncroUmbral: a cualquier programa que corra con tu usuario en ese PC
y use su clave de `adb`), y que **algunas apps, sobre todo de bancos, se niegan a abrirse mientras las Opciones de desarrollador o la depuración están
activas** (igual que con servicios de accesibilidad como los auto-clics). Recomendación: usa el modo básico a diario y activa la depuración solo cuando
quieras abrir una app en el PC; desactívala al terminar. SyncroUmbral muestra el modo actual en Móviles y ajustes, tarjeta "Modo de uso".

La red WiFi no puede tener "aislamiento de clientes" (las redes de invitados suelen tenerlo): PC y móvil deben verse entre sí.

## 1. Instalar en el PC

1. Copia `SyncroUmbral_0.1.0_x64-setup.exe` al otro PC y ejecútalo. Se instala solo para tu usuario, sin pedir administrador.
2. Windows SmartScreen avisará porque el instalador no está firmado: pulsa "Más información" y "Ejecutar de todas formas".
3. Abre SyncroUmbral y crea la **clave de respaldo** (mínimo 6 caracteres). Sirve para abrir la aplicación cuando el móvil no está.

## 2. Firewall de Windows

La primera vez que se abre, Windows pregunta si permite a `syncroumbral.exe` recibir conexiones. Marca **Redes privadas** y pulsa Permitir.
Si no apareció el aviso o lo cerraste, abre PowerShell **como administrador** y ejecuta (cambia la ruta si instalaste en otro sitio):

```
New-NetFirewallRule -DisplayName "SyncroUmbral" -Direction Inbound -Protocol TCP -LocalPort 47600 -Action Allow -Profile Private
```

Si la red está marcada como "Pública" en Windows, el móvil no podrá llegar: cámbiala a "Privada" en Configuración, Red e Internet.

## 3. Instalar en el móvil

1. Pasa `app-release.apk` al móvil (cable, nube, WhatsApp a ti mismo...) y ábrelo. Para generarlo, ver "Compilar el APK de producción" más abajo.
2. Android pedirá permitir "instalar apps desconocidas" para la app desde la que lo abras. Acéptalo.
3. Google Play Protect puede advertir porque el APK no viene de Play Store. Pulsa "Instalar de todas formas".
   **Si en vez de advertir lo bloquea** ("Se bloqueó la app para proteger tu dispositivo. Esta app puede solicitar acceso a datos sensibles"): es la protección
   contra fraude de Google, que rechaza apps instaladas a mano que piden acceso a notificaciones y permisos parecidos (SyncroUmbral los necesita de verdad). Dos salidas:
   - **Instalar por `adb`** (no pasa por ese filtro): con la depuración inalámbrica activa, `adb install -r SyncroUmbral-movil.apk`. Es lo más fiable.
   - Desactivar temporalmente en Play Store, tu perfil, Play Protect, Configuración, "Analizar apps con Play Protect" (y la protección contra fraude mejorada si aparece),
     instalar, y volver a activarla.
   Hay dos APK: `SyncroUmbral-movil.apk` (estándar, con el mínimo de permisos) y `SyncroUmbral-movil-con-superposicion.apk` (añade "Mostrar sobre otras apps" para contestar
   llamadas de WhatsApp desde segundo plano; junto al acceso a notificaciones es lo que más desconfianza despierta en Play Protect). Usa la estándar salvo que la necesites.
   Se genera con `gradlew assembleRelease -PconOverlay=true`.
4. Abre SyncroUmbral en el móvil y concede, en este orden:
   - **Notificaciones** (para que el servicio no se mate en segundo plano).
   - **Permitir acceso a los archivos** (para el explorador de archivos).
   - **Permitir acceso a las notificaciones**: abre la lista de Android, activa "SyncroUmbral". Android 13 o superior bloquea este permiso en apps instaladas a mano: si aparece atenuado o con aviso de "ajuste restringido", ve a Ajustes, Aplicaciones, SyncroUmbral, menú de los tres puntos, "Permitir ajustes restringidos", y vuelve a intentarlo.
   - **Permitir contestar llamadas** y **Permitir mostrar sobre otras apps (WhatsApp)**: solo si quieres contestar llamadas desde el PC (ver "Llamadas" en Uso diario).
   - **Permitir uso en segundo plano (batería)**. En Xiaomi, Samsung, Huawei y similares, desactiva además la optimización de batería de SyncroUmbral y activa el "inicio automático"; si no, el sistema mata el latido y el PC se bloqueará solo.

## 4. Vincular

1. En el PC: pestaña **Móviles y ajustes**, "Mostrar QR de vinculación". Debajo del QR aparece la **huella de este PC** (por ejemplo `bc32-d877-964e`).
2. En el móvil: "Escanear QR del PC". Si no tiene cámara de Google, copia el código de la opción "Sin cámara" y pégalo en el móvil.
3. El móvil **pide confirmar** y muestra el nombre, la dirección y la huella del PC. **Compara la huella con la del PC**; si no coincide o no fuiste tú quien
   mostró el código, cancela. (Un QR ajeno no vincula nada sin tu confirmación.)
4. El PC debe mostrar "(tu móvil) conectado" en verde. El código sirve una sola vez y caduca en 5 minutos.
   Un intento con código equivocado ya no gasta el bueno; tras 8 fallos seguidos desde una misma dirección, esa dirección queda bloqueada un minuto.

Si el móvil muestra un aviso de **servicios de accesibilidad activos**, ten en cuenta que esos servicios pueden pulsar botones por ti, incluidas las confirmaciones de SyncroUmbral.

Desde ahí, con el móvil en la red la app del PC está abierta; si se va, se bloquea y limpia sola.

### Comprobar las notificaciones

En el móvil pulsa **Enviar notificación de prueba**. En el PC debe aparecer una ventana emergente en la esquina inferior derecha
(y en la pestaña Notificaciones) con un campo "Responder". Escribe algo y pulsa Enviar: el móvil publica "Respuesta recibida: ..."
y así queda comprobado que las notificaciones llegan y que el PC puede responderlas.
Si el móvil muestra "notificaciones: no" en su pantalla principal, falta conceder el acceso del paso 3.4.

## 5. Pestaña Aplicaciones: `adb`

Listar apps funciona sin `adb`. Abrir, detener, extraer APK, borrar datos y desinstalar sí lo necesitan.

1. **Instalar `adb`:** en SyncroUmbral, pestaña **Móviles y ajustes**, tarjeta **Herramientas**, pulsa "Instalar" junto a adb. SyncroUmbral descarga una versión fija
   (platform-tools r37.0.2) de los servidores de Google (`dl.google.com`) a su propia carpeta de datos, sin permisos de administrador, y **solo la instala si su SHA-256
   coincide con el incrustado en SyncroUmbral**.
   Si ya tienes adb, lo usa: busca `%ANDROID_HOME%\platform-tools\adb.exe`, luego el que descargó SyncroUmbral y por último `adb` en el `PATH`.
2. En el móvil activa las **Opciones de desarrollador** (Ajustes, Acerca del teléfono, toca 7 veces "Número de compilación").
3. **Por cable:** activa "Depuración USB", conecta el cable y acepta el aviso en el móvil. En SyncroUmbral, "Activar modo cable".
4. **Por WiFi (sin cable):** activa "Depuración inalámbrica" y entra en "Vincular dispositivo con código de sincronización": el móvil muestra una
   dirección IP:puerto y un código de 6 dígitos. En SyncroUmbral, tarjeta **Depuración inalámbrica**, pulsa "Detectar el móvil en la red" (rellena la dirección),
   escribe el código y pulsa "Emparejar". Solo hace falta la primera vez. Después SyncroUmbral localiza el móvil solo.
   Android apaga la depuración inalámbrica al cambiar de red o reiniciar: hay que volver a activarla (no hace falta emparejar otra vez).

## 6. Abrir apps del móvil en ventanas del PC: `scrcpy`

El botón **Abrir en el PC** de la pestaña Aplicaciones usa [scrcpy](https://github.com/Genymobile/scrcpy) (código abierto, licencia Apache 2.0).
La app corre en una pantalla virtual del móvil, aparte de su pantalla física, así que **el móvil puede estar bloqueado**.

1. Instala scrcpy desde SyncroUmbral: tarjeta **Herramientas**, "Instalar" junto a scrcpy. Se descarga la versión probada (v4.1) de GitHub (`Genymobile/scrcpy`) y **solo se instala si su SHA-256
   coincide con el incrustado en SyncroUmbral**. También sirve el que ya tengas (`winget install Genymobile.scrcpy` o en el `PATH`).
2. Necesita lo mismo que el paso 5: `adb` y la depuración inalámbrica o USB activa.
3. Una ventana por app. Al bloquear SyncroUmbral (o irse el móvil) todas las ventanas se cierran de forma ordenada.

**Con el teléfono bloqueado la app sigue visible.** Android duerme la pantalla a los 10 s cuando el teléfono está bloqueado (ignora el tiempo que
hayas puesto en Ajustes), y al dormirse el teléfono apaga también la pantalla virtual: la app se veía negra hasta tocarla. Por eso, mientras haya
apps abiertas en el PC, SyncroUmbral mantiene el teléfono despierto (un bloqueo de energía de la app del móvil, que se suelta solo si se pierde la conexión
o pasan 6 horas) y apaga solo el panel físico si nadie lo está usando. Al cerrar la última ventana, un teléfono bloqueado vuelve a dormirse;
uno desbloqueado no se toca. En la pantalla virtual no se dibuja la barra de tareas del sistema.
**Detener, borrar datos o desinstalar** una app cierra también su ventana en el PC.

Límites: las apps que bloquean la captura de pantalla (bancos, algunas de pagos) se ven en negro. El móvil sigue haciendo el trabajo (batería y datos),
así que debe estar encendido y en la red.

## 7. Pestaña Mensajería web

WhatsApp, Instagram, Facebook, Messenger y Telegram en sus **clientes web oficiales**, mostrados **dentro de la propia ventana de SyncroUmbral**
(eliges el servicio arriba y aparece en el área principal; se adapta al tamaño de la ventana y se oculta al cambiar de pestaña), con su sesión guardada en el PC.
Inicias sesión una vez (en WhatsApp, escaneando el QR) y las siguientes veces entra directo, incluso con el móvil apagado.

- La sesión se guarda en `%APPDATA%\app.syncroapp.umbral\web\<servicio>`. Es como el perfil de un navegador: quien tenga acceso a tu cuenta de Windows puede leerla.
- Al bloquear SyncroUmbral las ventanas se cierran, pero **la sesión se conserva**. "Olvidar sesión" la borra.
- Las páginas web no pueden usar ninguna función interna de SyncroUmbral (comprobado), y no pueden navegar fuera de los dominios de su servicio: los enlaces externos se abren en tu navegador.
- Las notificaciones de estos servicios llegan por la pestaña Notificaciones (las del móvil), no por las del navegador.

## 8. Uso diario

- **Bandeja del sistema.** Al cerrar la ventana SyncroUmbral sigue en la bandeja (icono junto al reloj) con solo unos 5 MB de memoria, para que las
  notificaciones sigan llegando. La ventana se abre al pulsar el icono, o al volver a abrir SyncroUmbral desde el menú Inicio (nunca corren dos copias).
  Para salir del todo: clic derecho en el icono, "Salir".
- **Iniciar con Windows.** En Móviles y ajustes, tarjeta Funcionamiento: SyncroUmbral arranca en segundo plano al iniciar sesión.
- **Compartir desde el móvil.** En cualquier app del móvil usa Compartir y elige **SyncroUmbral (PC)**: el texto aparece en el PC con un botón Copiar
  y los archivos se guardan en `Descargas\SyncroUmbral` (se verifica que lleguen completos). Solo funciona con la aplicación desbloqueada.
- **Arrastrar y soltar.** En la pestaña Archivos, arrastra archivos o carpetas desde el Explorador de Windows al panel del móvil.
- **Portapapeles.** La barra superior de Archivos copia un texto en el portapapeles del móvil. (Android no deja a una app en segundo plano
  leer el portapapeles, por eso no hay copia automática del móvil al PC: usa Compartir.)
- **Notificaciones.** "Silenciar app" en una notificación deja de mostrar esa app; se reactiva desde la pestaña Notificaciones.
  **"Limpiar todas"** descarta en el móvil todas las notificaciones que Android permite descartar (no las de reproductores de música) y quita lo
  recibido con Compartir.
- **Archivos ocultos.** Los que empiezan por punto (cachés e índices de las apps) no se muestran salvo que marques "Mostrar ocultos".
- **Archivos nuevos en el móvil.** Los archivos que subes desde el PC se registran en el índice de Android para que el administrador de archivos
  y la galería los vean. Algunos administradores de archivos solo muestran ciertos tipos en cada vista: los archivos existen aunque no los listen.
- **Llamadas.** Una llamada entrante (del operador o de WhatsApp, Telegram, Messenger...) muestra una ventana emergente con **Contestar** y **Rechazar**
  (**Colgar** si ya está en curso) y suena un aviso en el PC hasta que se atiende. Mientras suena, SyncroUmbral **silencia el resto de sonidos del PC** (música,
  vídeos...) para que se oiga solo el timbre; al terminar los restaura tal como estaban, y lo que ya tenías en silencio no se toca. Se desactiva en
  Móviles y ajustes, tarjeta Llamadas.
  Para que contestar funcione hacen falta permisos en la app del móvil (botones en su pantalla principal):
  - **Permitir contestar llamadas**: para las llamadas normales (línea del operador). Se contestan y cuelgan con la API de telefonía de Android.
  - Para las llamadas de WhatsApp y similares, contestar abre la pantalla de la llamada y Android 10 o superior prohíbe que una app en segundo plano abra pantallas.
    Por eso **rechazar funcionaba y contestar no**: rechazar no abre nada. SyncroUmbral lo intenta y **comprueba si la llamada realmente se contestó**; si Android lo bloqueó,
    lo dice. La variante con superposición añade **Mostrar sobre otras apps**, que Android sí exime de ese bloqueo.
  SyncroUmbral muestra en Móviles y ajustes qué permiso falta. Son permisos sensibles: concédelos solo si vas a contestar desde el PC; se revocan en Ajustes del móvil.
  **El audio de la llamada sigue en el móvil**: no encontré una forma soportada de que una app de terceros haga de auricular Bluetooth en Windows
  (eso lo hace Phone Link de Microsoft con su propio componente), y Android no permite capturar el audio de las llamadas normales. Si quieres hablar
  con las diademas del PC, usa Phone Link solo para llamadas, o las llamadas de WhatsApp desde la pestaña Mensajería web.
  Comprobado con llamadas simuladas (contestar con la app en segundo plano, rechazar, ausencia de permisos); las llamadas reales dependen de cada app
  y de la versión de Android: si alguna no se contesta, el mensaje de la ventana emergente indica qué falta.
- **Batería.** La cabecera muestra el nivel de batería del móvil.
- **Conexión con la pantalla apagada.** Si el móvil apaga la pantalla y se desconecta y reconecta una y otra vez (el PC se bloquea y desbloquea), casi siempre es el
  **ahorro de batería** o la optimización de batería de Android. SyncroUmbral avisa en la tarjeta del móvil del PC y en la pantalla del móvil. Soluciones: "Permitir uso en
  segundo plano (batería)", desactivar el ahorro de batería, y dejar activa la opción "Conexión con pantalla apagada" (por defecto), que mantiene CPU y Wi-Fi despiertos
  mientras hay sesión con el PC (cuesta algo de batería; se puede desactivar).
- **Carpetas del PC.** En el panel "Este PC" hay accesos directos a Inicio, Escritorio, Descargas, Documentos e Imágenes. Con OneDrive el Escritorio real es
  `C:\Users\<tú>\OneDrive\Escritorio`, no `C:\Users\<tú>\Desktop`; los accesos usan la ruta que dice Windows.

## Cosas que conviene saber al cambiar de PC o de móvil

- **Cada PC tiene su propio certificado y sus propias claves.** Una vinculación no se copia de un PC a otro: hay que vincular de nuevo en el PC nuevo.
- **El móvil recuerda un solo PC.** Al vincularlo con otro PC, olvida el anterior. Un PC sí puede tener varios móviles vinculados. (Soportar varios PC en el mismo móvil está pendiente.)
- Si cambias de móvil: instala el APK, vincula de nuevo y, en el PC, quita el móvil viejo en "Móviles vinculados".
- Si el router cambia la IP del PC no hace falta vincular otra vez: el PC se anuncia en la red (mDNS, `_umbral._tcp`) y el móvil lo busca solo,
  comprobando siempre la huella de su certificado. Si tu red bloquea el multicast, vuelve a escanear el QR.
- **Modo portátil / perfil aparte:** inicia `syncroumbral.exe --data-dir=D:\MiCarpeta` para que SyncroUmbral guarde sus datos en otra carpeta (útil para pruebas o para
  llevar la instalación en una unidad). Es un argumento, no una variable de entorno: así un entorno heredado no puede redirigir dónde se guardan las claves.
  Nota: los datos cifrados con DPAPI (ver abajo) solo se descifran con el mismo usuario de Windows en el mismo equipo, así que una carpeta portátil no sirve en otro PC: se vincula de nuevo.

## Dónde guarda SyncroUmbral sus datos

- **PC:** `%APPDATA%\app.syncroapp.umbral` contiene:
  - `key.dpapi` y `vault.bin`: la clave privada del certificado y la llave de sellado, **cifradas con DPAPI de Windows** (solo tu usuario en este equipo las descifra).
  - `devices.json`, `password.phc` y `muted.json` con su sello (`.sig`): si alguien los edita a mano, SyncroUmbral los ignora, no autoriza ningún móvil, no deja abrir con otra clave y lo anota en `security.log`.
  - `security.log`: vinculaciones, desvinculaciones, firmas inválidas, claves incorrectas, desbloqueos y manipulaciones (con rotación a 256 KB). No contiene secretos.
  - `cert.der` (certificado público), las sesiones de Mensajería web (carpeta `web`) y las herramientas descargadas (carpeta `tools`).
  Las listas de archivos y de apps solo viven en memoria y se borran al bloquear. Las descargas se guardan donde tú elijas.
  Los archivos que llegan del móvil llevan la marca de origen de Windows (SmartScreen avisa al abrirlos); los ejecutables y accesos directos recibidos con Compartir
  se guardan con el sufijo `.recibido`.
  **Límite honesto:** DPAPI y los sellos protegen frente a copias de la carpeta, otras cuentas de Windows y ediciones manuales. Un programa que corra con tu mismo usuario
  de Windows podría usarlos igualmente: el bloqueo de SyncroUmbral es una barrera de privacidad para quien se sienta a tu PC, no una defensa contra malware en tu cuenta.
- **Móvil:** la clave privada está en el Android Keystore y no puede extraerse. Solo se guarda la dirección y la huella del PC.

## Compilar el APK de producción

El APK de producción no es depurable (nadie puede leer sus datos con `adb run-as`), está reducido y ofuscado con R8 y va firmado con **tu propia clave**.
La clave no está en el repositorio; cada quien genera la suya una vez:

```
keytool -genkeypair -v -keystore C:\ruta\segura\umbral-release.jks -storetype PKCS12 -alias umbral -keyalg EC -groupname secp256r1 -sigalg SHA256withECDSA -validity 9125
```

Crea `android\keystore.properties` (ya está en `.gitignore`) con:

```
storeFile=C:/ruta/segura/umbral-release.jks
storePassword=...
keyAlias=umbral
keyPassword=...
```

Y compila: `cd android` y `gradlew assembleRelease` → `app\build\outputs\apk\release\app-release.apk` (unos 600 KB). Guarda la clave: sin ella no podrás
actualizar la app instalada (habría que desinstalar y volver a vincular). El APK de depuración (`assembleDebug`) solo sirve para desarrollo: es depurable y trae intents de prueba. Se instala como otra app
(`app.syncroapp.umbral.debug`), así que puede convivir con la de producción sin tocar su vinculación.

## Desinstalar

- PC: "Aplicaciones instaladas" de Windows, SyncroUmbral. Para borrar también las claves, elimina `%APPDATA%\app.syncroapp.umbral`.
- Móvil: en la app, "Desvincular" (borra la clave) y luego desinstala.

## Si algo falla

| Síntoma | Causa probable | Qué hacer |
|---|---|---|
| El móvil dice "PC no encontrado" | Firewall o red pública | Paso 2; comprueba que ambos están en la misma red |
| Conecta pero el PC se bloquea cada pocos segundos | Android mata el servicio | Optimización de batería e inicio automático (paso 3.4) |
| "Código de vinculación inválido o caducado" | Pasaron 5 minutos o ya se usó | Genera un QR nuevo |
| "Huella del PC no coincide" (el móvil no conecta) | El móvil tiene guardado otro PC | Vuelve a escanear el QR del PC correcto |
| Aplicaciones: "No hay acceso por adb" | Falta `adb` o la depuración | Paso 5 |
| El explorador dice "Falta el permiso de acceso a todos los archivos" | Permiso sin conceder | Botón "Permitir acceso a los archivos" en el móvil |
