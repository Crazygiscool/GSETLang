# GSET Windows Installer
# Run with: irm https://raw.githubusercontent.com/Crazygiscool/GSETLang/main/install.ps1 | iex

param(
    [string]$InstallPath = "$env:LOCALAPPDATA\GSET",
    [switch]$AddToPath = $true
)

$ErrorActionPreference = "Stop"

# Colors for output
function Write-Step { param([string]$Message) Write-Host "[+] $Message" -ForegroundColor Cyan }
function Write-Success { param([string]$Message) Write-Host "[+] $Message" -ForegroundColor Green }
function Write-Info { param([string]$Message) Write-Host "[*] $Message" -ForegroundColor White }
function Write-Warn { param([string]$Message) Write-Host "[!] $Message" -ForegroundColor Yellow }
function Write-Err { param([string]$Message) Write-Host "[X] $Message" -ForegroundColor Red }

Write-Host ""
Write-Host "  ____  _____ " -ForegroundColor Cyan -NoNewline
Write-Host " |  _ \\|  __ \\ " -ForegroundColor White
Write-Host " | |_) | |__) |____      _____  __ __   ____ _ _   _ " -ForegroundColor Cyan
Write-Host " |  _ </|  ___/ _ \\ \\ /\\ / / __| \\ \\ / / _` | | | |" -ForegroundColor White
Write-Host " | |_) | |  | (_) \\ V  V / (__   \\ V / (_| | |_| |" -ForegroundColor Cyan
Write-Host " |____/|_|   \\___/ \\_/\\_/ \\___|  \\_/ \\__,_|\\__, |" -ForegroundColor White
Write-Host "                                              |___/ " -ForegroundColor Cyan
Write-Host "  Generic Syntax Extension Tool - Windows Installer"
Write-Host ""

# Get latest version from GitHub API
Write-Step "Checking for latest version..."
try {
    $ReleasesUrl = "https://api.github.com/repos/Crazygiscool/GSETLang/releases/latest"
    $ReleaseInfo = Invoke-RestMethod -Uri $ReleasesUrl -UseBasicParsing
    $Version = $ReleaseInfo.tag_name -replace '^v', ''
    Write-Info "Latest version: $Version"
} catch {
    Write-Warn "Could not check for latest version, using default"
    $Version = "2.1.2"
}

# Detect architecture
Write-Step "Detecting system architecture..."
if ($env:PROCESSOR_ARCHITECTURE -eq "AMD64" -or (Get-WmiObject Win32_ComputerSystem).SystemType -match "x64") {
    $Arch = "amd64"
} else {
    $Arch = "386"
}
Write-Info "Architecture: $Arch"

# Create download URL
$DownloadUrl = "https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-windows-$Arch.zip"
Write-Step "Download URL: $DownloadUrl"

# Create temp directory
$TempDir = "$env:TEMP\gset_install_$PID"
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
$ZipPath = "$TempDir\gset.zip"

# Download
Write-Step "Downloading GSET v$Version..."
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing
    Write-Success "Download complete"
} catch {
    Write-Err "Download failed: $_"
    Write-Info "Try downloading manually from: $DownloadUrl"
    Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    exit 1
}

# Create install directory
Write-Step "Installing to $InstallPath..."
if (-not (Test-Path $InstallPath)) {
    New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
}

# Extract
Write-Step "Extracting files..."
try {
    Expand-Archive -Path $ZipPath -DestinationPath $InstallPath -Force
    Write-Success "Extraction complete"
} catch {
    Write-Err "Extraction failed: $_"
    Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    exit 1
}

# Find the executable
$GsetExe = Get-ChildItem -Path $InstallPath -Filter "*.exe" -Recurse | Select-Object -First 1
if ($GsetExe) {
    $GsetPath = $GsetExe.FullName
    Write-Success "GSET installed: $GsetPath"
} else {
    Write-Err "Could not find gset.exe in installation directory"
    Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    exit 1
}

# Add to PATH
if ($AddToPath) {
    Write-Step "Adding to PATH..."
    
    # User-level PATH
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    
    if ($UserPath -notlike "*$InstallPath*") {
        [Environment]::SetEnvironmentVariable(
            "Path",
            "$UserPath;$InstallPath",
            "User"
        )
        
        # Update current session PATH
        $env:Path = "$UserPath;$InstallPath;$env:Path"
        Write-Success "Added $InstallPath to user PATH"
    } else {
        Write-Info "Already in PATH, skipping"
    }
}

# Cleanup
Write-Step "Cleaning up..."
Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
Write-Success "Cleanup complete"

# Verify installation
Write-Step "Verifying installation..."
Write-Host ""

try {
    $GsetVersion = & $GsetPath version 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Installation verified!"
        Write-Host ""
        Write-Host "  $GsetVersion" -ForegroundColor Green
        Write-Host ""
    }
} catch {
    Write-Warn "Could not verify, but GSET is installed"
}

# Instructions
Write-Host ""
Write-Host "============================================" -ForegroundColor Cyan
Write-Host "  Installation Complete!" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "To use GSET, open a NEW PowerShell window and run:"
Write-Host ""
Write-Host "  gset version" -ForegroundColor Yellow
Write-Host "  gset help" -ForegroundColor Yellow
Write-Host ""
Write-Host "Installed files:"
Write-Host "  $GsetPath"
Write-Host ""
Write-Host "Uninstall: Remove the directory and remove from PATH" -ForegroundColor Gray
Write-Host ""
