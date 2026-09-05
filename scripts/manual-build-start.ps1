[CmdletBinding()]
param(
    [switch]$CheckOnly,
    [switch]$SkipDependencySync,
    [switch]$UseDefaultWebView
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
$manualConfig = Join-Path $projectRoot "src-tauri\tauri.manual.conf.json"
$lockFile = Join-Path $projectRoot "package-lock.json"
$dependencyMarker = Join-Path $projectRoot "node_modules\.photo-organizer-package-lock.sha256"

function Write-Step([string]$Message) {
    Write-Host ""
    Write-Host "[PhotoOrganizer] $Message" -ForegroundColor Cyan
}

function Require-Command([string]$Name, [string]$InstallHint) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name was not found. $InstallHint"
    }
}

function Invoke-Checked([string]$Label, [scriptblock]$Action) {
    Write-Step $Label
    & $Action
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE."
    }
}

Push-Location $projectRoot
try {
    $cargoHome = Join-Path $env:USERPROFILE ".cargo\bin"
    if ((Test-Path (Join-Path $cargoHome "cargo.exe")) -and
        -not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) {
        $env:PATH = "$cargoHome;$env:PATH"
    }

    Require-Command "node.exe" "Install Node.js 22 or newer."
    Require-Command "npm.cmd" "Install npm together with Node.js."
    Require-Command "cargo.exe" "Install the Rust MSVC toolchain."

    if (-not (Test-Path $manualConfig)) {
        throw "Missing manual Tauri config: $manualConfig"
    }
    if (-not (Test-Path $lockFile)) {
        throw "Missing package-lock.json."
    }

    $nodeVersionText = (& node.exe --version).Trim()
    Write-Host "Node:  $nodeVersionText"
    Write-Host "npm:   $(& npm.cmd --version)"
    $package = Get-Content -LiteralPath (Join-Path $projectRoot "package.json") -Raw | ConvertFrom-Json
    if ($package.engines.node -match '(\d+\.\d+\.\d+)') {
        $minimumNode = [version]$Matches[1]
        $currentNode = [version]$nodeVersionText.TrimStart("v")
        if ($currentNode -lt $minimumNode) {
            Write-Warning "package.json recommends Node $minimumNode or newer. The build will still be attempted."
        }
    }
    Write-Host "Rust:  $(& cargo.exe --version)"

    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    $lockStream = [System.IO.File]::OpenRead($lockFile)
    try {
        $lockSignature = [BitConverter]::ToString($sha256.ComputeHash($lockStream)).Replace("-", "")
    }
    finally {
        $lockStream.Dispose()
        $sha256.Dispose()
    }
    $installedHash = if (Test-Path $dependencyMarker) {
        (Get-Content -LiteralPath $dependencyMarker -Raw).Trim()
    } else {
        ""
    }
    $dependenciesReady =
        (Test-Path (Join-Path $projectRoot "node_modules\.bin\vite.cmd")) -and
        (Test-Path (Join-Path $projectRoot "node_modules\.bin\tauri.cmd")) -and
        ($installedHash -eq $lockSignature)

    if (-not $dependenciesReady) {
        if ($SkipDependencySync) {
            throw "Dependencies are missing or out of date. Run this script without -SkipDependencySync once."
        }
        Invoke-Checked "Synchronizing frontend dependencies" {
            npm.cmd install --no-audit --no-fund
        }
        Set-Content -LiteralPath $dependencyMarker -Value $lockSignature -Encoding ascii
    } else {
        Write-Host "Dependencies: ready"
    }

    if ($CheckOnly) {
        Write-Step "Manual launch environment is ready"
        exit 0
    }

    Invoke-Checked "Building the frontend" {
        npm.cmd run build
    }

    if ([string]::IsNullOrWhiteSpace($env:PHOTO_ORGANIZER_DATA_DIR)) {
        $env:PHOTO_ORGANIZER_DATA_DIR = Join-Path $env:TEMP "PhotoOrganizer-dev-data"
    }
    if ([string]::IsNullOrWhiteSpace($env:PHOTO_ORGANIZER_WEBVIEW_DATA_DIR)) {
        $profileName = "PhotoOrganizer-webview2-" + [guid]::NewGuid().ToString("N")
        $env:PHOTO_ORGANIZER_WEBVIEW_DATA_DIR = Join-Path $env:TEMP $profileName
    }
    if (-not $UseDefaultWebView -and
        [string]::IsNullOrWhiteSpace($env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS)) {
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS =
            "--disable-gpu --disable-gpu-compositing --in-process-gpu --no-sandbox"
    }

    Write-Host "Data:     $env:PHOTO_ORGANIZER_DATA_DIR"
    Write-Host "WebView2: $env:PHOTO_ORGANIZER_WEBVIEW_DATA_DIR"
    Write-Step "Starting the Tauri desktop app"
    npm.cmd run tauri -- dev --config src-tauri\tauri.manual.conf.json --no-dev-server
    if ($LASTEXITCODE -ne 0) {
        throw "Desktop startup failed with exit code $LASTEXITCODE."
    }
}
catch {
    Write-Host ""
    Write-Host "[PhotoOrganizer] $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}
finally {
    Pop-Location
}
