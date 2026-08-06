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
$arguments = if ($Command.Count -gt 1) { @($Command[1..($Command.Count - 1)]) } else { @() }

"command: $executable $($arguments -join ' ')" | Tee-Object -FilePath $logPath
"startedAt: $([DateTimeOffset]::UtcNow.ToString('O'))" | Tee-Object -FilePath $logPath -Append

$exitCode = 1
$nativePreferenceAvailable = Test-Path Variable:PSNativeCommandUseErrorActionPreference
if ($nativePreferenceAvailable) {
    $previousNativePreference = $PSNativeCommandUseErrorActionPreference
    $PSNativeCommandUseErrorActionPreference = $false
}

try {
    $previousErrorPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $executable @arguments 2>&1 | Tee-Object -FilePath $logPath -Append
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $previousErrorPreference
}
catch {
    $_ | Out-String | Tee-Object -FilePath $logPath -Append
    $exitCode = 1
}
finally {
    if ($nativePreferenceAvailable) {
        $PSNativeCommandUseErrorActionPreference = $previousNativePreference
    }
}

"finishedAt: $([DateTimeOffset]::UtcNow.ToString('O'))" | Tee-Object -FilePath $logPath -Append
"exitCode: $exitCode" | Tee-Object -FilePath $logPath -Append
exit $exitCode
