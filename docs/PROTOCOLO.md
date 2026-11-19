# Protocolo SyncroUmbral v1

El **móvil es el cliente** y el **PC es el servidor**. Así el mismo protocolo sirve por WiFi
y por cable (`adb reverse tcp:47600 tcp:47600` hace que el móvil llegue al PC en `127.0.0.1:47600`).

## Transporte
- `wss://<host>:47600/` (WebSocket sobre TLS), mensajes de texto JSON.
- Certificado autofirmado del PC. El móvil lo **fija por huella SHA-256** (viene en el QR);
  no usa autoridades de certificación ni valida el nombre de host.

## Emparejamiento (una vez)
El PC muestra un QR con:
```json
{"v":1,"name":"MI-PC","hosts":["192.168.1.5"],"port":47600,"fp":"<sha256 hex del cert>","tok":"<token un solo uso, 5 min>"}
```
Móvil a PC: `{"t":"pair","tok":"...","pub":"<b64 SPKI DER P-256>","name":"Pixel 9 Pro"}`
PC a móvil: `{"t":"paired"}` o `{"t":"error","msg":"..."}`.
El PC guarda la clave pública. La clave privada vive en el Android Keystore y no sale del teléfono.

## Sesión (cada conexión)
1. Móvil a PC: `{"t":"hello","id":"<sha256 hex de la clave pública>"}`
2. PC a móvil: `{"t":"challenge","n":"<32 bytes aleatorios, b64>"}`
3. Móvil a PC: `{"t":"auth","sig":"<b64 DER ECDSA-SHA256 sobre 'umbral-v1' || n>"}`
4. PC a móvil: `{"t":"ok","hb":2000}` (intervalo de latido en ms) o `{"t":"error",...}`
5. Móvil a PC cada `hb` ms: `{"t":"hb","seq":N}`. PC responde `{"t":"ack","seq":N}`.

## Peticiones del PC al móvil
Solo dentro de una sesión autenticada y con la app desbloqueada.
PC a móvil: `{"t":"req","id":N,"op":"<operación>", ...parámetros}`
Móvil a PC: `{"t":"res","id":N,"ok":true,"data":{...}}` o `{"t":"res","id":N,"ok":false,"err":"mensaje"}`

Las rutas de archivos son relativas al almacenamiento interno (`""` es la raíz) y no pueden salir de él.

| op | parámetros | respuesta |
|---|---|---|
| `ls` | `path` | `path`, `items[{name,dir,size,mtime}]`, `free`, `total` |
| `read` | `path`, `offset`, `len` | `n`, `size`, `eof`, `data` (b64) |
| `write` | `path`, `offset`, `data` (b64), `final` | `n`. Escribe en `<path>.umbral-part` y renombra con `final` |
| `abort` | `path` | borra solo el temporal `<path>.umbral-part` (se permite aunque la app esté bloqueada) |
| `mkdir`, `delete`, `rename` | `path` (+ `name`) | vacío |
| `apps` | | `apps[{pkg,label,version,code,system,enabled,launchable,installed,updated,size}]` |
| `icons` | `pkgs` (máx. 40) | `icons{pkg: png 48 px b64}` |

| `notifs` | | `granted` (si el móvil dio acceso a notificaciones), `items[notificación]` |
| `notif_reply` | `key`, `index`, `text` | rellena el campo de respuesta (RemoteInput) de la acción y la pulsa |
| `notif_action` | `key`, `index` | pulsa una acción sin texto |
| `notif_dismiss` | `key` | descarta la notificación en el móvil |

Notificación: `{key, pkg, app, title, text, sub, when, quiet, clearable, actions:[{index,title,reply,hint}]}`.
Se omiten las notificaciones en curso (música, servicios) y los resúmenes de grupo. `quiet` marca las de importancia
baja: van a la lista pero no abren la ventana emergente.

| `clip_set` | `text` (máx. 100 000) | copia el texto en el portapapeles del móvil |
| `notif_clear_all` | | descarta todas las notificaciones descartables; `n` = cuántas |
| `keep_awake` | `on` | mantiene el teléfono despierto (bloqueo de energía, máx. 6 h) mientras el PC tiene apps abiertas. El PC lo envía aunque esté bloqueado cuando es para soltarlo; el móvil lo suelta solo si se pierde la conexión |

Las notificaciones de llamada (`call: true`) se envían aunque Android las marque "en curso"; `incoming: true` indica llamada entrante (Android 12 o superior).

### Galería, contactos, llamadas, SMS y buscar el móvil (0.4.0)

Todas son peticiones `req` con la app desbloqueada; los permisos del móvil son opcionales y, si faltan, la respuesta es un error legible.

| op | parámetros | respuesta |
|---|---|---|
| `media_recent` | `offset`, `limit` (máx. 60), `refresh` | `items[{path,name,mtime,size,video}]`, `total`. Recorre DCIM, Pictures y Movies (profundidad 3), más nuevos primero |
| `media_thumb` | `paths` (máx. 12) | `thumbs{path: jpeg 240 px b64}` |
| `contacts` | `q` (opcional) | `items[{name,numbers[]}]` (máx. 400) |
| `calllog` | `limit` (máx. 200) | `items[{number,name,type,date,secs}]` (`type` de Android: 1 entrante, 2 saliente, 3 perdida...) |
| `dial` | `number` | inicia la llamada con `TelecomManager.placeCall` |
| `find_ring` | `on` | hace sonar el móvil al máximo (alarma) hasta 60 s; `on` indica si suena |
| `sms_threads` | | `items[{address,name,snippet,date,mine,unread}]` (máx. 60) |
| `sms_list` | `address`, `limit` (máx. 300) | `items[{text,date,mine}]`, más nuevos primero |
| `sms_send` | `address`, `text` (máx. 1000) | envía por SMS (varias partes si hace falta) |
| `phone_perms`, `sms_perms` | | permisos concedidos |

El botón «Copiar al PC» de la notificación permanente lee el portapapeles del móvil y manda `{"t":"share_text","text":"..."}` (ya existente).

## Descubrimiento
El PC anuncia `_umbral._tcp.local.` por mDNS con el puerto 47600 y el TXT `fp=<16 primeros hex de la huella>`.
Si las direcciones guardadas no responden, el móvil busca ese servicio, acepta solo IPv4 de la red (no bucle local ni enlace local)
con la huella esperada, y conecta igualmente con el certificado fijado: anunciarse no da ninguna confianza.

## Mensajes espontáneos del móvil
- `{"t":"status","battery":0-100,"charging":bool}`: cada 30 s.
- `{"t":"share_text","text":"..."}`: texto compartido desde el menú Compartir.
- `{"t":"share_begin","id":N,"name":"...","size":N}`, `{"t":"share_chunk","id":N,"data":"<b64>"}`, `{"t":"share_end","id":N}`:
  archivo compartido. El PC lo escribe en `<nombre>.umbral-part` y solo lo renombra si recibió exactamente `size` bytes; si no, lo borra.
- `{"t":"notif", ...notificación}`: llega o se actualiza (misma `key` reemplaza).
- `{"t":"notif_gone","key":"..."}`: desapareció (descartada, respondida o cancelada por la app).

Con la app bloqueada el PC los ignora y no guarda nada. Al bloquear o irse el móvil se borran de la memoria y se cierra la ventana emergente.

Las acciones sobre apps que Android restringe (abrir, detener, desinstalar, borrar datos, extraer APK)
no viajan por este canal: el PC las ejecuta con `adb`, localizando el móvil por la IP de la sesión.

## Presencia
- Latido válido reciente: `UNLOCKED`.
- Sin latido durante `timeout` (10 s por defecto) o socket cerrado: `LOCKED`
  y se ejecuta la limpieza (memoria, temporales, caché del WebView, transferencias en curso).
- Mientras el móvil está presente manda la presencia; si se va, se bloquea siempre.
- Con el móvil ausente solo se vuelve a `UNLOCKED` con la clave (5 fallos: espera creciente desde 30 s).
- El desbloqueo con clave dura hasta que el usuario pulsa "Bloquear ahora" o el móvil se conecta y se va.
- El móvil puede pedir `{"t":"lock_pc"}` (bloquear) y `{"t":"unlock_pc"}` (levantar un bloqueo con el móvil presente) desde la notificación o la app. En Android la acción "Desbloquear" de la notificación exige tener el teléfono desbloqueado.
- "Bloquear ahora" gana sobre la presencia: sigue bloqueado aunque el móvil esté conectado, hasta usar la clave o hasta que el móvil se reconecte.
