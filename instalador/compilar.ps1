# Genera el instalador de Windows a partir del ejecutable ya compilado.
#   cd desktop-slint; cargo build --release
#   powershell -ExecutionPolicy Bypass -File instalador\compilar.ps1
# Busca makensis en el PATH, en Program Files o en la copia que descarga Tauri. Salida: instalador\SyncroUmbral-<versión>-setup.exe
param([string]$Exe = (Join-Path $PSScriptRoot "..\desktop-slint\target\release\syncroumbral.exe"))
$ErrorActionPreference = "Stop"

$version = (Select-String -Path (Join-Path $PSScriptRoot "..\desktop-slint\Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
if (-not (Test-Path $Exe)) { throw "No existe $Exe. Compila antes con: cargo build --release" }

$candidatos = @(@(
    (Get-Command makensis -ErrorAction SilentlyContinue).Source,
    "${env:ProgramFiles(x86)}\NSIS\makensis.exe",
    "$env:ProgramFiles\NSIS\makensis.exe",
    "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe"
) | Where-Object { $_ -and (Test-Path $_) })
if (-not $candidatos) { throw "No se encontró makensis (NSIS 3). Instálalo: winget install NSIS.NSIS" }
$makensis = $candidatos[0]

$out = "SyncroUmbral-$version-setup.exe"
Push-Location $PSScriptRoot
try {
    & $makensis /V2 "/DVERSION=$version" "/DEXE=$((Resolve-Path $Exe).Path)" "/DOUTFILE=$out" syncroumbral.nsi
    if ($LASTEXITCODE -ne 0) { throw "makensis falló ($LASTEXITCODE)" }
} finally { Pop-Location }
Write-Host "Instalador: $(Join-Path $PSScriptRoot $out)"
