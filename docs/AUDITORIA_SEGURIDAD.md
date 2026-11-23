# Auditoría de seguridad de SyncroUmbral

Fecha: 2026-11-07. Alcance: aplicación de escritorio (Tauri/Rust), aplicación Android (Kotlin), protocolo entre ambas, descarga de herramientas,
clientes web integrados y documentación. Versión 0.1.0.

**Método.** Revisión del código completo, pruebas automáticas y manuales contra el servidor real, comprobaciones sobre los binarios y el APK,
`cargo audit` y `npm audit`. Cada hallazgo indica si está **comprobado** (con prueba) o si es un **análisis** del código.
No es una prueba de penetración externa. Esta auditoría no pudo usar el Pixel (la depuración inalámbrica se apagó), así que lo que depende
del teléfono es análisis de código, no prueba en vivo.

## 0. Estado de las correcciones (2026-11-08)

Tras la auditoría se aplicaron y **probaron** (pruebas automáticas, 37 en total, y comprobación en vivo con el Pixel 9 Pro y el ejecutable real):

| ID | Estado | Qué se hizo y cómo se comprobó |
|---|---|---|
| S-01 | Corregido | La clave TLS se guarda con DPAPI (`key.dpapi`); la `key.der` en claro se migra y se borra. Prueba automática y perfil nuevo sin `key.der`. |
| S-02 | Mitigado | `devices.json`, `password.phc` y `muted.json` llevan sello HMAC con llave DPAPI. Editado a mano: se ignora, no se autoriza ningún móvil y la clave original deja de valer. Comprobado en vivo con el exe real. Límite: código corriendo con tu usuario sigue pudiendo usar DPAPI. |
| S-03 | Pendiente | Presencia no es proximidad (diseño). Segundo factor Bluetooth LE queda para el largo plazo. |
| S-04 | Corregido | Un código equivocado ya no gasta el bueno; comparación en tiempo constante. Prueba automática y en vivo. |
| S-05 | Corregido | El QR se valida (IPv4, puerto, huella, código) y el móvil pide confirmar mostrando nombre, dirección y huella corta, que el PC muestra también. Comprobado en el Pixel. |
| S-06 | Corregido | Mark of the Web en todo lo recibido; ejecutables y accesos directos de Compartir con sufijo `.recibido`. Comprobado (flujo `Zone.Identifier` leído). |
| S-07 | Corregido | Compartir pide confirmación en el móvil; tope de 2 GB por archivo y 4 GB por sesión en el PC. Prueba automática; el diálogo se vio en el Pixel (ver nota sobre auto-clic). |
| S-08 | Corregido | Mensajes de 16 KiB como máximo antes de autenticar, 16 handshakes simultáneos en total y 3 por origen, plazo de 12 s, bloqueo de 60 s tras 8 fallos. Comprobado en vivo: 200 KB y 10 MB cortados, 40 conexiones inactivas reducidas a 3, bloqueo tras 8 códigos falsos. |
| S-09 | Corregido | APK de producción no depurable (`run-as` falla), R8, firmado con clave propia, 618 KB; los intents de prueba solo existen en depuración. Instalado y probado en el Pixel. |
| S-10 | Parcial | Se añadió el aviso de servicios de accesibilidad activos. El resto (notificación persistente, "Desconectar todo") sigue pendiente. |
| S-11 | Parcial | Actualizador hecho (0.4.0) con firma ECDSA propia de la lista de huellas (0.6.0) e instalador (0.7.2). Falta la firma de código de Windows (SignPath Foundation no la aprobó por ahora). |
| S-12 | Corregido | mDNS ya no publica el nombre del equipo; la dirección descubierta se guarda solo tras una sesión autenticada. |
| S-13 | Sin cambios | Aviso `crypto-bigint` no explotable; añadir `cargo audit` a la compilación sigue pendiente. |
| S-14 | Corregido | Los datos se redirigen con `--data-dir=` (argumento); las variables `UMBRAL_*` solo se leen en compilaciones de depuración. |
| S-15 | Corregido | Compilar `webdebug` en release da error de compilación. |
| S-16 | Corregido | "Bloquear ahora" ya no se levanta al reconectar el móvil si hay clave. Prueba automática. |
| S-17 | Pendiente | Permisos de las webs integradas (requiere acceder al manejador de permisos de WebView2). |
| S-18 | Corregido | adb r37.0.2 y scrcpy v4.1 con versión fija y SHA-256 incrustado (el SHA-1 de adb coincide con el manifiesto de Google). |
| S-19 | Corregido en PC | `security.log` con rotación. Aviso en el móvil al vincular: la confirmación obligatoria cubre el caso. |

**Cambios posteriores que amplían la superficie (2026-11-08):** para poder contestar llamadas desde el PC la app del móvil pide dos permisos nuevos,
`ANSWER_PHONE_CALLS` (contestar y colgar llamadas normales) y `SYSTEM_ALERT_WINDOW` (que Android deje abrir la pantalla de una llamada de WhatsApp desde
segundo plano). Ambos son opcionales: se conceden con botones en la app y se revocan en Ajustes. Ahora un PC vinculado comprometido podría contestar o colgar
llamadas (no oír su audio). Recomendación pendiente: confirmar en el móvil las acciones de llamada del PC cuando el teléfono esté en uso.

**Hallazgos nuevos durante las pruebas:**
- Un móvil desvinculado que reintenta contaba como ataque y se bloqueaba a sí mismo: corregido (solo cuentan códigos falsos y firmas inválidas).
- El móvil tenía un servicio de accesibilidad de auto-clic activo que confirmó solo los diálogos de SyncroUmbral. Ninguna app puede impedirlo; ahora SyncroUmbral avisa en pantalla.
- El código de llamadas entrantes no compilaba (constante inexistente) y por eso nunca se había probado en el móvil: corregido.

## 1. Resumen ejecutivo (auditoría original, 2026-11-07)

SyncroUmbral tiene una base criptográfica y de aislamiento sólida para un MVP: autenticación mutua real, certificado fijado, clave del móvil no
exportable, contenido web remoto sin acceso a las funciones internas y entradas validadas. Sus puntos débiles están en **lo que rodea a esa base**:
los secretos y la configuración del PC están sin proteger en disco, el bloqueo se puede saltar editando archivos con acceso a la cuenta de Windows,
la vinculación y la recepción de archivos del móvil admiten abusos, y los binarios no están firmados.

| Dimensión | Nota (1 a 10) | Comentario |
|---|---|---|
| Criptografía y autenticación | 8 | TLS con huella fijada, reto-respuesta ECDSA, Argon2id. Sin fallos de diseño. |
| Control de acceso y bloqueo | 5 | Sólido contra el uso normal; saltable con acceso a los archivos del usuario. |
| Superficie de red | 6 | Un puerto, handshake con plazos; sin límites antes de autenticar. |
| Datos en reposo | 4 | Clave TLS, lista de dispositivos y sesiones web sin protección adicional. |
| Interfaz y contenido remoto | 8 | CSP estricta, sin HTML dinámico con datos externos, ACL probada. |
| Seguridad en Android | 5 | Keystore bien usado; APK de depuración, componentes exportados, permisos muy amplios. |
| Cadena de suministro y distribución | 4 | Sin firma de código, sin actualizador, herramientas descargadas con verificación parcial. |
| Privacidad | 8 | Todo local, sin telemetría ni servidores propios. |

**Nota global del MVP: 6 sobre 10.** Apto para uso personal en una red de confianza; no para distribuirlo a terceros sin cerrar los puntos de abajo.

### Los tres riesgos más críticos

1. **Secretos y configuración del PC sin proteger (S-01, S-02):** quien tenga acceso a la cuenta de Windows puede suplantar al PC ante el móvil o saltarse el bloqueo.
2. **Vinculación manipulable (S-04, S-05):** un dispositivo de la red puede impedir vincular, y el móvil acepta cualquier QR sin pedir confirmación.
3. **Entrega de archivos desde el móvil sin controles (S-06, S-07):** cualquier app del móvil puede mandar archivos al PC, sin marca de origen y sin límite de espacio.

## 2. Modelo de amenazas

| Atacante | Qué puede intentar | Cubierto por |
|---|---|---|
| A. Persona con acceso físico al PC (sesión de Windows abierta, móvil ausente) | Ver o usar SyncroUmbral | Pantalla de bloqueo, clave. **Saltable (S-02).** |
| B. Dispositivo en la misma red | Suplantar, espiar, bloquear la vinculación, agotar recursos | TLS fijado, firma del móvil. Fallos S-04, S-08, S-12. |
| C. Malware en el PC con el mismo usuario | Robar claves, sesiones, suplantar | Poco: S-01, S-02, S-14. Inherente a la cuenta. |
| D. App maliciosa en el móvil | Enviar archivos al PC, leer su propio entorno | Parcial: S-07, S-09. |
| E. Contenido hostil (notificaciones, nombres de archivo, páginas web) | Inyección en la interfaz, salir del aislamiento | **Bien cubierto.** |
| F. Cadena de suministro | Binarios falsos, herramientas manipuladas | Parcial: S-11, S-18. |

## 3. Lo que está bien (con evidencia)

| Control | Evidencia |
|---|---|
| **TLS 1.2/1.3 con certificado fijado por huella SHA-256.** El móvil no confía en autoridades de certificación. | Comprobado: un servidor con otra huella es rechazado (prueba automática). |
| **Autenticación del móvil por reto-respuesta** (ECDSA P-256 sobre un reto aleatorio de 32 bytes por conexión), con la clave privada en el Android Keystore, no exportable. | Comprobado: firma de otra clave, firma de un reto anterior y dispositivo desconocido, todos rechazados. |
| **Código de vinculación de 128 bits, un solo uso, caduca a los 5 minutos.** | Comprobado: la reutilización se rechaza. |
| **Clave de respaldo con Argon2id** (m=19 MiB, t=2, p=1), sal aleatoria y espera creciente tras 5 fallos (de 30 s a ~32 min). | Comprobado en el perfil limpio: 5 intentos, mensaje de espera, la clave buena también se rechaza durante la espera. |
| **Contenido web remoto aislado.** Desde WhatsApp integrado en la ventana principal, 14 comandos internos, escuchar y emitir eventos y `window.open` están bloqueados por la lista de permisos. | Comprobado en vivo (página remota inspeccionada). |
| **CSP estricta** (`default-src 'self'`, sin scripts ni estilos externos) y **sin HTML dinámico con datos externos**: notificaciones, nombres de archivo y apps se insertan con `textContent`. | Comprobado por revisión de todo `innerHTML`: solo constantes y el QR que genera el propio programa. |
| **Los comandos sensibles exigen la aplicación desbloqueada** (revisados los 45 comandos IPC). | Comprobado. Los que no lo exigen son inofensivos (`get_state`, `lock`, redimensionar la ventana emergente). |
| **Validación de entradas:** nombres de paquete (anti inyección de shell), dirección y código de `adb pair`, nombres de archivo del móvil (sin `..`, `\`, `:`, nombres reservados de Windows), rutas del móvil confinadas al almacenamiento, entradas zip fuera de carpeta ignoradas. | Comprobado con pruebas automáticas y en vivo (un paquete con `;` fue rechazado y no creó nada en el móvil). |
| **Integridad de transferencias:** tamaño final verificado, temporales `.umbral-part` que nunca quedan con el nombre final, limpieza al cancelar o bloquear. | Comprobado con hash en el Pixel (300 archivos idénticos, 0 temporales). |
| **Bloqueo efectivo para lo que muestra:** borra interfaz, memoria, temporales; cierra ventanas, apps y transferencias; las notificaciones no se guardan mientras está bloqueado. | Comprobado. |
| **Privacidad por diseño:** sin telemetría ni servidores propios, `allowBackup=false`, datos solo en el PC y el móvil. | Revisión. |
| Instalación por usuario (sin administrador), un único puerto TCP, capacidades de Tauri solo para las ventanas `main` y `toast`. | Revisión. |

## 4. Hallazgos

Severidad: Crítico, Alto, Medio, Bajo, Informativo.

| ID | Severidad | Hallazgo | Estado |
|---|---|---|---|
| S-01 | Alto | La clave privada TLS del PC (`key.der`) está **en claro** en `%APPDATA%\app.syncroapp.umbral`. | Comprobado |
| S-02 | Alto | El bloqueo se puede saltar con acceso a los archivos del usuario (`devices.json`, `password.phc`). | Análisis |
| S-03 | Medio | "Presencia" no es proximidad: el móvil desbloquea el PC en cualquier lugar de la red. | Análisis |
| S-04 | Medio | Cualquier intento de vinculación, aun con código erróneo, **consume el código bueno**. | Comprobado |
| S-05 | Medio | El móvil se vincula a cualquier QR sin mostrar a qué PC. | Análisis |
| S-06 | Medio | Los archivos recibidos del móvil se guardan sin la marca de origen de Windows (Mark of the Web). | Análisis |
| S-07 | Medio | Cualquier app del móvil puede enviar archivos y texto al PC (actividad exportada), sin confirmación ni límite de espacio. | Análisis |
| S-08 | Medio | Antes de autenticarse el servidor acepta mensajes de hasta 16 MiB y no limita conexiones. | Comprobado |
| S-09 | Medio | El APK es de **depuración** y está firmado con la clave de depuración. | Comprobado |
| S-10 | Medio | Permisos del móvil muy amplios: comprometer el PC es comprometer el móvil. | Análisis |
| S-11 | Medio | Instalador, `syncroumbral.exe` y APK sin firma de código; sin actualizador. | Comprobado |
| S-12 | Bajo | mDNS publica el nombre del equipo y parte de la huella; la dirección se guarda antes de verificar la sesión. | Análisis |
| S-13 | Bajo | Dependencias: 1 aviso real (`crypto-bigint`, RUSTSEC-2026-0387) y 5 avisos de crates de Linux. | Comprobado |
| S-14 | Bajo | Variables de entorno de diagnóstico siguen activas en la versión instalada. | Comprobado |
| S-15 | Bajo | La función de pruebas `webdebug` abre un puerto de depuración si se empaqueta por error. | Análisis |
| S-16 | Bajo | "Bloquear ahora" se levanta solo si el móvil reconecta. | Análisis |
| S-17 | Informativo | Las webs integradas pueden pedir cámara, micrófono, notificaciones y ubicación. | Análisis |
| S-18 | Informativo | `adb` se descarga de Google solo con TLS (sin hash); scrcpy se verifica con el hash de GitHub. | Análisis |
| S-19 | Informativo | No hay registro de eventos de seguridad ni aviso en el móvil al vincular. | Análisis |

### S-01 Clave privada TLS en claro (Alto)
`key.der` (138 bytes, PKCS#8) es legible por el usuario y los administradores. Quien la copie puede **hacerse pasar por el PC**: el móvil se conecta
al servidor que presente ese certificado, se autentica y recibe peticiones, es decir, el impostor obtiene el control de archivos, notificaciones con
respuesta y apps del móvil. Requiere acceso al disco (malware o acceso físico) y que el móvil se conecte al impostor (misma red, suplantando la dirección).
**Recomendación:** proteger la clave con DPAPI (`CryptProtectData`, ligada a la cuenta de Windows) o guardarla en el almacén de claves no exportable.

### S-02 El bloqueo se salta editando archivos (Alto)
El escenario que el bloqueo debe cubrir (alguien en tu PC, con Windows abierto, mientras te llevas el móvil) es justo el que lo vence: `devices.json`
guarda las claves públicas de los móviles permitidos y se carga al arrancar. Un atacante local puede añadir **su propia clave**, reiniciar SyncroUmbral y
conectarse desde otro equipo de la red, que se autentica como móvil y desbloquea. Lo mismo con `password.phc` (sustituir el hash por el suyo).
Las sesiones web guardadas también son legibles por la cuenta (decisión documentada del proyecto).
**Recomendación:** firmar (HMAC) `devices.json`, `password.phc` y `muted.json` con una clave protegida por DPAPI y rechazar el arranque si no coinciden;
avisar de cualquier dispositivo nuevo. Aun así, el bloqueo sigue siendo una barrera de **privacidad**, no de seguridad frente a quien controle la cuenta de Windows; debe decirse así en la documentación.

### S-03 Presencia no es proximidad (Medio)
Estar en la misma red basta, aunque el móvil esté en otra habitación o en manos de otra persona. No es un fallo de implementación sino una propiedad del diseño.
**Recomendación:** documentarlo y, a medio plazo, añadir Bluetooth LE (intensidad de señal) como segundo factor opcional.

### S-04 El código de vinculación se consume con cualquier intento (Medio, comprobado)
`take_token` retira el código guardado sea correcto o no. Prueba: un dispositivo de la red envió un código falso y, **justo después**, la vinculación legítima con el código
bueno falló. Un atacante en la red impide vincular de forma continua (denegación de servicio).
**Recomendación:** consumir el código solo si coincide, comparar en tiempo constante, limitar intentos por origen y caducar el código tras varios fallos.

### S-05 Vinculación sin confirmación (Medio)
La app del móvil guarda directamente lo que trae el QR (dirección, huella, código). Un QR malicioso (por ejemplo pegado en un cartel o enviado como imagen) vincularía el
móvil al PC del atacante, que obtendría los permisos descritos en S-10. Además `hosts` y `port` no se validan como direcciones.
**Recomendación:** pantalla de confirmación con nombre del PC y un **código corto de verificación** que se muestre igual en ambos lados; validar formato de dirección y puerto.

### S-06 Archivos recibidos sin Mark of the Web (Medio)
Los archivos que llegan con Compartir (y las descargas del móvil) se escriben sin la marca `Zone.Identifier`. Si una app maliciosa del móvil envía un `.exe`, `.lnk`
o `.js`, SmartScreen **no avisará** al ejecutarlo desde `Descargas\SyncroUmbral`.
**Recomendación:** añadir el flujo alterno `Zone.Identifier` (`ZoneId=3`) a todo lo recibido; avisar o bloquear extensiones ejecutables.

### S-07 Cualquier app del móvil puede enviar archivos al PC (Medio)
`ShareActivity` está exportada para poder usar el menú Compartir; cualquier aplicación puede invocarla con archivos o texto. Mientras el móvil esté conectado y la aplicación
desbloqueada, el PC los guarda (hasta 8 GB por archivo y 4 a la vez): relleno del disco o entrega de malware (ver S-06).
**Recomendación:** diálogo de confirmación en el móvil antes de enviar, cuota total y límite de tamaño razonable (por ejemplo 500 MB), y opción para desactivarlo.

### S-08 Sin límites antes de autenticar (Medio, comprobado)
Un mensaje de 10 MB antes de autenticar fue aceptado y procesado (el servidor respondió "se esperaba hello"); más de 16 MiB se corta. Se abrieron **1500 conexiones inactivas** sin
límite (la legítima siguió respondiendo en 9 ms: el impacto actual es pequeño pero no hay tope).
**Recomendación:** tamaño máximo de mensaje de 64 KiB hasta autenticar, límite de conexiones simultáneas y por dirección, y limitación de tasa del handshake.

### S-09 APK de depuración (Medio, comprobado)
`application-debuggable` y certificado `CN=Android Debug`. Con `adb`, `run-as` permite leer los datos privados de la app (dirección y huella del PC, código pendiente).
La clave de depuración es la genérica de cualquier entorno de desarrollo.
**Recomendación:** compilar `release`, no depurable, con minimización (R8) y firmado con una clave propia guardada fuera del repositorio.

### S-10 Permisos amplios en el móvil (Medio)
Acceso a todos los archivos, lista de todas las apps, lectura y respuesta a notificaciones, bloqueo de energía. Es inherente a lo que hace la aplicación, pero convierte
cualquier compromiso del PC autenticado en control del móvil.
**Recomendación:** mostrar en el móvil una notificación persistente con el PC conectado y un botón "Desconectar todo"; pedir confirmación en el móvil para borrados masivos.

### S-11 Sin firma de código ni actualizador (Medio, comprobado)
`syncroumbral.exe`, el instalador y el APK no están firmados. SmartScreen y Play Protect avisan, y se acostumbra al usuario a ignorar advertencias. Sin actualizador no hay cómo distribuir parches.
**Recomendación:** certificado de firma de código y un actualizador con firma verificada.

### S-12 mDNS (Bajo)
El anuncio incluye el nombre del equipo y los 16 primeros caracteres hexadecimales de la huella. Cualquiera en la red puede identificar el equipo. El móvil guarda la dirección **antes**
de verificar la sesión, de modo que anuncios falsos con esa huella podrían desplazar la dirección real de su lista (se redescubre, pero es un ruido innecesario).
**Recomendación:** usar un identificador aleatorio en vez de la huella y guardar la dirección solo tras una sesión autenticada.

### S-13 Dependencias (Bajo, comprobado)
`cargo audit`: 1 vulnerabilidad, `crypto-bigint 0.5.5` (RUSTSEC-2026-0387, variaciones de tiempo) por `p256`. **No es explotable aquí:** SyncroUmbral solo verifica firmas con claves públicas y nunca opera con
claves privadas de `p256`. Cinco avisos de mantenimiento corresponden a dependencias de Linux que no se usan en Windows. `npm audit`: 0 vulnerabilidades.
**Recomendación:** actualizar cuando `p256` publique la versión con la corrección y ejecutar `cargo audit` en cada compilación.

### S-14 Variables de entorno de diagnóstico (Bajo, comprobado)
`UMBRAL_DATA_DIR`, `UMBRAL_INBOX_DIR` y `UMBRAL_MDNS_LOG` siguen activas en la versión instalada; quien controle el entorno del usuario puede redirigir los datos o la recepción de archivos.
Además, WebView2 acepta `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` (ajena a SyncroUmbral): fijarla abre un puerto de depuración con control total de la interfaz.
**Recomendación:** ignorar las variables propias en `release` (solo en depuración) y documentar la de WebView2.

### S-15 La compilación `webdebug` (Bajo)
Esa función abre un puerto de depuración y crea una clave de prueba. El instalador actual se compila **sin** ella (comprobado: puertos 9333 y 9444 cerrados), pero nada impide empaquetarla por error.
**Recomendación:** que el script de compilación de versiones falle si la función está activa.

### S-16 Bloqueo manual levantable por reconexión (Bajo)
Tras "Bloquear ahora", si el móvil se reconecta (un parpadeo de red), la aplicación se desbloquea sin clave. Está documentado, pero sorprende.
**Recomendación:** opción para exigir la clave tras un bloqueo manual.

### S-17 Permisos de las webs integradas (Informativo)
Las páginas pueden solicitar cámara, micrófono, notificaciones y ubicación; WebView2 muestra su aviso. La navegación está limitada a los dominios del servicio (probado en unidad; no se probaron flujos con redirecciones de inicio de sesión de terceros).
**Recomendación:** denegar por defecto ubicación y cámara; permitir micrófono solo en WhatsApp si se usan llamadas.

### S-18 Herramientas descargadas (Informativo)
`adb` se descarga de `dl.google.com` solo con TLS (Google no publica un hash estable); scrcpy se verifica con el SHA-256 de la API de GitHub (protege de manipulación en la red o en el CDN, no de un compromiso de la cuenta del proyecto).
**Recomendación:** fijar versiones concretas con su hash dentro de SyncroUmbral.

### S-19 Sin registro de seguridad (Informativo)
No queda constancia de vinculaciones, intentos fallidos o desbloqueos, y el móvil no avisa cuando se vincula a un PC.
**Recomendación:** registro local con rotación y notificación en el móvil al conectar un PC nuevo.

## 5. Privacidad y cumplimiento

- **Datos personales:** mensajes, archivos y apps se procesan solo en el PC y el móvil del usuario; no hay servidores propios, telemetría ni cuentas. El usuario es el responsable del tratamiento; no hay transferencia a terceros desde SyncroUmbral.
  Para GDPR o la Ley de Habeas Data de Colombia, el uso personal no exige más; si se distribuyera a terceros haría falta política de privacidad y descripción de los datos que la aplicación puede leer (notificaciones, archivos, lista de apps).
- **Servicios de terceros:** se usan los clientes web oficiales de WhatsApp, Instagram, Facebook, Messenger y Telegram (no bibliotecas no oficiales). Reflejar apps de terceros con scrcpy puede ir contra las condiciones de algunas (banca, pagos).
- **Licencias:** Tauri (MIT/Apache-2.0), scrcpy (Apache-2.0, se descarga, no se redistribuye), OkHttp (Apache-2.0), ML Kit / Play Services (propietario de Google, exige servicios de Google en el móvil). No se detectaron dependencias con licencias copyleft fuertes en el código propio; falta un inventario formal (SBOM).

## 6. Hoja de ruta

| Plazo | Acciones |
|---|---|
| **Inmediato (quick wins)** | S-04 consumir el código solo si coincide y limitar intentos. S-08 tope de 64 KiB antes de autenticar y límite de conexiones. S-06 Mark of the Web en lo recibido. S-14 ignorar variables de diagnóstico en `release`. S-12 guardar la dirección solo tras sesión autenticada. S-15 comprobación en el script de compilación. S-16 opción de bloqueo manual estricto. |
| **Corto plazo** | S-01 y S-02 proteger `key.der`, `devices.json`, `password.phc` y `muted.json` con DPAPI y firma. S-05 confirmación de vinculación con código corto y validación de dirección. S-07 confirmación y cuota para Compartir. S-09 APK `release` firmado con clave propia y no depurable. S-19 registro de seguridad y aviso en el móvil. S-17 permisos de las webs. |
| **Largo plazo** | S-11 firma de código y actualizador firmado. S-03 Bluetooth LE como segundo factor. S-10 modo de confirmaciones en el móvil. S-18 versiones fijadas con hash. Pruebas de fuzzing del protocolo, `cargo audit` y SBOM en la compilación, revisión externa antes de distribuir. |

## 7. Qué no cubre esta auditoría

- Pruebas de las funciones que dependen del móvil durante este ciclo (el Pixel estaba desconectado): lo del lado Android es análisis de código.
- El comportamiento de la plataforma (Keystore con respaldo de hardware, Play Services, WebView2) se asume correcto; no se auditó.
- Flujos de inicio de sesión con redirecciones de terceros dentro de las webs integradas.
- Una revisión independiente de un tercero y pruebas de fuzzing.

