# 04 · Compatibilidad multiplataforma

Nada de esta sección está probado en Linux ni macOS: es análisis del código y de las bibliotecas usadas. Clasificación: **Compatible**, **Compatible con adaptación**, **Limitado**, **No compatible**, **Pendiente de validación**.

## Punto de partida

- **Portable sin cambios [I]**: el agente de red (tokio, rustls, tungstenite, mdns-sd), la criptografía, el protocolo, Slint + winit (X11, Wayland, macOS), `tray-icon` (Linux con GTK/AppIndicator, macOS barra de menús), `arboard` (portapapeles), adb y scrcpy (existen para los tres sistemas), `ureq`.
- **Atado a Windows**: 10 archivos con `cfg(windows)` y 4 programas externos de Windows. El propio código declara «SyncroUmbral de escritorio es solo para Windows» (`vault.rs:63`).

## APIs específicas y su equivalente

| Capacidad (archivo) | Windows hoy | Linux | macOS |
|---|---|---|---|
| Cifrado de secretos (`vault.rs`) | DPAPI | Secret Service vía `keyring` (GNOME Keyring, KWallet); sin servicio → derivar de la clave de respaldo | Keychain vía `keyring` |
| Bloquear la sesión (`glue.rs:1808`) | `LockWorkStation` | `loginctl lock-session` (systemd) o D-Bus `org.freedesktop.ScreenSaver.Lock`; varía por escritorio | No hay API pública estable; `CGSession -suspend` desapareció; alternativa: dormir la pantalla. **Limitado** |
| Inicio automático (`autostart.rs`) | Clave Run | `~/.config/autostart/syncroumbral.desktop` (XDG) | LaunchAgent en `~/Library/LaunchAgents` o `SMAppService` (macOS 13+) |
| Menú «Enviar al móvil» (`glue.rs context_menu`) | `HKCU\Software\Classes\*\shell` | Nautilus scripts, Dolphin service menus, Thunar custom actions: uno por gestor | Acción rápida de Finder (extensión firmada): **Limitado** |
| Carpetas conocidas (`places.rs`) | `SHGetKnownFolderPath` | `xdg-user-dirs` (crate `dirs`) | `dirs` |
| Marca de origen de archivos recibidos (S-06) | Zone.Identifier | No existe equivalente | Atributo `com.apple.quarantine` |
| Silenciar otras apps en llamadas (`audio.rs`) | Core Audio por sesión | PipeWire/PulseAudio (`pactl`), por flujo | Sin API por aplicación: **No compatible** (degradar: no silenciar) |
| Posición del aviso (`glue.rs:1101`) | Área de trabajo | winit no da el área útil; Wayland no permite posicionar ventanas: **Limitado** | Compatible con adaptación (`NSScreen.visibleFrame`) |
| Mensajería web (`glue.rs:1016`) | Edge `--app` | Chromium/Chrome/Edge si están instalados; si no, navegador por defecto sin ventana propia | Igual que Linux |
| Abrir enlaces/carpetas | `explorer` | `xdg-open` | `open` |
| Cerrar procesos (instalador) | `taskkill` | `pkill -u` | `pkill` |

## Matriz de capacidades

| Capacidad | Windows | Linux | macOS | Adaptación requerida |
|---|---|---|---|---|
| Inicio automático | Compatible (validado 2026-11-21) | Compatible con adaptación | Compatible con adaptación | Adaptador por sistema |
| Segundo plano y bandeja | Compatible | Limitado: GNOME sin AppIndicator no muestra iconos de bandeja; `tray-icon` exige bucle GTK en Linux | Compatible con adaptación (barra de menús) | Hilo GTK en Linux; plan B: ventana mínima |
| Interfaz Slint | Compatible | Pendiente de validación (X11 y Wayland) | Pendiente de validación | Fuentes y DPI |
| Avisos emergentes propios | Compatible | Limitado en Wayland (posición) | Compatible con adaptación | Opción: notificaciones nativas (D-Bus / UserNotifications) |
| Conexión con el móvil | Compatible | Compatible con adaptación: cortafuegos (ufw/firewalld) y mDNS (Avahi) | Compatible con adaptación: aviso de red local de macOS 15 | Documentación y permisos |
| Bloqueo de sesión | Compatible | Limitado (según escritorio) | Limitado | Ocultar la opción donde no exista |
| Apps del móvil (scrcpy) | Compatible | Compatible (scrcpy nativo, a menudo en el gestor de paquetes) | Compatible (Homebrew o binario) | Descarga y huella por sistema en `tools.rs` |
| Mensajería web | Compatible (Edge) | Compatible con adaptación | Compatible con adaptación | Detectar navegador Chromium |
| Instalación | Compatible (NSIS por usuario) | Compatible con adaptación (AppImage) | Limitado sin cuenta de desarrollador de Apple | Ver 10 |
| Actualizaciones | Compatible | Compatible con adaptación (AppImage se reemplaza a sí misma) | Limitado: reemplazar un `.app` firmado exige volver a firmar | Asset por sistema |
| Permisos | Por usuario, sin admin | Por usuario | Red local, notificaciones, acceso a carpetas (TCC) | Textos y guía |
| Silenciar audio en llamadas | Compatible | Compatible con adaptación | No compatible | Desactivar en macOS |

## Distribuciones Linux

- **Soporte oficial propuesto**: Ubuntu LTS (24.04+) y Fedora (última), GNOME y KDE, X11 y Wayland. Son las que más usuarios cubren y ofrecen Secret Service y systemd.
- **Compatibles sin garantía**: Debian 12+, Linux Mint (Cinnamon tiene bandeja nativa), otras con glibc reciente.
- **Formato**: AppImage primero (un archivo, sin root, actualizable por el propio actualizador, sin *sandbox* que rompa adb/scrcpy/red local). Flatpak: su *sandbox* dificulta lanzar adb/scrcpy y Edge; posterior y opcional. `.deb`/`.rpm`: cuando haya demanda.

## macOS

- **Técnicamente posible**: Slint, winit, tray-icon, red y criptografía funcionan en Apple Silicon e Intel (compilación `aarch64-apple-darwin` y `x86_64-apple-darwin`, binario universal).
- **Distribución profesional**: requiere cuenta Apple Developer (99 USD/año), firma con Developer ID y notarización; sin ellas Gatekeeper bloquea la app para usuarios normales. Sin Mac propio no se puede probar: hace falta hardware o un runner `macos-latest` (gratis en repos públicos) solo para compilar y pruebas sin interfaz.
- **Funciones degradadas**: bloqueo de sesión, silenciar audio por app, menú del Finder.

## Bloqueos técnicos

1. Ninguno impide Linux; el trabajo es la capa de plataforma (A-03) y empaquetar.
2. macOS: el bloqueo es económico y de hardware (firma, notarización, pruebas), no de código.
3. Wayland: posicionar el aviso emergente; aceptar que lo coloque el compositor o usar notificaciones nativas.
