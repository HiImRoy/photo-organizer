[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$modelRoot = Join-Path $projectRoot 'src-tauri\resources\models'
$placesRoot = Join-Path $modelRoot 'places365-resnet18'
$siglipRoot = Join-Path $modelRoot 'siglip2-base-patch16-224'
$subjectRoot = Join-Path $modelRoot 'subject-picodet'
$faceRoot = Join-Path $modelRoot 'subject-yunet'
$runtimeRoot = Join-Path $projectRoot 'src-tauri\resources\runtime'
$required = @(
    @{ Path = (Join-Path $placesRoot 'resnet18_places365.onnx'); Sha256 = '3c3cd0d42693e2957fcaa0bc365ce78e169a2e1162356742adfbd11077e8f7bf' },
    @{ Path = (Join-Path $placesRoot 'categories_places365.txt'); Sha256 = '6cc3f1f8eae85b7016dc634e2d333cdcce5fd16cfada4afd87977fff5f8b12ba' },
    @{ Path = (Join-Path $placesRoot 'IO_places365.txt'); Sha256 = 'd7e6abfeb228d789720326e630bedd231a7eaedcae8fd13d6d9dcd8eca95f59e' },
    @{ Path = (Join-Path $siglipRoot 'model_int8.onnx'); Sha256 = 'bfe28fe2ccdb685874586648035ea349593e487ce33bd0939b28813681a8f167' },
    @{ Path = (Join-Path $siglipRoot 'tokenizer.json'); Sha256 = 'cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322' },
    @{ Path = (Join-Path $subjectRoot 'picodet_s_320_lcnet_postprocessed.onnx'); Sha256 = '09fc88131be8ad224f13739a5cf8fc838600d76a77539af7f0400fa90506c5f3' },
    @{ Path = (Join-Path $faceRoot 'face_detection_yunet_2023mar.onnx'); Sha256 = '8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4' },
    @{ Path = (Join-Path $runtimeRoot 'onnxruntime.dll'); Sha256 = '8a1aad8d59d02a5337d4e3f5bbd1158c3f7bf84fe3b3f0052f957dd3e75a91cb' },
    @{ Path = (Join-Path $placesRoot 'PLACES365-LICENSE.txt'); Sha256 = $null },
    @{ Path = (Join-Path $placesRoot 'MODEL-SOURCE.md'); Sha256 = $null },
    @{ Path = (Join-Path $siglipRoot 'config.json'); Sha256 = $null },
    @{ Path = (Join-Path $siglipRoot 'preprocessor_config.json'); Sha256 = $null },
    @{ Path = (Join-Path $siglipRoot 'MODEL-SOURCE.md'); Sha256 = $null },
    @{ Path = (Join-Path $subjectRoot 'coco80.txt'); Sha256 = $null },
    @{ Path = (Join-Path $subjectRoot 'MODEL-SOURCE.md'); Sha256 = $null },
    @{ Path = (Join-Path $faceRoot 'MODEL-SOURCE.md'); Sha256 = $null },
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
