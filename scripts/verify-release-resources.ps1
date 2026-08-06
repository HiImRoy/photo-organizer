[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$modelRoot = Join-Path $projectRoot 'src-tauri\resources\models\tinyclip-vit-8m-16-text-3m-yfcc15m'
$runtimeRoot = Join-Path $projectRoot 'src-tauri\resources\runtime'
$required = @(
    @{ Path = (Join-Path $modelRoot 'model-int8.onnx'); Sha256 = '10921310ddef06557ec1598d1260470a0a4db53f70ffe0deb60b946dcad6d27a' },
    @{ Path = (Join-Path $modelRoot 'tokenizer.json'); Sha256 = '6d9109cc838977f3ca94a379eec36aecc7c807e1785cd729660ca2fc0171fb35' },
    @{ Path = (Join-Path $runtimeRoot 'onnxruntime.dll'); Sha256 = '8a1aad8d59d02a5337d4e3f5bbd1158c3f7bf84fe3b3f0052f957dd3e75a91cb' },
    @{ Path = (Join-Path $modelRoot 'config.json'); Sha256 = $null },
    @{ Path = (Join-Path $modelRoot 'preprocessor_config.json'); Sha256 = $null },
    @{ Path = (Join-Path $modelRoot 'MIT-LICENSE.txt'); Sha256 = $null },
    @{ Path = (Join-Path $modelRoot 'MODEL-SOURCE.md'); Sha256 = $null },
    @{ Path = (Join-Path $runtimeRoot 'ONNXRUNTIME-LICENSE.txt'); Sha256 = $null },
    @{ Path = (Join-Path $runtimeRoot 'ONNXRUNTIME-THIRD-PARTY-NOTICES.txt'); Sha256 = $null },
    @{ Path = (Join-Path $runtimeRoot 'RUNTIME-SOURCE.md'); Sha256 = $null }
)

foreach ($resource in $required) {
    if (-not (Test-Path -LiteralPath $resource.Path -PathType Leaf)) {
        throw "Required release resource is missing: $($resource.Path)"
    }
    $item = Get-Item -LiteralPath $resource.Path
    $hash = Get-FileHash -Algorithm SHA256 -LiteralPath $resource.Path
    if ($resource.Sha256 -and $hash.Hash -ne $resource.Sha256) {
        throw "Release resource hash mismatch: $($resource.Path)"
    }
    $relativePath = $resource.Path.Substring($projectRoot.Length).TrimStart([IO.Path]::DirectorySeparatorChar)
    [pscustomobject]@{
        RelativePath = $relativePath
        Bytes        = $item.Length
        Sha256       = $hash.Hash.ToLowerInvariant()
    }
}
