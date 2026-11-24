# 08 · Mejoras y funcionalidades

Solo se proponen cosas que resuelven un problema observado o una carencia frente al objetivo (Phone Link local y ligero en Windows, luego Linux).

| ID | Propuesta | Clase | Problema que resuelve | Valor | Complejidad | Impacto 3 SO | Mantenimiento | Prioridad |
|---|---|---|---|---|---|---|---|---|
| F-01 | Aviso «Actualizado a X» con novedades | Corrección indispensable | H-01: la actualización parece un cierre | Alto | S | Igual en los 3 | Bajo | P1 |
| F-02 | Rollback automático del actualizador | Estabilidad | H-05 | Alto | M | Igual | Bajo | P2 |
| F-03 | Registro general de errores con rotación | Estabilidad | A-04: fallos sin rastro | Alto para soporte | S | Igual | Bajo | P1 |
| F-04 | Panel «Diagnóstico» en Ajustes: inicio automático, puertos, red, versión, herramientas | UX | H-06: 5 reinicios para diagnosticar | Alto | S–M | Adaptador por SO | Medio | P2 |
| F-05 | «Exportar diagnóstico» (zip con logs y estado, sin secretos) | Mantenimiento | Soporte a otros usuarios | Medio | S | Igual | Bajo | P2 |
| F-06 | Gestión de espacio: tamaño de sesiones web y «Vaciar caché»; borrar datos de la versión Tauri | UX | H-08: 2 GB | Medio | S | Igual | Bajo | P2 |
| F-07 | Instancia única sin puerto fijo | Estabilidad | A-05 | Bajo-medio | M | Por SO | Bajo | P3 |
| F-08 | Detección de compatibilidad al arrancar (Edge/Chromium, adb, cortafuegos, mDNS) con mensajes claros | UX | Funciones que fallan sin explicar | Medio | M | Por SO | Medio | P3 |
| F-09 | Modo degradado explícito: ocultar lo no soportado según `Capability` | Multiplataforma | Evitar funciones fingidas en Linux/macOS | Alto en E3 | S con E2 | Clave para 3 SO | Bajo | P2 (con E3) |
| F-10 | Copia de seguridad de la configuración (móviles vinculados, ajustes) exportable con la clave de respaldo | Funcionalidad | Cambiar de PC sin volver a vincular todo | Medio | M | Igual | Medio | P3 |
| F-11 | Reglas de notificaciones con horario y modo concentración (M3) | Funcionalidad | Pendiente declarado | Medio | M | Igual | Medio | P3 |
| F-12 | Búsqueda única en notificaciones, mensajes y archivos (M8) | Funcionalidad | Pendiente declarado | Medio | M | Igual | Medio | P3 |
| F-13 | Varios PC por móvil (M9) | Funcionalidad | Pendiente declarado | Medio | L | Igual | Alto | P3 |
| F-14 | Verificación de integridad de la instalación (huella del exe frente a la publicación) | Seguridad | Detectar exe manipulado | Bajo | S | Igual | Bajo | P3 |

**No se recomiendan ahora**: configuración portable (choca con DPAPI/Keychain y con el inicio automático), notificaciones nativas de Windows (decisión ya documentada P14), audio Bluetooth de llamadas (P12), conexión fuera de la red local (P17, contradice el enfoque).

## Cómo funciona hoy la actualización automática (explicación)

1. **Cuándo mira**: al arrancar y luego cada 6 horas (`start_update_check`, `glue.rs:1898`), o al pulsar «Buscar actualizaciones» en Acerca de.
2. **Qué consulta**: la API pública de GitHub, la publicación más reciente de `JDR404/syncroumbral` (`releases?per_page=1`). Compara su etiqueta con la versión compilada dentro del exe (`CARGO_PKG_VERSION`). Si es mayor, muestra la franja «Hay una versión nueva… Actualizar ahora».
3. **Nada se instala sin tu clic**: solo al pulsar «Actualizar ahora» (`do_update`):
   - descarga `SHA256SUMS.txt` y `SHA256SUMS.txt.sig`;
   - comprueba la **firma ECDSA P-256** de esa lista con la clave pública incrustada en la app (`UPDATE_PUBKEY`); la privada solo existe en el PC del autor, así que ni alguien que robe la cuenta de GitHub puede publicar una actualización aceptada;
   - descarga `SyncroUmbral-X-windows-x64.exe` (máx. 100 MB) y compara su SHA-256 con la lista firmada;
   - renombra el exe en uso a `syncroumbral.old` (Windows permite renombrar un ejecutable en marcha), escribe el nuevo en su lugar (si falla, restaura el viejo), actualiza la versión en «Aplicaciones instaladas», lanza el nuevo con `--background` y cierra el viejo.
4. **Por qué no viste nada**: ese `--background` es el mismo modo del inicio con Windows (solo icono en la bandeja) y la nueva versión no sabe que viene de una actualización. Es el hallazgo H-01; la mejora F-01 añade el aviso con las novedades.
5. **Qué conserva**: todos los datos (`%APPDATA%\app.syncroapp.umbral`), la entrada de inicio (apunta a la misma ruta) y los accesos directos.
