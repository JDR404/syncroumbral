# Seguridad de SyncroUmbral

Análisis del diseño y de lo revisado en el código (revisión interna, 2026-11-19; no es una auditoría independiente).

## Cómo se conectan el PC y el móvil
- El **PC es el servidor** (puerto TCP 47600) y el **móvil el cliente**: WebSocket sobre **TLS 1.2/1.3**.
- El PC tiene un certificado **autofirmado**. Su huella SHA-256 viaja en el **código QR** de vinculación y el móvil solo confía en ese certificado exacto (*pinning*): no usa autoridades de certificación, así que no hay certificado «de otro» que sirva.
- **Vinculación**: el QR contiene un token aleatorio de 128 bits de **un solo uso y 5 minutos**. El móvil envía su clave pública (ECDSA P-256, generada en el **Android Keystore** y no exportable).
- **Cada conexión**: reto-respuesta. El PC envía 32 bytes aleatorios y el móvil los firma con su clave privada. Sin esa firma no hay sesión.
- **Presencia**: un latido cada 2 s dentro de la sesión autenticada; 10 s sin latido y el PC se bloquea.
- En el PC, la clave privada del certificado, la clave de respaldo (Argon2) y la lista de móviles se guardan **cifradas con DPAPI** (ligadas a tu usuario de Windows) y los archivos de configuración llevan **sello de integridad**.

## ¿Se puede interceptar?
| Escenario | Resultado |
|---|---|
| Alguien escucha el tráfico en tu Wi-Fi | Solo ve TLS cifrado; no obtiene archivos, notificaciones ni claves |
| Ataque «hombre en el medio» (ARP/DNS/mDNS falsos) en la misma red | El móvil rechaza cualquier certificado distinto del fijado: la conexión falla y no envía nada. Un servidor falso no puede presentar el certificado del PC sin su clave privada (protegida con DPAPI) |
| Un equipo de la red se hace pasar por tu móvil | Sin la clave privada del Keystore no puede firmar el reto; tras 8 fallos su dirección se bloquea 60 s |
| Fuerza bruta del token de vinculación | 128 bits, caduca a los 5 min y se gasta al usarse; además hay bloqueo por origen |
| Desde internet | El puerto no se publica en el router. Además, desde 0.6.0 el PC **descarta conexiones de direcciones públicas** (solo red local, Tailscale/CGNAT y el propio equipo) |
| Móvil robado y desbloqueado | Es la llave: podría abrir el PC. Por eso el botón «Desbloquear» de la notificación exige tener el teléfono desbloqueado, y puedes quitar el móvil desde el PC (corta la sesión al instante) |
| Software malicioso ya ejecutándose en tu PC con tu usuario | Puede leer lo que ve la aplicación (memoria, archivos que tú puedes leer). Ningún diseño local lo evita; mantén Windows y el antivirus al día |
| Alguien ve el QR (mirando tu pantalla) | Puede vincular su móvil durante 5 min **si el PC está desbloqueado** (solo se genera con la aplicación abierta); verás la vinculación en la lista y en `security.log` |

## Qué se revisó y qué se corrigió
Corregido en 0.6.0:
1. **(Media) Quitar un móvil no cortaba la sesión abierta.** El PC seguía atendiendo al móvil quitado. Ahora lo corta en el siguiente latido y el móvil se desvincula. Hay prueba automática.
2. **(Media) Actualizaciones**: antes la huella del ejecutable venía de la misma publicación de GitHub; quien tuviera la cuenta podía publicar un ejecutable y su huella. Ahora la lista de huellas va **firmada (ECDSA P-256)** y la clave pública está incrustada en la aplicación; la clave privada no está en el repositorio.
3. **(Baja) Exposición del puerto**: el servidor escuchaba en todas las interfaces y aceptaba cualquier origen. Ahora solo atiende redes locales.
4. **(Baja) Capacidades con efecto fuera del móvil** (llamar y enviar SMS desde el PC): ahora tienen un tope (6 llamadas y 15 SMS cada 10 min) por si el PC estuviera comprometido.
5. **(Funcional con efecto en seguridad)** Los ajustes de la interfaz pisaban el archivo de ajustes sellado del agente; ahora son archivos separados.

Revisado y sin hallazgos: mensajes previos a la autenticación limitados a 16 KB y 12 s, máximo de conexiones sin autenticar (16 en total, 3 por origen), tamaño máximo de mensaje 4 MB, nombres de archivo recibidos saneados (sin salir de la carpeta, sin nombres reservados de Windows, marca de la web), componentes de Android no exportados salvo los necesarios, sin copia de seguridad de la app (`allowBackup=false`), sin tráfico en claro, intents de prueba solo en compilaciones depurables.

## Limitaciones conocidas (no resueltas)
- **El reto firmado no enlaza la sesión TLS** (firma `umbral-v1 ‖ nonce`, sin la huella del PC). No es explotable hoy porque el móvil solo habla con el PC fijado, pero lo correcto es incluir la huella; cambiaría el protocolo y exigiría actualizar PC y móvil a la vez. Pendiente para una v2.
- **mDNS** anuncia el nombre del equipo y los 16 primeros caracteres de la huella a toda la red local. No da acceso, pero revela que SyncroUmbral está en uso.
- **Puerto local 47601** (segunda instancia / «Enviar al móvil»): solo escucha en 127.0.0.1, pero cualquier proceso local puede pedir que se envíe un archivo **a tu propio móvil** (no a otro destino).
- Los ejecutables **no están firmados con certificado de Windows**: SmartScreen puede avisar. La actualización automática sí verifica la firma propia.
- Si la **clave de firma de publicaciones se pierde**, las actualizaciones automáticas dejan de aceptarse hasta publicar una versión con otra clave instalada a mano.
- La clave de respaldo (para desbloquear sin el móvil) protege con Argon2 y espera creciente tras 8 fallos; una clave débil sigue siendo débil.

## Cómo informar de un problema
Abre una incidencia en el repositorio sin datos sensibles, o escribe al desarrollador (contacto en el README).
