# install.ps1 — installe le xerboxion-core (xion) sur Windows.
# Lancer:  powershell -ExecutionPolicy Bypass -File install.ps1
$ErrorActionPreference = "Stop"
$Prefix = "$env:LOCALAPPDATA\xerboxion"
$Self = (Resolve-Path "$PSScriptRoot\..").Path
Write-Host "== xerboxion-core installer — Windows -> $Prefix =="

$dist = "$Self\dist\x86_64-pc-windows-gnu"
if (Test-Path "$dist\xerboxion-rt.exe") {
  Write-Host "  source: binaire prebuild"
  $Bin = "$dist\xerboxion-rt.exe"; $Plox = "$dist\ploxions"
} elseif (Get-Command cargo -ErrorAction SilentlyContinue) {
  Write-Host "  source: build cargo (les ploxions wasm nécessitent bash : WSL ou Git Bash)..."
  Push-Location $Self
  cargo build --release -p xerboxion-host
  bash scripts/build-ploxions.sh
  Pop-Location
  $Bin = "$Self\target\release\xerboxion-rt.exe"; $Plox = "$Self\target\ploxions"
} else {
  Write-Host "  !! Rust absent. Installe-le: https://rustup.rs  (ou copie un dossier dist\ prebuild)."
  exit 1
}

New-Item -ItemType Directory -Force -Path "$Prefix\ploxions" | Out-Null
Copy-Item $Bin "$Prefix\xerboxion-rt.exe" -Force
Copy-Item "$Plox\*.wasm" "$Prefix\ploxions\" -Force

# launcher .bat
$run = "@echo off`r`n`"$Prefix\xerboxion-rt.exe`" serve `"$Prefix\ploxions`" --addr 127.0.0.1 --port 8730 --state-dir `"$Prefix\state`""
Set-Content -Path "$Prefix\xion.bat" -Value $run -Encoding ASCII

# raccourci menu Démarrer
try {
  $ws = New-Object -ComObject WScript.Shell
  $lnk = $ws.CreateShortcut("$env:APPDATA\Microsoft\Windows\Start Menu\Programs\xion.lnk")
  $lnk.TargetPath = "$Prefix\xion.bat"; $lnk.Save()
  Write-Host "  raccourci 'xion' ajouté au menu Démarrer"
} catch { }

Write-Host "  installé: $Prefix\xerboxion-rt.exe + ploxions"
Write-Host "  lancer:   $Prefix\xion.bat   puis ouvrir http://127.0.0.1:8730/healthz"
Write-Host "== fini =="
