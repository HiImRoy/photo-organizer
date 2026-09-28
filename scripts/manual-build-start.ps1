[CmdletBinding()]
param(
    [switch]$CheckOnly,
    [switch]$SkipDependencySync,
    [switch]$UseDefaultWebView
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$manualConfig = Join-Path $projectRoot "src-tauri\tauri.manual.conf.json"
$packageFile = Join-Path $projectRoot "package.json"
$lockFile = Join-Path $projectRoot "package-lock.json"
$cargoManifest = Join-Path $projectRoot "src-tauri\Cargo.toml"
$nodeModules = Join-Path $projectRoot "node_modules"
$dependencyMarker = Join-Path $nodeModules ".photo-organizer-package-lock.sha256"
$script:Issues = New-Object 'System.Collections.Generic.List[string]'
$script:NativeExitCode = 0
$exitCode = 0
$locationPushed = $false

function Write-Step([string]$Message) {
    Write-Host ""
    Write-Host "[PhotoOrganizer] $Message" -ForegroundColor Cyan
}

function Write-Check([string]$Name, [string]$State, [ConsoleColor]$Color = [ConsoleColor]::Green) {
    Write-Host ("  {0}: {1}" -f $Name, $State) -ForegroundColor $Color
}

function Add-Issue([string]$Message) {
    [void]$script:Issues.Add($Message)
}

function Get-CommandPath([string]$Name) {
    $command = Get-Command -Name $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command -and $command.CommandType -eq "Application") {
        return $command.Source
    }
    return $null
}

function Get-VersionFromText([string]$Text) {
    $match = [System.Text.RegularExpressions.Regex]::Match($Text, '\d+\.\d+\.\d+')
    if (-not $match.Success) {
        return $null
    }
    return [version]$match.Value
}

function Get-FileSha256([string]$Path) {
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        return [BitConverter]::ToString($sha256.ComputeHash($stream)).Replace("-", "")
    }
    finally {
        $stream.Dispose()
        $sha256.Dispose()
    }
}

function Invoke-NativeForOutput([string]$Label, [string]$Executable, [string[]]$Arguments) {
    $output = @(& $Executable @Arguments 2>&1)
    $commandExitCode = $LASTEXITCODE
    $text = (($output | ForEach-Object { "$_" }) -join [Environment]::NewLine).Trim()
    if ($commandExitCode -ne 0) {
        Write-Check $Label ("failed with exit code {0}" -f $commandExitCode) Yellow
        if ($text) {
            Write-Host $text
        }
        Add-Issue ("$Label returned exit code $commandExitCode.")
        if ($script:NativeExitCode -eq 0) {
            $script:NativeExitCode = $commandExitCode
        }
        return $null
    }
    Write-Check $Label $text
    return $text
}

function Invoke-NativeChecked([string]$Label, [string]$Executable, [string[]]$Arguments) {
    Write-Step $Label
    & $Executable @Arguments
    $commandExitCode = $LASTEXITCODE
    if ($commandExitCode -ne 0) {
        $script:NativeExitCode = $commandExitCode
        throw "$Label failed with exit code $commandExitCode."
    }
}

function Get-FrontendDependencyState([string]$LockSignature) {
    if (-not (Test-Path -LiteralPath $nodeModules -PathType Container)) {
        return "missing node_modules"
    }

    $missingBins = @()
    foreach ($bin in @("vite.cmd", "tsc.cmd", "tauri.cmd")) {
        if (-not (Test-Path -LiteralPath (Join-Path $nodeModules ".bin\$bin") -PathType Leaf)) {
            $missingBins += $bin
        }
    }
    if ($missingBins.Count -gt 0) {
        return ("incomplete; missing {0}" -f ($missingBins -join ", "))
    }

    if (-not (Test-Path -LiteralPath $dependencyMarker -PathType Leaf)) {
        return "present but unverified; lockfile marker is missing"
    }

    $installedSignature = (Get-Content -LiteralPath $dependencyMarker -Raw).Trim()
    if ($installedSignature -ne $LockSignature) {
        return "out of date; lockfile marker does not match package-lock.json"
    }
    return "ready; Vite, TypeScript, and Tauri CLI are present and the lockfile marker matches"
}

function Test-MsvcBuildTools {
    $clPath = Get-CommandPath "cl.exe"
    if ($clPath) {
        return "available through cl.exe at $clPath"
    }

    $programFilesX86 = [Environment]::GetEnvironmentVariable("ProgramFiles(x86)")
    $programFiles = [Environment]::GetEnvironmentVariable("ProgramFiles")
    $vswhereCandidates = @()
    if ($programFilesX86) {
        $vswhereCandidates += Join-Path $programFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"
    }
    if ($programFiles) {
        $vswhereCandidates += Join-Path $programFiles "Microsoft Visual Studio\Installer\vswhere.exe"
    }

    foreach ($vswhere in $vswhereCandidates) {
        if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
            continue
        }
        $installation = @(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null)
        $vswhereExitCode = $LASTEXITCODE
        $installationPath = (($installation | ForEach-Object { "$_" }) -join "").Trim()
        if ($vswhereExitCode -eq 0 -and $installationPath -and (Test-Path -LiteralPath $installationPath -PathType Container)) {
            return "available in Visual Studio at $installationPath"
        }
    }
    return $null
}

function Test-WebView2Runtime {
    $registryRoots = @(
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients",
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients",
        "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients"
    )
    $webView2ProductId = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    foreach ($root in $registryRoots) {
        if (-not (Test-Path -LiteralPath $root)) {
            continue
        }
        foreach ($client in (Get-ChildItem -LiteralPath $root -ErrorAction SilentlyContinue)) {
            $clientInfo = Get-ItemProperty -LiteralPath $client.PSPath -ErrorAction SilentlyContinue
            if (($clientInfo.name -match "WebView2") -or ($client.PSChildName -eq $webView2ProductId)) {
                if ($clientInfo.pv -and $clientInfo.pv -ne "0.0.0.0") {
                    return "available (version $($clientInfo.pv))"
                }
            }
        }
    }

    $runtimeRoots = @(
        (Join-Path ([Environment]::GetEnvironmentVariable("ProgramFiles(x86)")) "Microsoft\EdgeWebView\Application"),
        (Join-Path ([Environment]::GetEnvironmentVariable("ProgramFiles")) "Microsoft\EdgeWebView\Application"),
        (Join-Path ([Environment]::GetEnvironmentVariable("LOCALAPPDATA")) "Microsoft\EdgeWebView\Application")
    )
    foreach ($root in $runtimeRoots) {
        if (-not $root -or -not (Test-Path -LiteralPath $root -PathType Container)) {
            continue
        }
        foreach ($versionDirectory in (Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue)) {
            if (Test-Path -LiteralPath (Join-Path $versionDirectory.FullName "msedgewebview2.exe") -PathType Leaf) {
                return "available at $($versionDirectory.FullName)"
            }
        }
    }
    return $null
}

function Resolve-ConfiguredDirectory([string]$Name, [string]$Value) {
    try {
        $resolved = [System.IO.Path]::GetFullPath($Value)
    }
    catch {
        Add-Issue ("$Name is not a valid filesystem path: $Value")
        Write-Check $Name "invalid path: $Value" Red
        return $null
    }

    if (Test-Path -LiteralPath $resolved -PathType Leaf) {
        Add-Issue ("$Name points to a file instead of a directory: $resolved")
        Write-Check $Name "a file exists at the configured path: $resolved" Red
        return $null
    }

    $pathRoot = [System.IO.Path]::GetPathRoot($resolved)
    if (-not $pathRoot -or -not (Test-Path -LiteralPath $pathRoot -PathType Container)) {
        Add-Issue ("$Name has no accessible root directory: $resolved")
        Write-Check $Name "root directory is unavailable: $pathRoot" Red
        return $null
    }

    if (Test-Path -LiteralPath $resolved -PathType Container) {
        Write-Check $Name "directory exists: $resolved"
    }
    else {
        Write-Check $Name "valid path; the app can create it on launch: $resolved" DarkYellow
    }
    return $resolved
}

Push-Location -LiteralPath $projectRoot
$locationPushed = $true
try {
    $userProfile = [Environment]::GetEnvironmentVariable("USERPROFILE")
    if ($userProfile) {
        $cargoHome = Join-Path $userProfile ".cargo\bin"
        if ((Test-Path -LiteralPath (Join-Path $cargoHome "cargo.exe") -PathType Leaf) -and
            -not (Get-Command -Name "cargo.exe" -ErrorAction SilentlyContinue)) {
            $env:PATH = "$cargoHome;$env:PATH"
        }
    }

    Write-Step "Checking the manual Tauri launch environment"

    foreach ($requiredFile in @(
        @{ Name = "Manual Tauri config"; Path = $manualConfig },
        @{ Name = "package.json"; Path = $packageFile },
        @{ Name = "package-lock.json"; Path = $lockFile },
        @{ Name = "Rust manifest"; Path = $cargoManifest }
    )) {
        if (Test-Path -LiteralPath $requiredFile.Path -PathType Leaf) {
            Write-Check $requiredFile.Name "found"
        }
        else {
            Write-Check $requiredFile.Name "missing: $($requiredFile.Path)" Red
            Add-Issue ("Required file is missing: $($requiredFile.Path)")
        }
    }

    $package = $null
    if (Test-Path -LiteralPath $packageFile -PathType Leaf) {
        try {
            $package = Get-Content -LiteralPath $packageFile -Raw | ConvertFrom-Json
        }
        catch {
            Write-Check "package.json" "could not be parsed: $($_.Exception.Message)" Red
            Add-Issue "package.json is not valid JSON."
        }
    }

    $lockSignature = $null
    if (Test-Path -LiteralPath $lockFile -PathType Leaf) {
        $lockSignature = Get-FileSha256 $lockFile
        Write-Check "Dependency lockfile" "SHA-256 $lockSignature"
    }

    $nodePath = Get-CommandPath "node.exe"
    $npmPath = Get-CommandPath "npm.cmd"
    $cargoPath = Get-CommandPath "cargo.exe"
    $rustcPath = Get-CommandPath "rustc.exe"

    foreach ($tool in @(
        @{ Name = "Node.js"; Path = $nodePath },
        @{ Name = "npm"; Path = $npmPath },
        @{ Name = "Cargo"; Path = $cargoPath },
        @{ Name = "Rust compiler"; Path = $rustcPath }
    )) {
        if ($tool.Path) {
            Write-Check $tool.Name "found at $($tool.Path)"
        }
        else {
            Write-Check $tool.Name "missing" Red
            Add-Issue ("$($tool.Name) is required for the manual Tauri launch.")
        }
    }

    $nodeVersion = $null
    $npmVersion = $null
    if ($nodePath) {
        $nodeVersionText = Invoke-NativeForOutput "Node.js version" $nodePath @("--version")
        if ($nodeVersionText) {
            $nodeVersion = Get-VersionFromText $nodeVersionText
            if (-not $nodeVersion) {
                Write-Check "Node.js version" "could not parse version: $nodeVersionText" Red
                Add-Issue "Could not determine the Node.js version."
            }
        }
    }
    if ($npmPath) {
        $npmVersionText = Invoke-NativeForOutput "npm version" $npmPath @("--version")
        if ($npmVersionText) {
            $npmVersion = Get-VersionFromText $npmVersionText
            if (-not $npmVersion) {
                Write-Check "npm version" "could not parse version: $npmVersionText" Red
                Add-Issue "Could not determine the npm version."
            }
        }
    }

    if ($package -and $package.engines) {
        $nodeRequirement = [System.Text.RegularExpressions.Regex]::Match([string]$package.engines.node, '\d+\.\d+\.\d+')
        if ($nodeRequirement.Success -and $nodeVersion -and $nodeVersion -lt [version]$nodeRequirement.Value) {
            Write-Check "Project Node.js engine requirement" ("package.json declares >= {0}; continuing with installed {1}" -f $nodeRequirement.Value, $nodeVersion) DarkYellow
        }

        $npmRequirement = [System.Text.RegularExpressions.Regex]::Match([string]$package.engines.npm, '\d+')
        if ($npmRequirement.Success -and $npmVersion -and $npmVersion.Major -lt [int]$npmRequirement.Value) {
            Write-Check "npm engine requirement" ("package.json declares >= {0}; found {1}" -f $npmRequirement.Value, $npmVersion) Red
            Add-Issue ("npm $npmVersion is below the package requirement $($npmRequirement.Value).")
        }
    }

    $cargoVersionText = $null
    if ($cargoPath) {
        $cargoVersionText = Invoke-NativeForOutput "Cargo version" $cargoPath @("--version")
    }
    $rustcDetails = $null
    if ($rustcPath) {
        $rustcDetails = Invoke-NativeForOutput "Rust compiler details" $rustcPath @("-vV")
    }
    if ($rustcDetails) {
        $rustcVersion = Get-VersionFromText $rustcDetails
        if (Test-Path -LiteralPath $cargoManifest -PathType Leaf) {
            $manifestText = Get-Content -LiteralPath $cargoManifest -Raw
            $minimumRust = [System.Text.RegularExpressions.Regex]::Match($manifestText, '(?m)^rust-version\s*=\s*"(?<version>\d+\.\d+(?:\.\d+)?)"')
            if ($minimumRust.Success -and $rustcVersion -and $rustcVersion -lt [version]$minimumRust.Groups["version"].Value) {
                Write-Check "Rust toolchain requirement" ("Cargo.toml requires rustc >= {0}; found {1}" -f $minimumRust.Groups["version"].Value, $rustcVersion) Red
                Add-Issue ("rustc $rustcVersion is below the project requirement $($minimumRust.Groups['version'].Value).")
            }
        }

        $hostMatch = [System.Text.RegularExpressions.Regex]::Match($rustcDetails, '(?m)^host:\s*(\S+)')
        if ($hostMatch.Success -and $hostMatch.Groups[1].Value -notmatch 'windows-msvc$') {
            Write-Check "Rust target" ("expected a Windows MSVC toolchain; found {0}" -f $hostMatch.Groups[1].Value) Red
            Add-Issue "The manual Tauri launcher requires the Windows MSVC Rust toolchain."
        }
    }

    $msvcStatus = Test-MsvcBuildTools
    if ($msvcStatus) {
        Write-Check "MSVC C++ Build Tools" $msvcStatus
    }
    else {
        Write-Check "MSVC C++ Build Tools" "missing; install the Visual C++ x86/x64 build tools" Red
        Add-Issue "Microsoft C++ Build Tools are required for the Windows Tauri build."
    }

    $webViewStatus = Test-WebView2Runtime
    if ($webViewStatus) {
        Write-Check "WebView2 Runtime" $webViewStatus
    }
    else {
        Write-Check "WebView2 Runtime" "missing; install the Microsoft Edge WebView2 Runtime" Red
        Add-Issue "The Microsoft Edge WebView2 Runtime is required to launch the desktop app."
    }

    $dataDirectoryValue = $env:PHOTO_ORGANIZER_DATA_DIR
    if ([string]::IsNullOrWhiteSpace($dataDirectoryValue)) {
        $dataDirectoryValue = Join-Path ([System.IO.Path]::GetTempPath()) "PhotoOrganizer-dev-data"
    }
    $webViewDirectoryValue = $env:PHOTO_ORGANIZER_WEBVIEW_DATA_DIR
    if ([string]::IsNullOrWhiteSpace($webViewDirectoryValue)) {
        $webViewDirectoryValue = Join-Path ([System.IO.Path]::GetTempPath()) ("PhotoOrganizer-webview2-{0}" -f [guid]::NewGuid().ToString("N"))
    }
    $dataDirectory = Resolve-ConfiguredDirectory "App data directory" $dataDirectoryValue
    $webViewDirectory = Resolve-ConfiguredDirectory "WebView2 profile directory" $webViewDirectoryValue

    $frontendDependencyState = "not checked"
    if ($lockSignature) {
        $frontendDependencyState = Get-FrontendDependencyState $lockSignature
    }
    Write-Check "Frontend dependencies" $frontendDependencyState $(if ($frontendDependencyState -like "ready;*") { [ConsoleColor]::Green } else { [ConsoleColor]::DarkYellow })

    if ($CheckOnly) {
        if ($frontendDependencyState -notlike "ready;*") {
            Add-Issue "Frontend dependencies are $frontendDependencyState. Run the launcher normally to synchronize them."
        }
        if ($script:Issues.Count -gt 0) {
            throw ("Environment check found {0} issue(s): {1}" -f $script:Issues.Count, ($script:Issues -join " "))
        }
        Write-Step "Environment check passed; no dependency sync, marker write, build, or app launch was performed"
    }
    else {
        if ($script:Issues.Count -gt 0) {
            throw ("Required launch prerequisites are missing or invalid: {0}" -f ($script:Issues -join " "))
        }

        if ($frontendDependencyState -notlike "ready;*") {
            if ($SkipDependencySync) {
                throw "Frontend dependencies are $frontendDependencyState. Run without -SkipDependencySync to synchronize them."
            }
            Invoke-NativeChecked "Synchronizing frontend dependencies" $npmPath @("install", "--engine-strict=false", "--no-audit", "--no-fund")
            $lockSignature = Get-FileSha256 $lockFile
            if (-not (Test-Path -LiteralPath $nodeModules -PathType Container)) {
                throw "npm completed, but node_modules was not created."
            }
            Set-Content -LiteralPath $dependencyMarker -Value $lockSignature -Encoding ascii
            $frontendDependencyState = Get-FrontendDependencyState $lockSignature
            Write-Check "Frontend dependencies after sync" $frontendDependencyState
            if ($frontendDependencyState -notlike "ready;*") {
                throw "npm completed, but frontend dependencies are still $frontendDependencyState."
            }
        }

        Invoke-NativeChecked "Building the frontend" $npmPath @("run", "build")

        $env:PHOTO_ORGANIZER_DATA_DIR = $dataDirectory
        $env:PHOTO_ORGANIZER_WEBVIEW_DATA_DIR = $webViewDirectory
        if (-not $UseDefaultWebView -and
            [string]::IsNullOrWhiteSpace($env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS)) {
            $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--disable-gpu --disable-gpu-compositing --in-process-gpu"
        }

        Write-Host "Data:     $env:PHOTO_ORGANIZER_DATA_DIR"
        Write-Host "WebView2: $env:PHOTO_ORGANIZER_WEBVIEW_DATA_DIR"
        Write-Step "Starting Tauri desktop with the manual config (beforeDevCommand is disabled; no dev server)"
        Invoke-NativeChecked "Tauri desktop startup" $npmPath @("run", "tauri", "--", "dev", "--config", $manualConfig, "--no-dev-server")
    }
}
catch {
    Write-Host ""
    Write-Host "[PhotoOrganizer] $($_.Exception.Message)" -ForegroundColor Red
    if ($script:NativeExitCode -ne 0) {
        $exitCode = $script:NativeExitCode
    }
    else {
        $exitCode = 1
    }
}
finally {
    if ($locationPushed) {
        Pop-Location
    }
}

exit $exitCode
