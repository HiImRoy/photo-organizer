[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)]
        [bool]$Condition,

        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Get-WorkflowStepBlocks {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string[]]$Lines)

    $blocks = New-Object 'System.Collections.Generic.List[string]'
    $current = New-Object 'System.Collections.Generic.List[string]'

    foreach ($line in $Lines) {
        if ($line -match '^      - ') {
            if ($current.Count -gt 0) {
                $blocks.Add(($current -join "`n"))
                $current.Clear()
            }
            $current.Add($line)
            continue
        }

        if ($current.Count -gt 0 -and $line -match '^  [A-Za-z0-9_-]+:\s*$') {
            $blocks.Add(($current -join "`n"))
            $current.Clear()
            continue
        }

        if ($current.Count -gt 0) {
            $current.Add($line)
        }
    }

    if ($current.Count -gt 0) {
        $blocks.Add(($current -join "`n"))
    }

    return $blocks.ToArray()
}

function Get-ArgvFromOutput {
    param([Parameter(Mandatory = $true)][string]$Output)

    $match = [regex]::Match($Output, '(?m)^ARGV64:(?<value>[A-Za-z0-9+/=]*)\s*$')
    if (-not $match.Success) {
        return $null
    }

    $bytes = [Convert]::FromBase64String($match.Groups['value'].Value)
    $json = [Text.Encoding]::UTF8.GetString($bytes)
    return ,@((ConvertFrom-Json -InputObject $json))
}

function Invoke-WrapperChild {
    param(
        [Parameter(Mandatory = $true)]
        [psobject]$Engine,

        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter(Mandatory = $true)]
        [string[]]$Command
    )

    $env:CI_TEST_NAME = $Name
    $env:CI_TEST_COMMAND_JSON = ConvertTo-Json -InputObject $Command -Compress
    $stdoutPath = Join-Path $testRoot "$Name.stdout.txt"
    $stderrPath = Join-Path $testRoot "$Name.stderr.txt"
    $argumentLine = '-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + $launcherPath + '"'

    $process = Start-Process -FilePath $Engine.Path -ArgumentList $argumentLine -Wait -PassThru -NoNewWindow `
        -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath

    $stdout = if (Test-Path -LiteralPath $stdoutPath) { Get-Content -LiteralPath $stdoutPath -Raw } else { '' }
    $stderr = if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath -Raw } else { '' }
    return [pscustomobject]@{
        ExitCode = $process.ExitCode
        Output   = $stdout + "`n" + $stderr
        Stdout   = $stdout
        Stderr   = $stderr
        LogPath  = Join-Path $logRoot "$Name.log"
    }
}

function Assert-ArgvEquals {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$Actual,

        [Parameter(Mandatory = $true)]
        [string[]]$Expected,

        [Parameter(Mandatory = $true)]
        [string]$Context
    )

    Assert-Condition -Condition ($null -ne $Actual) -Message "$Context did not emit the argv probe marker."
    Assert-Condition -Condition ($Actual.Count -eq $Expected.Count) -Message "$Context received $($Actual.Count) argument(s), expected $($Expected.Count): $($Actual -join ' | ')"
    for ($index = 0; $index -lt $Expected.Count; $index++) {
        Assert-Condition -Condition ([string]$Actual[$index] -ceq $Expected[$index]) `
            -Message "$Context argument $index differed. Expected <$($Expected[$index])>, got <$($Actual[$index])>."
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$wrapperPath = Join-Path $PSScriptRoot 'invoke-ci-command.ps1'
$workflowPath = Join-Path $repoRoot '.github/workflows/windows-build.yml'
$workflowText = Get-Content -LiteralPath $workflowPath -Raw
$workflowLines = Get-Content -LiteralPath $workflowPath
$workflowSteps = @(Get-WorkflowStepBlocks -Lines $workflowLines)

$checkoutSteps = @($workflowSteps | Where-Object { $_ -match '(?m)^\s{6}- uses: actions/checkout@v4\s*$' })
Assert-Condition -Condition ($checkoutSteps.Count -eq 2) -Message "Expected two checkout steps; found $($checkoutSteps.Count)."
foreach ($checkoutStep in $checkoutSteps) {
    Assert-Condition -Condition ($checkoutStep -match '(?m)^\s{8}with:\s*\r?\n\s{10}lfs:\s*true\s*$') `
        -Message 'Every Windows job checkout must enable Git LFS before release resource hash validation.'
}

Assert-Condition -Condition ($workflowText -match '(?m)^\s{6}upload_diagnostic_logs:\s*$') -Message 'The manual workflow is missing the diagnostic log input.'
Assert-Condition -Condition ($workflowText -match '(?m)^\s{8}type:\s+boolean\s*$') -Message 'The diagnostic log input must be boolean.'
Assert-Condition -Condition ($workflowText -match '(?m)^\s{8}default:\s+false\s*$') -Message 'Diagnostic log upload must default to false.'

$expectedLogCondition = "(?m)^\s{8}if:\s*always\(\)\s*&&\s*github\.event_name == 'workflow_dispatch'\s*&&\s*inputs\.upload_diagnostic_logs\s*$"
$validationLogStep = @($workflowSteps | Where-Object { $_ -match '(?m)^\s{6}- name: Upload validation logs\s*$' })
$bundleLogStep = @($workflowSteps | Where-Object { $_ -match '(?m)^\s{6}- name: Upload bundle logs\s*$' })
Assert-Condition -Condition ($validationLogStep.Count -eq 1 -and $validationLogStep[0] -match $expectedLogCondition -and $validationLogStep[0] -match '(?m)^\s{10}retention-days:\s+3\s*$') `
    -Message 'Validation logs must upload only after an opted-in manual run and remain for 3 days.'
Assert-Condition -Condition ($bundleLogStep.Count -eq 1 -and $bundleLogStep[0] -match $expectedLogCondition -and $bundleLogStep[0] -match '(?m)^\s{10}retention-days:\s+3\s*$') `
    -Message 'Bundle logs must upload only after an opted-in manual run and remain for 3 days.'

$installerStep = @($workflowSteps | Where-Object { $_ -match '(?m)^\s{6}- name: Upload Windows installers and hashes\s*$' })
Assert-Condition -Condition ($installerStep.Count -eq 1 -and $installerStep[0] -match '(?m)^\s{10}retention-days:\s+7\s*$') `
    -Message 'Installer artifacts must retain for 7 days.'
Assert-Condition -Condition ($workflowText -match "(?m)^\s{4}if:\s+github\.event_name == 'workflow_dispatch' \|\| startsWith\(github\.ref, 'refs/tags/v'\)\s*$") `
    -Message 'The installer job must remain gated to manual dispatch or version tags.'

$wrapperCallCount = [regex]::Matches($workflowText, 'scripts/invoke-ci-command\.ps1').Count
$explicitCallCount = [regex]::Matches($workflowText, '(?m)^\s{8}run:\s+\./scripts/invoke-ci-command\.ps1[^\r\n]*-Command @\([^\r\n]*\); exit \$LASTEXITCODE\s*$').Count
Assert-Condition -Condition ($wrapperCallCount -gt 0 -and $wrapperCallCount -eq $explicitCallCount) `
    -Message 'Every CI wrapper call must pass a literal -Command string array and exit with its returned native status.'
Assert-Condition -Condition ($workflowText -match '(?m)^\s{8}run:\s+pwsh .*scripts/test-invoke-ci-command\.ps1\s*$') `
    -Message 'The wrapper regression test must run directly in validate, without using the wrapper.'

$requiredSteps = @(
    'Install JavaScript dependencies',
    'Prettier',
    'ESLint',
    'TypeScript',
    'Vitest',
    'Rustfmt',
    'Rust tests',
    'Clippy',
    'Frontend production build',
    'Build NSIS installer',
    'Build MSI installer when WiX succeeds'
)
foreach ($requiredStep in $requiredSteps) {
    Assert-Condition -Condition ($workflowText.Contains("name: $requiredStep")) -Message "Existing workflow check or bundle step was removed: $requiredStep"
}

$pwshCommand = Get-Command pwsh.exe -ErrorAction SilentlyContinue
Assert-Condition -Condition ($null -ne $pwshCommand) -Message 'PowerShell 7 (pwsh.exe) is required to run this regression suite.'
$engines = New-Object 'System.Collections.Generic.List[object]'
$engines.Add([pscustomobject]@{ Name = 'PowerShell 7'; Path = $pwshCommand.Source })
$windowsPowerShellPath = Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
if (Test-Path -LiteralPath $windowsPowerShellPath) {
    $engines.Add([pscustomobject]@{ Name = 'Windows PowerShell 5.1'; Path = $windowsPowerShellPath })
}

$nodeCommand = Get-Command node.exe -ErrorAction SilentlyContinue
$npmCommand = Get-Command npm.cmd -ErrorAction SilentlyContinue
Assert-Condition -Condition ($null -ne $nodeCommand -and $null -ne $npmCommand) -Message 'Node.js and npm must be available (the CI test runs after setup-node).'
$nodePath = $nodeCommand.Source
$npmPath = $npmCommand.Source
$directNpmVersion = (& $npmPath --version 2>&1 | Out-String).Trim()
$directNpmExitCode = $LASTEXITCODE
Assert-Condition -Condition ($directNpmExitCode -eq 0 -and $directNpmVersion -match '^\d+\.\d+\.\d+') `
    -Message "The read-only npm.cmd --version check failed: exit=$directNpmExitCode output=<$directNpmVersion>."

$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('ci command forwarding ' + [guid]::NewGuid().ToString('N'))
$logRoot = Join-Path $testRoot 'logs'
$fakeBin = Join-Path $testRoot 'fake npm'
$probePath = Join-Path $testRoot 'argv probe.js'
$launcherPath = Join-Path $testRoot 'invoke wrapper.ps1'
$savedLogDir = $env:CI_LOG_DIR
$savedWrapperPath = $env:CI_TEST_WRAPPER
$savedName = $env:CI_TEST_NAME
$savedCommandJson = $env:CI_TEST_COMMAND_JSON

try {
    New-Item -ItemType Directory -Path $testRoot, $logRoot, $fakeBin | Out-Null
    $probeSource = @'
const args = process.argv.slice(2);
if (args[0] === "__FAIL__") {
  process.stdout.write("stdout-marker\n");
  process.stderr.write("stderr-marker\n");
  process.exit(Number(args[1]));
}
process.stdout.write("ARGV64:" + Buffer.from(JSON.stringify(args), "utf8").toString("base64") + "\n");
'@
    [IO.File]::WriteAllText($probePath, $probeSource, (New-Object System.Text.UTF8Encoding($false)))

    $fakeNpmPath = Join-Path $fakeBin 'npm.cmd'
    $fakeNpmSource = '@echo off' + "`r`n" + '"' + $nodePath + '" "' + $probePath + '" %*' + "`r`n"
    [IO.File]::WriteAllText($fakeNpmPath, $fakeNpmSource, [Text.Encoding]::Default)

    $launcherSource = @'
$command = [string[]]@((ConvertFrom-Json -InputObject $env:CI_TEST_COMMAND_JSON))
& $env:CI_TEST_WRAPPER -Name $env:CI_TEST_NAME -Command $command
exit $LASTEXITCODE
'@
    [IO.File]::WriteAllText($launcherPath, $launcherSource, (New-Object System.Text.UTF8Encoding($true)))

    $env:CI_LOG_DIR = $logRoot
    $env:CI_TEST_WRAPPER = $wrapperPath

    foreach ($engine in $engines) {
        Write-Host "Testing $($engine.Name)"
        $hostSlug = ($engine.Name -replace '\W', '-').ToLowerInvariant()

        $noArgumentCase = Invoke-WrapperChild -Engine $engine -Name "no-args-$hostSlug" -Command @('whoami.exe')
        Assert-Condition -Condition ($noArgumentCase.ExitCode -eq 0) -Message "$($engine.Name) no-argument native command failed: $($noArgumentCase.Output)"
        Assert-Condition -Condition (Test-Path -LiteralPath $noArgumentCase.LogPath) -Message "$($engine.Name) did not create its command log."

        $singleArgumentCase = Invoke-WrapperChild -Engine $engine -Name "single-ci-$hostSlug" -Command @($fakeNpmPath, 'ci')
        Assert-Condition -Condition ($singleArgumentCase.ExitCode -eq 0) -Message "$($engine.Name) explicit npm.cmd ci probe failed: $($singleArgumentCase.Output)"
        Assert-ArgvEquals -Actual (Get-ArgvFromOutput -Output $singleArgumentCase.Output) -Expected @('ci') -Context "$($engine.Name) explicit npm.cmd ci probe"

        $multipleCase = Invoke-WrapperChild -Engine $engine -Name "multiple-$hostSlug" -Command @($nodePath, $probePath, 'alpha', 'beta', 'three')
        Assert-Condition -Condition ($multipleCase.ExitCode -eq 0) -Message "$($engine.Name) multiple-argument probe failed: $($multipleCase.Output)"
        Assert-ArgvEquals -Actual (Get-ArgvFromOutput -Output $multipleCase.Output) -Expected @('alpha', 'beta', 'three') -Context "$($engine.Name) multiple arguments"

        $unicodeCase = Invoke-WrapperChild -Engine $engine -Name "unicode-$hostSlug" -Command @($nodePath, $probePath, 'photo set with spaces', '中文目录', 'Фото', 'emoji 📷')
        Assert-Condition -Condition ($unicodeCase.ExitCode -eq 0) -Message "$($engine.Name) Unicode argument probe failed: $($unicodeCase.Output)"
        Assert-ArgvEquals -Actual (Get-ArgvFromOutput -Output $unicodeCase.Output) -Expected @('photo set with spaces', '中文目录', 'Фото', 'emoji 📷') -Context "$($engine.Name) spaces and Unicode"

        $switchCase = Invoke-WrapperChild -Engine $engine -Name "switches-$hostSlug" -Command @($nodePath, $probePath, '-p', '-e', '--all-targets', '--', '--check')
        Assert-Condition -Condition ($switchCase.ExitCode -eq 0) -Message "$($engine.Name) native switch probe failed: $($switchCase.Output)"
        Assert-ArgvEquals -Actual (Get-ArgvFromOutput -Output $switchCase.Output) -Expected @('-p', '-e', '--all-targets', '--', '--check') -Context "$($engine.Name) native switches and literal --"

        $failureCase = Invoke-WrapperChild -Engine $engine -Name "failure-$hostSlug" -Command @($nodePath, $probePath, '__FAIL__', '23')
        Assert-Condition -Condition ($failureCase.ExitCode -eq 23) -Message "$($engine.Name) did not propagate native exit 23 (actual $($failureCase.ExitCode)): $($failureCase.Output)"
        $failureLog = Get-Content -LiteralPath $failureCase.LogPath -Raw
        Assert-Condition -Condition ($failureLog.Contains('stdout-marker') -and $failureLog.Contains('stderr-marker')) `
            -Message "$($engine.Name) log did not preserve both native stdout and stderr."

        $realNpmCase = Invoke-WrapperChild -Engine $engine -Name "npm-version-$hostSlug" -Command @($npmPath, '--version')
        Assert-Condition -Condition ($realNpmCase.ExitCode -eq 0 -and $realNpmCase.Output -match [regex]::Escape($directNpmVersion)) `
            -Message "$($engine.Name) read-only npm.cmd --version did not match direct invocation (exit $($realNpmCase.ExitCode)): $($realNpmCase.Output)"
    }

    Write-Host "Workflow structure and command forwarding checks passed under $($engines.Count) PowerShell host(s)."
}
finally {
    if ($null -eq $savedLogDir) { Remove-Item Env:CI_LOG_DIR -ErrorAction SilentlyContinue } else { $env:CI_LOG_DIR = $savedLogDir }
    if ($null -eq $savedWrapperPath) { Remove-Item Env:CI_TEST_WRAPPER -ErrorAction SilentlyContinue } else { $env:CI_TEST_WRAPPER = $savedWrapperPath }
    if ($null -eq $savedName) { Remove-Item Env:CI_TEST_NAME -ErrorAction SilentlyContinue } else { $env:CI_TEST_NAME = $savedName }
    if ($null -eq $savedCommandJson) { Remove-Item Env:CI_TEST_COMMAND_JSON -ErrorAction SilentlyContinue } else { $env:CI_TEST_COMMAND_JSON = $savedCommandJson }

    $tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    $resolvedTestRoot = [IO.Path]::GetFullPath($testRoot)
    if ($resolvedTestRoot.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path -Leaf $resolvedTestRoot) -like 'ci command forwarding *') {
        Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
