; Instalador de SyncroUmbral para Windows 10/11 (NSIS 3, por usuario, sin permisos de administrador).
;
; Compilar (ver instalador/compilar.ps1, que pasa estas definiciones):
;   makensis /DVERSION=0.7.1 /DEXE=..\desktop-slint\target\release\syncroumbral.exe /DOUTFILE=SyncroUmbral-0.7.1-setup.exe syncroumbral.nsi
;
; Qué hace:
;   - Copia syncroumbral.exe a %LOCALAPPDATA%\SyncroUmbral (la misma carpeta que usaban la versión Tauri e instalar.ps1).
;   - Se registra en Configuración > Aplicaciones > Aplicaciones instaladas (HKCU), con desinstalación normal y silenciosa.
;   - Acceso directo en el menú Inicio (y en el escritorio, opcional).
;   - Inicio con Windows en segundo plano (clave Run de HKCU), salvo que el usuario lo haya desactivado en la app.
; /DPRUEBA: variante solo para pruebas automáticas (no cierra ni abre la app).
; Silencioso: setup.exe /S   ·   Desinstalación silenciosa: uninstall.exe /S (conserva los datos del usuario).
; Los datos (móviles vinculados, claves, ajustes) viven en %APPDATA%\app.syncroapp.umbral, nunca en la carpeta del programa.

!ifndef VERSION
  !error "Falta /DVERSION=x.y.z"
!endif
!ifndef EXE
  !error "Falta /DEXE=ruta\a\syncroumbral.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "SyncroUmbral-${VERSION}-setup.exe"
!endif

!define APPNAME   "SyncroUmbral"
!define PUBLISHER "SyncroApp"
!define WEB       "https://www.syncroapp.dev/syncroumbral/"
!define REPO      "https://github.com/JDR404/syncroumbral"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\SyncroUmbral"
!define RUNKEY    "Software\Microsoft\Windows\CurrentVersion\Run"
!define APPROVED  "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
!define DATADIR   "$APPDATA\app.syncroapp.umbral"

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma
RequestExecutionLevel user
Name "${APPNAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\SyncroUmbral"
InstallDirRegKey HKCU "${UNINSTKEY}" "InstallLocation"
BrandingText "${APPNAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=0 "ProductName" "${APPNAME}"
VIAddVersionKey /LANG=0 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileDescription" "Instalador de ${APPNAME}"
VIAddVersionKey /LANG=0 "CompanyName" "${PUBLISHER}"
VIAddVersionKey /LANG=0 "LegalCopyright" "GPL-3.0-or-later"

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

!define MUI_ICON   "..\desktop-slint\icon.ico"
!define MUI_UNICON "..\desktop-slint\icon.ico"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\syncroumbral.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Abrir SyncroUmbral"
!define MUI_FINISHPAGE_SHOWREADME ""
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Crear acceso directo en el escritorio"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CrearEscritorio
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Spanish"

; Cierra la app si está abierta (solo la del usuario actual) para poder reemplazar el ejecutable.
!macro CerrarApp
!ifndef PRUEBA
  ExpandEnvStrings $0 "%USERNAME%"
  nsExec::Exec 'taskkill /F /IM syncroumbral.exe /FI "USERNAME eq $0"'
  Pop $1
  Sleep 800
!endif
!macroend

Function CrearEscritorio
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\syncroumbral.exe" "" "$INSTDIR\syncroumbral.exe" 0
FunctionEnd

Section "SyncroUmbral" SecMain
  SectionIn RO
  !insertmacro CerrarApp
  SetOutPath "$INSTDIR"
  SetOverwrite on
  File "/oname=syncroumbral.exe" "${EXE}"
  Delete "$INSTDIR\syncroumbral.old"        ; resto de una actualización anterior
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Menú Inicio (el mismo nombre que usaba instalar.ps1: se reemplaza, no se duplica).
  CreateShortcut "$SMPROGRAMS\${APPNAME}.lnk" "$INSTDIR\syncroumbral.exe" "" "$INSTDIR\syncroumbral.exe" 0

  ; Inicio con Windows, salvo que el usuario lo desactivara en la app (archivo autostart-off). Si lo desactivó en
  ; Configuración de Windows, esa marca (StartupApproved) no se toca: se respeta.
  ${IfNot} ${FileExists} "${DATADIR}\autostart-off"
    WriteRegStr HKCU "${RUNKEY}" "${APPNAME}" '"$INSTDIR\syncroumbral.exe" --background'
  ${EndIf}

  ; Aplicaciones instaladas.
  WriteRegStr   HKCU "${UNINSTKEY}" "DisplayName"          "${APPNAME}"
  WriteRegStr   HKCU "${UNINSTKEY}" "DisplayVersion"       "${VERSION}"
  WriteRegStr   HKCU "${UNINSTKEY}" "Publisher"            "${PUBLISHER}"
  WriteRegStr   HKCU "${UNINSTKEY}" "DisplayIcon"          "$INSTDIR\syncroumbral.exe,0"
  WriteRegStr   HKCU "${UNINSTKEY}" "InstallLocation"      "$INSTDIR"
  WriteRegStr   HKCU "${UNINSTKEY}" "UninstallString"      '"$INSTDIR\uninstall.exe"'
  WriteRegStr   HKCU "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr   HKCU "${UNINSTKEY}" "URLInfoAbout"         "${WEB}"
  WriteRegStr   HKCU "${UNINSTKEY}" "HelpLink"             "${REPO}"
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" "$0"

  ; En modo silencioso no hay página final: se abre en segundo plano para que quede activo como tras un reinicio.
!ifndef PRUEBA
  ${If} ${Silent}
    Exec '"$INSTDIR\syncroumbral.exe" --background'
  ${EndIf}
!endif
SectionEnd

Section "Uninstall"
  !insertmacro CerrarApp

  ; Solo se quita la entrada de inicio si es la de esta instalación.
  ReadRegStr $0 HKCU "${RUNKEY}" "${APPNAME}"
  ${If} $0 == '"$INSTDIR\syncroumbral.exe" --background'
  ${OrIf} $0 == '$INSTDIR\syncroumbral.exe --background'
    DeleteRegValue HKCU "${RUNKEY}" "${APPNAME}"
    DeleteRegValue HKCU "${APPROVED}" "${APPNAME}"
  ${EndIf}

  ; Menú contextual «Enviar al móvil» (lo crea la app si el usuario lo activa).
  DeleteRegKey HKCU "Software\Classes\*\shell\SyncroUmbral"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\SyncroUmbral"

  Delete "$SMPROGRAMS\${APPNAME}.lnk"
  Delete "$DESKTOP\${APPNAME}.lnk"
  Delete "$INSTDIR\syncroumbral.exe"
  Delete "$INSTDIR\syncroumbral.old"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"                         ; solo si quedó vacía: no se borra nada ajeno
  DeleteRegKey HKCU "${UNINSTKEY}"

  ; Datos del usuario: solo con confirmación expresa (en silencioso se conservan).
  ${IfNot} ${Silent}
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "¿Borrar también sus datos de SyncroUmbral (móviles vinculados, claves y ajustes)?$\r$\n$\r$\nSi elige No, se conservan por si vuelve a instalarlo." IDNO conservar
      RMDir /r "${DATADIR}"
    conservar:
  ${EndIf}
SectionEnd
