# Registro de funciones

Qué tiene SyncroUmbral, qué se está haciendo y qué falta. Se actualiza en cada entrega. Estados: **Hecho**, **En curso**, **Pendiente**,
**Investigar** (puede no ser posible), **No** (descartado, con el motivo).

Comparación de referencia: Microsoft Phone Link (Enlace a Windows). Lo que sigue de él viene de lo que se sabe de su funcionamiento y puede
variar por modelo de teléfono y versión.

## Frente a Phone Link

| # | Función | Estado | Notas |
|---|---|---|---|
| P1 | Notificaciones del móvil en el PC, con respuesta y botones | Hecho | Ventana propia (ligera). Además admite llamadas y reproductor |
| P2 | Archivos del móvil (explorador, enviar y traer) | Hecho | |
| P3 | Apps del móvil en una ventana del PC | Hecho | Cualquier Android con adb y scrcpy (Phone Link lo limita por fabricante) |
| P4 | Contestar y colgar llamadas desde el PC | Hecho | |
| P5 | Arrastrar y soltar desde el Explorador | Hecho | Soltar sobre la ventana envía a la carpeta del móvil abierta. Además, clic derecho en un archivo: «Enviar al móvil» (ver M7). La suelta con el ratón no se pudo probar de forma automática: el envío por el menú del Explorador usa el mismo camino y sí está probado |
| P6 | Portapapeles PC al móvil | Hecho | `clip_set` |
| P7 | Portapapeles móvil al PC | Hecho | Botón «Copiar al PC» en la notificación permanente (abre una actividad transparente que lo lee en primer plano, único modo permitido desde Android 10). La pulsación del botón está probada; el viaje completo del texto no se pudo comprobar de forma automática |
| P8 | Galería con miniaturas y recientes | Hecho | Todos los álbumes (MediaStore): cámara, capturas, WhatsApp, descargas...; miniaturas por tandas, vídeos marcados, seleccionar y traer al PC (Descargas\SyncroUmbral\Galería). Probado con 842 elementos en 10 álbumes |
| P9 | Contactos del móvil | Hecho | Con búsqueda. Permiso opcional |
| P10 | Hacer llamadas desde el PC (teclado, contactos, historial) | Hecho | Marcar, llamar a un contacto o devolver una llamada del historial. Probado marcando un código de servicio inocuo |
| P11 | SMS y MMS: leer, buscar y escribir | Hecho (solo SMS) | Conversaciones y mensajes probados con el móvil; el envío está implementado pero no se probó con un destinatario real. **MMS no se lee y RCS no es posible** (Google no da acceso). Android no guarda en Mensajes lo enviado por otra app: el PC lo muestra en pantalla. Pedir permisos de SMS junto al acceso a notificaciones puede hacer saltar Play Protect en algunos móviles; aquí instaló sin avisos |
| P12 | Audio de la llamada en el PC (Bluetooth) | No (decisión) | Windows no ofrece a los programas normales el perfil de manos libres (HFP) como pasarela de audio; haría falta un controlador propio o una app de Microsoft. Fuera del alcance de una app ligera sin controladores. Alternativa ya disponible: contestar y colgar desde el PC con el audio en el móvil |
| P13 | Punto de acceso del móvil desde el PC | No (investigado) | Comprobado en un Pixel con Android reciente: `adb shell cmd wifi` solo ofrece consultas del punto de acceso, no encenderlo, y una app normal no puede hacerlo. Exigiría root o una app con permisos de sistema |
| P14 | Notificaciones nativas de Windows (Centro de notificaciones) | No (decisión) | Se mantiene la ventana propia: más ligera, con respuesta, botones del reproductor y llamadas. Las nativas de un programa sin empaquetar no admiten bien esas acciones |
| P15 | Instalador firmado y actualización automática | Parcial | **Actualización hecha**: comprueba GitHub cada 6 h, muestra un aviso y, al aceptar, descarga, verifica la huella SHA-256 de la misma publicación, reemplaza el ejecutable y reinicia. **Firma pendiente**: cuesta dinero (certificado o Azure Trusted Signing); sin ella SmartScreen avisa. La huella protege de descargas dañadas, no de una publicación comprometida |
| P16 | iPhone | No | Apple no permite este tipo de integración |
| P17 | Conexión fuera de la red local con cuenta en la nube | No | Contradice el enfoque local y privado |

## Mejoras que Phone Link no tiene

| # | Función | Estado | Notas |
|---|---|---|---|
| M0 | Bloqueo y desbloqueo de SyncroUmbral por presencia, con botones en la notificación | Hecho | Función central |
| M1 | Bloquear también la sesión de Windows al alejarte | Hecho | Opcional (apagado por defecto): pasados 15 s sin el móvil bloquea Windows. Desbloquear Windows con el móvil queda fuera: exige un componente de inicio de sesión y es arriesgado. No se probó el bloqueo real para no bloquear el PC en pruebas; la lógica de espera sí |
| M2 | Copiar al portapapeles los códigos de verificación (2FA) con un clic | Hecho | Botón «Copiar código» en la tarjeta y en el aviso emergente; se detecta en cualquier notificación (también SMS) que hable de códigos |
| M3 | Reglas de notificaciones (por app, palabra, horario) y modo concentración | Parcial | Hecho: silenciar el aviso emergente por palabras. Por app ya existía (`notif_mute`). **Pendiente**: horarios y modo concentración |
| M4 | Ocultar el contenido de las notificaciones mientras compartes pantalla | Hecho | Interruptor: avisos y lista muestran solo la app y «Contenido oculto». Es manual; no detecta que compartes pantalla |
| M5 | Buscar el móvil: hacerlo sonar desde el PC y ver su batería | Hecho | Suena al máximo (alarma) hasta 60 s y restaura el volumen. La batería ya se ve en el panel lateral |
| M6 | Copia automática de fotos al PC en tu red de casa | Hecho (sin comprobar la red) | Cada 5 min, con el móvil conectado, copia las fotos nuevas (hechas desde que se activa) a Imágenes\SyncroUmbral\Copia. No distingue «red de casa»: copia siempre que el móvil esté conectado. No probado de forma automática en una ronda real |
| M7 | Enviar al móvil con un clic (enlaces, texto, archivos) desde el Explorador y el navegador | Parcial | Hecho para archivos y carpetas: botón en Ajustes añade «Enviar al móvil con SyncroUmbral» al clic derecho (probado con `--send`). **Pendiente**: enlaces y texto desde el navegador |
| M8 | Búsqueda única en archivos, notificaciones y mensajes (local) | Pendiente | Ahora hay búsqueda solo en contactos |
| M10 | Interfaz cuidada: tipografía, iconos, panel derecho, aprovechamiento de pantalla grande | Hecho (0.5.0) | Mejoras visuales pendientes: ver la lista de ideas en docs/SLINT.md |
| M11 | Mensajería: servicios propios (cualquier web https) y más servicios incluidos | Hecho (0.6.0) | |
| M12 | Actualizaciones firmadas | Hecho (0.6.0) | Clave privada fuera del repositorio |
| M9 | Varios PC y varios móviles | Pendiente | El PC ya admite varios móviles vinculados; falta un cambio de móvil activo y que el móvil recuerde varios PC |

## Cómo se decide qué sigue
1. Lo que cierra una diferencia con Phone Link y es barato va primero (lote 1).
2. Después lo que da más seguridad y control (lote 2).
3. Lo que depende de dinero o de una investigación (firma, Bluetooth, punto de acceso) se anota aquí con su motivo y no se promete.
