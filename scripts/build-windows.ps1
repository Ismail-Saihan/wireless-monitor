<#
.SYNOPSIS
    Automated Windows installer build script for Wireless Virtual Monitor.
.DESCRIPTION
    Builds the production frontend bundle and packages standard Windows
    NSIS (.exe) and MSI (.msi) turnkey installers with bundled virtual display driver assets.
#>

[CmdletBinding()]
param(
    [switch]$SkipFrontend = $false,
    [ValidateSet("nsis", "msi", "all")]
    [string]$Target = "all"
)

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host " Building Wireless Virtual Monitor Windows Installers     " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Environment and PATH validation
Write-Host "[1/5] Checking build environment & prerequisites..." -ForegroundColor Yellow
$CargoPath = "$env:USERPROFILE\.cargo\bin"
if (Test-Path "$CargoPath\cargo.exe") {
    $env:PATH = "$CargoPath;$env:PATH"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "Rust toolchain ('cargo') not found. Please install Rust from https://rustup.rs/"
    exit 1
}

if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    Write-Error "Node.js not found. Please install Node.js v18+."
    exit 1
}

Write-Host "  -> Cargo: $(cargo --version)" -ForegroundColor Green
Write-Host "  -> Rustc: $(rustc --version)" -ForegroundColor Green
Write-Host "  -> Node:  $(node --version)" -ForegroundColor Green

# 2. Application icon assets validation / generation
Write-Host "`n[2/5] Validating application icon suite..." -ForegroundColor Yellow
$IconFile = Join-Path $RepoRoot "src-tauri\icons\icon.ico"
if (-not (Test-Path $IconFile)) {
    Write-Host "  -> Synthesizing application icons..." -ForegroundColor Gray
    python scripts/generate_icons.py
}
Write-Host "  -> Application icons ready." -ForegroundColor Green

# 3. Build frontend bundle
if (-not $SkipFrontend) {
    Write-Host "`n[3/5] Building frontend production bundle (Vite + Tailwind)..." -ForegroundColor Yellow
    pnpm run build
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Frontend build failed."
        exit $LASTEXITCODE
    }
    Write-Host "  -> Frontend build successful (dist/ ready)." -ForegroundColor Green
} else {
    Write-Host "`n[3/5] Skipping frontend build as requested." -ForegroundColor Gray
}

# 4. Target mapping
Write-Host "`n[4/5] Preparing packaging targets ($Target)..." -ForegroundColor Yellow
$TauriArgs = @("run", "tauri", "build")
if ($Target -eq "nsis") {
    $TauriArgs += @("--target", "nsis")
} elseif ($Target -eq "msi") {
    $TauriArgs += @("--target", "msi")
}

# 5. Build Tauri installer packages
Write-Host "`n[5/5] Compiling Rust backend and generating Windows installers..." -ForegroundColor Yellow
Write-Host "Executing: pnpm $($TauriArgs -join ' ')" -ForegroundColor Gray
pnpm @TauriArgs

if ($LASTEXITCODE -ne 0) {
    Write-Error "Tauri installer bundling failed."
    exit $LASTEXITCODE
}

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host " Windows Installers Built Successfully!                   " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "Generated installer artifacts:" -ForegroundColor Cyan
Get-ChildItem -Path "src-tauri\target\release\bundle" -Recurse -Include "*.exe", "*.msi" | ForEach-Object {
    Write-Host "  [+] $($_.FullName) ($([math]::Round($_.Length / 1MB, 2)) MB)" -ForegroundColor Green
}
