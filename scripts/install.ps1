$ErrorActionPreference = "Stop"

$InstallDir = "$env:USERPROFILE\.zero-agent\bin"
$BridgeDir = Split-Path -Parent $PSScriptRoot | Join-Path -ChildPath "bridge\rust"
$ZeroDir = Split-Path -Parent $PSScriptRoot

Write-Host "Zero-Agent installer"
Write-Host ""
Write-Host "This script builds and installs Zero-Agent from source."
Write-Host ""

# Check for required tools
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Error: cargo not found. Install Rust first."
    exit 1
}

Write-Host "Building Rust bridge..."
Push-Location $BridgeDir
cargo build --release 2>&1 | Write-Host
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Bridge build failed."
    Pop-Location
    exit 1
}
Pop-Location

Write-Host "Installing to $InstallDir..."
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$BridgeBinary = Join-Path $BridgeDir "target\release\zero-agent-bridge.exe"
if (Test-Path $BridgeBinary) {
    Copy-Item $BridgeBinary "$InstallDir\zero-agent-bridge.exe" -Force
} else {
    Write-Host "Error: Bridge binary not found."
    exit 1
}

Write-Host ""
Write-Host "Zero-Agent bridge installed to $InstallDir"
Write-Host ""
Write-Host "Add to your PATH:"
Write-Host "  `$env:PATH = `"$InstallDir;`$env:PATH`""
Write-Host ""
Write-Host "Build from source:"
Write-Host "  cd $ZeroDir"
Write-Host "  zero build src/main.0"
