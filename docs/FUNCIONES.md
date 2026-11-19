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
| P5 | Arrastrar y soltar desde el Explorador | En curso | winit lo permite |
| P6 | Portapapeles PC al móvil | Hecho | `clip_set` |
| P7 | Portapapeles móvil al PC | En curso | Android 10+ no deja leerlo en segundo plano: botón en la notificación que abre una actividad transparente |
| P8 | Galería con miniaturas y recientes | En curso | Se recorren DCIM, Pictures y Screenshots con el permiso de archivos que ya tiene |
| P9 | Contactos del móvil | En curso | Permiso de contactos |
| P10 | Hacer llamadas desde el PC (teclado, contactos, historial) | En curso | Permisos de llamar y de registro de llamadas |
| P11 | SMS y MMS: leer, buscar y escribir | Pendiente | Lote 2. **RCS no es posible**: Google no da acceso; solo se ve por notificación |
| P12 | Audio de la llamada en el PC (Bluetooth) | Investigar | Necesitaría usar el Bluetooth de Windows como manos libres |
| P13 | Punto de acceso del móvil desde el PC | Investigar | Android no deja a una app normal encenderlo; quizá con adb |
| P14 | Notificaciones nativas de Windows (Centro de notificaciones) | Pendiente | Como opción; la ventana propia sigue siendo la principal (más ligera y con respuesta y llamadas) |
| P15 | Instalador firmado y actualización automática | Pendiente | Lote 3. La firma cuesta dinero (certificado o Azure Trusted Signing); la actualización por GitHub con hash no |
| P16 | iPhone | No | Apple no permite este tipo de integración |
| P17 | Conexión fuera de la red local con cuenta en la nube | No | Contradice el enfoque local y privado |

## Mejoras que Phone Link no tiene

| # | Función | Estado | Notas |
|---|---|---|---|
| M0 | Bloqueo y desbloqueo de SyncroUmbral por presencia, con botones en la notificación | Hecho | Función central |
| M1 | Bloquear también la sesión de Windows al alejarte | Pendiente | Lote 2. Desbloquear Windows con el móvil queda fuera: exige un componente de inicio de sesión y es arriesgado |
| M2 | Copiar al portapapeles los códigos de verificación (2FA) con un clic | En curso | Se detectan en notificaciones y SMS |
| M3 | Reglas de notificaciones (por app, palabra, horario) y modo concentración | Pendiente | Lote 2 |
| M4 | Ocultar el contenido de las notificaciones mientras compartes pantalla | Pendiente | Lote 2 |
| M5 | Buscar el móvil: hacerlo sonar desde el PC y ver su batería | En curso | |
| M6 | Copia automática de fotos al PC en tu red de casa | Pendiente | Lote 3 |
| M7 | Enviar al móvil con un clic (enlaces, texto, archivos) desde el Explorador y el navegador | Pendiente | Lote 3 |
| M8 | Búsqueda única en archivos, notificaciones y mensajes (local) | Pendiente | Lote 3 |
| M9 | Varios PC y varios móviles | Pendiente | |

## Cómo se decide qué sigue
1. Lo que cierra una diferencia con Phone Link y es barato va primero (lote 1).
2. Después lo que da más seguridad y control (lote 2).
3. Lo que depende de dinero o de una investigación (firma, Bluetooth, punto de acceso) se anota aquí con su motivo y no se promete.
