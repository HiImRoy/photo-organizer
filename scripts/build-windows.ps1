[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    & (Join-Path $PSScriptRoot 'diagnose-msvc.ps1')
    & (Join-Path $PSScriptRoot 'verify-release-resources.ps1')
    npm.cmd ci
    npm.cmd run validate
    npm.cmd run tauri build
}
finally {
    Pop-Location
}
