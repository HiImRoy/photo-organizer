[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[a-z0-9-]+$')]
    [string]$Name,

    [Parameter(Mandatory = $true, Position = 1, ValueFromRemainingArguments = $true)]
    [string[]]$Command
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($Command.Count -eq 0) {
    throw 'A command is required.'
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$logRoot = if ($env:CI_LOG_DIR) { $env:CI_LOG_DIR } else { Join-Path $projectRoot 'artifacts\logs' }
New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
$logPath = Join-Path $logRoot "$Name.log"
$executable = $Command[0]
[string[]]$arguments = @()
if ($Command.Count -gt 1) {
    $arguments = [string[]]@($Command[1..($Command.Count - 1)])
}

"command: $executable $($arguments -join ' ')" | Tee-Object -FilePath $logPath
"startedAt: $([DateTimeOffset]::UtcNow.ToString('O'))" | Tee-Object -FilePath $logPath -Append

$exitCode = 1
$nativePreferenceAvailable = Test-Path Variable:PSNativeCommandUseErrorActionPreference
$previousErrorPreference = $ErrorActionPreference
if ($nativePreferenceAvailable) {
    $previousNativePreference = $PSNativeCommandUseErrorActionPreference
    $PSNativeCommandUseErrorActionPreference = $false
}

try {
    $ErrorActionPreference = 'Continue'
    & $executable @arguments 2>&1 | Tee-Object -FilePath $logPath -Append
    $exitCode = $LASTEXITCODE
}
catch {
    $_ | Out-String | Tee-Object -FilePath $logPath -Append
    $exitCode = 1
}
finally {
    $ErrorActionPreference = $previousErrorPreference
    if ($nativePreferenceAvailable) {
        $PSNativeCommandUseErrorActionPreference = $previousNativePreference
    }
}

"finishedAt: $([DateTimeOffset]::UtcNow.ToString('O'))" | Tee-Object -FilePath $logPath -Append
"exitCode: $exitCode" | Tee-Object -FilePath $logPath -Append
exit $exitCode
