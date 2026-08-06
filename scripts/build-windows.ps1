[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $env:CARGO_BUILD_TARGET = 'x86_64-pc-windows-msvc'
    & (Join-Path $PSScriptRoot 'diagnose-msvc.ps1')
    & (Join-Path $PSScriptRoot 'verify-release-resources.ps1')
    npm.cmd ci
    npm.cmd run validate
    npm.cmd run tauri build -- --target x86_64-pc-windows-msvc
}
finally {
    Pop-Location
}
