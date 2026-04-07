param(
  [switch]$Release,
  [string]$Bind = '127.0.0.1:18081',
  [switch]$OpenChrome
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$profileDir = if ($Release) { 'release' } else { 'debug' }
$exe = Join-Path $root "target\$profileDir\wx_station.exe"
$url = "http://$Bind/"
$logPath = Join-Path $root 'wx_station.log'

if (-not (Test-Path $exe)) {
  $cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
  if (-not (Test-Path $cargo)) {
    throw "cargo.exe not found. Install Rust with rustup first."
  }

  Push-Location $root
  try {
    if ($Release) {
      & $cargo build --release
    } else {
      & $cargo build
    }
    if ($LASTEXITCODE -ne 0) {
      throw "Cargo build failed with exit code $LASTEXITCODE"
    }
  } finally {
    Pop-Location
  }
}

$env:WX_STATION_BIND = $Bind
$env:WX_STATION_ROOT = $root
$env:WX_STATION_LOG = $logPath
Write-Host "Starting Weather Station at $url"
Write-Host "Weather log: $logPath"
if ($OpenChrome) {
  Start-Process 'chrome.exe' $url -ErrorAction SilentlyContinue | Out-Null
}
Push-Location $root
try {
  & $exe
} finally {
  Pop-Location
}
exit $LASTEXITCODE
