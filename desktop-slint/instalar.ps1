# Instala SyncroUmbral (interfaz nativa) para el usuario actual, sin permisos de administrador.
# Sustituye a la versión Tauri si estaba instalada en la misma carpeta (mismos móviles vinculados, claves y ajustes).
#   cargo build --release            (genera target\release\syncroumbral.exe)
#   powershell -ExecutionPolicy Bypass -File instalar.ps1
param([string]$Exe = (Join-Path $PSScriptRoot "target\release\syncroumbral.exe"))

if (-not (Test-Path $Exe)) { throw "No existe $Exe. Compila antes con: cargo build --release" }
$dir = Join-Path $env:LOCALAPPDATA "SyncroUmbral"
$dest = Join-Path $dir "syncroumbral.exe"

Get-Process syncroumbral -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item $Exe $dest -Force

# Inicio con Windows, en segundo plano (solo el icono de la bandeja). Se escribe con Set-ItemProperty: pasar comillas a
# reg.exe desde Windows PowerShell 5.1 las pierde y el valor quedaba sin comillas. Si el usuario lo desactivó, se respeta.
$off = Join-Path $env:APPDATA "app.syncroapp.umbral\autostart-off"
if (-not (Test-Path $off)) {
    $run = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
    Set-ItemProperty -Path $run -Name SyncroUmbral -Value ('"{0}" --background' -f $dest) -Type String
    $v = (Get-ItemProperty -Path $run -Name SyncroUmbral).SyncroUmbral
    if ($v -ne ('"{0}" --background' -f $dest)) { Write-Warning "El inicio con Windows quedó mal escrito: $v" }
}

# Acceso directo en el menú Inicio.
$lnk = Join-Path ([Environment]::GetFolderPath("Programs")) "SyncroUmbral.lnk"
$s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
$s.TargetPath = $dest
$s.IconLocation = $dest
$s.Save()

Start-Process $dest
Write-Host "SyncroUmbral instalado en $dir y en marcha."
