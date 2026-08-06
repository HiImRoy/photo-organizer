[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
    throw 'Visual Studio Installer/vswhere is missing. Install Visual Studio 2022 Build Tools with Desktop development with C++.'
}

[string]$installationPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$installationPath = $installationPath.Trim()
if (-not $installationPath) {
    throw 'MSVC x64/x86 build tools are missing (component Microsoft.VisualStudio.Component.VC.Tools.x86.x64).'
}

[string]$workloadPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Workload.VCTools -property installationPath
$workloadPath = $workloadPath.Trim()
$link = Get-Command link.exe -ErrorAction SilentlyContinue
$compiler = Get-Command cl.exe -ErrorAction SilentlyContinue
$sdkRoot = $env:WindowsSdkDir
$sdkVersion = $env:WindowsSDKVersion

if (-not $link) {
    throw 'link.exe is not available in PATH. Run from Developer PowerShell or initialize VsDevCmd.bat for x64.'
}
if (-not $compiler) {
    throw 'cl.exe is not available in PATH. Run from Developer PowerShell or initialize VsDevCmd.bat for x64.'
}
if (-not $sdkRoot -or -not (Test-Path -LiteralPath $sdkRoot -PathType Container)) {
    throw 'Windows SDK is not initialized. Install a Windows 10/11 SDK and load the Visual C++ developer environment.'
}

[pscustomobject]@{
    VisualStudioInstallation = $installationPath
    DesktopCppWorkload       = [bool]$workloadPath
    Linker                    = $link.Source
    Compiler                  = $compiler.Source
    WindowsSdkRoot            = $sdkRoot
    WindowsSdkVersion         = $sdkVersion
    RustupToolchain           = (& rustup show active-toolchain)
    Rustc                     = (& rustc -V)
    Cargo                     = (& cargo -V)
    BuildTarget               = if ($env:CARGO_BUILD_TARGET) { $env:CARGO_BUILD_TARGET } else { 'host default' }
} | Format-List
