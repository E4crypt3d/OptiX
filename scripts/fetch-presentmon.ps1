# Fetches the PresentMon console app (MIT, from Intel/GameTechDev) into
# src-tauri/resources/ so the release build can bundle it as a Tauri resource.
# The Benchmark panel needs PresentMon64.exe next to the Optix executable.
#
# Usage (Windows PowerShell):
#   powershell -ExecutionPolicy Bypass -File scripts/fetch-presentmon.ps1
#
# Requires an internet connection. Runs in CI before the Windows installer
# build (see .github/workflows/ci.yml) and can be run locally the same way.

param(
    [string]$Version = "2.3.1"
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$OutDir = Join-Path $Root "src-tauri\resources"
$WorkDir = Join-Path $env:TEMP "optix-presentmon-$Version"
$Zip = Join-Path $WorkDir "presentmon.zip"

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

$Url = "https://github.com/GameTechDev/PresentMon/releases/download/v$Version/PresentMon-$Version-x64.zip"
Write-Host "Downloading $Url"
Invoke-WebRequest -Uri $Url -OutFile $Zip
Expand-Archive -Path $Zip -DestinationPath $WorkDir -Force

# PresentMon 2.x ships PresentMon.exe; 1.x shipped PresentMon64.exe. Either is
# fine — the app looks for PresentMon64.exe by that name.
$Source = Get-ChildItem -Path $WorkDir -Recurse -Filter "PresentMon*.exe" | Select-Object -First 1
if (-not $Source) {
    throw "No PresentMon*.exe found in the downloaded archive"
}
Copy-Item -Path $Source.FullName -Destination (Join-Path $OutDir "PresentMon64.exe") -Force
Write-Host "PresentMon $Version -> $OutDir\PresentMon64.exe"
