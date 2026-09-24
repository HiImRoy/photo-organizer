# Import performance optimization: phase 1 baseline

## Scope and assumptions

Phase 1 establishes a release-profile baseline for the existing import path. It makes no application-code changes. The synthetic fixture, database, thumbnails, benchmark report, and any build output must stay beneath one newly selected `test-data/.tmp/import-opt-<id>` directory whose absence is checked immediately before creation. Do not remove a pre-existing path; preserve the generated directory after measurement.

The user supplied a live-library screenshot snapshot: 5,892 discovered and 1,536 done, with 217.73 s feature analysis, 382.99 s image processing, 110.50 s metadata/ownership, 39.14 s fingerprinting, 42.83 s thumbnail decode, and 3.74 s database writes. Treat this as an in-progress snapshot, not a completed-library wall-time benchmark. These are accumulated worker-stage values; thumbnail decode is nested within image processing, and worker stages overlap. Do not add them or compare their sum to elapsed wall time.

The release measurements in 0034/0035 are the comparable historical synthetic baseline. The newer 0066 run used an unoptimized development build and is retained only as diagnostic context, never as a substitute for release results.

The benchmark CLI has no worker-count option. `scan_library` uses the scanner’s automatic setting, clamped to 1–2 image workers. Record this limitation and the detected machine parallelism; do not claim a 1-worker versus 2-worker comparison.

## Recorded baseline snapshots

The user-supplied live screenshot snapshot is partial (5,892 discovered / 1,536 done):

| Counter                                          | Accumulated value |
| ------------------------------------------------ | ----------------: |
| Feature analysis                                 |          217.73 s |
| Image processing parent total                    |          382.99 s |
| Metadata/ownership                               |          110.50 s |
| Fingerprinting                                   |           39.14 s |
| Thumbnail decode (nested under image processing) |           42.83 s |
| Database writes                                  |            3.74 s |

The previously recorded release benchmark used 1,000 generated JPEGs at 320×240, quality 85. It completed all 1,000 images with zero failures:

| Pass                            | Wall time | Feature-analysis accumulated time | Database-write accumulated time |
| ------------------------------- | --------: | --------------------------------: | ------------------------------: |
| Cold import                     |    16.3 s |                            20.8 s |                          1.00 s |
| Cache reuse / feature recompute |    14.9 s |                            20.0 s |                          0.79 s |
| Warm rescan                     |     3.7 s |                      Not recorded |                    Not recorded |

The newer diagnostic run (development profile; not comparable to release) used 16 generated 1280×960 JPEGs. Cold wall time was 3,633.2 ms and accumulated image-processing time was 6,720.5 ms (thumbnail decode 161.3 ms; feature analysis 3,622.0 ms). Cache-reuse wall time was 3,387.3 ms and accumulated image-processing time was 6,184.3 ms (thumbnail decode 2,493.2 ms; feature analysis 3,651.3 ms). Warm wall time was 251.0 ms, with 16 skipped. Its other recorded stages were fingerprint 11.2/9.9 ms, metadata lookup 126.5/137.4 ms, and database write 30.3/27.1 ms for cold/cache reuse respectively; warm metadata/database were 131.4/12.5 ms.

Stage durations are accumulated worker-stage timings and can overlap in time. Image-processing totals also contain the reported decode and feature-analysis child timings. Do not add parent and child values, or compare their sum with wall time. Parallel workers can make a cumulative stage exceed wall time. Compare wall time separately and compare like-for-like stage counters across runs.

## Benchmark protocol

1. Choose a fresh unique ID and verify the exact `test-data/.tmp/import-opt-<id>` path does not exist. Use only that root for fixtures, application data, build artifacts, captured JSON stdout, and memory notes.
2. Set `CARGO_TARGET_DIR` to a path beneath the validated root. Query `cargo metadata --no-deps --format-version 1 --locked --offline` with that environment and use its reported `target_directory`; verify the release executable exists there after building. If the release build cannot complete within a reasonable local build window, record the command and failure/limitation; do not report a development-profile run as release data.
3. Run the binary once with `--generate 1000 --width 320 --height 240 --quality 85 --data-dir <root>/run`. The CLI writes generated sources under `run/source-fixtures`, and its database and thumbnails beneath `run/app-data`. It reports cold, cache-reuse, and warm passes in one JSON document.
4. Capture the complete JSON report from stdout in this plan. Record host logical processor count, the scanner’s automatic 1–2 worker bound, release profile, fixture count/dimensions/quality, and peak process working set if the Windows process API makes it available without materially perturbing the run.
5. Check pass counts and failures: cold and cache reuse should process all 1,000 fixture files; warm should reuse existing state; all passes must have zero failures. Preserve the generated directory and report its exact path.

Suggested invocation after the unique root is validated (the custom Cargo target keeps build artifacts inside the same authorized root):

```powershell
$cargo = 'C:\Users\13002\.cargo\bin\cargo.exe'
$env:PATH = "C:\Users\13002\.cargo\bin;$env:PATH"
$root = (Resolve-Path "test-data/.tmp/import-opt-<id>").Path
$env:CARGO_TARGET_DIR = [System.IO.Path]::GetFullPath((Join-Path $root "cargo-target"))
$metadata = & $cargo metadata --no-deps --format-version 1 --locked --offline --manifest-path src-tauri/Cargo.toml | ConvertFrom-Json
$targetDir = [System.IO.Path]::GetFullPath($metadata.target_directory)
if ($targetDir -ine $env:CARGO_TARGET_DIR) { throw "Cargo target_directory does not match the isolated root" }
& $cargo build --release --locked --offline --manifest-path src-tauri/Cargo.toml --bin import-benchmark
$exe = Join-Path $targetDir "release/import-benchmark.exe"
if (!(Test-Path -LiteralPath $exe)) { throw "Release benchmark executable missing: $exe" }
& $exe --generate 1000 --width 320 --height 240 --quality 85 --data-dir (Join-Path $root "run")
```

## Staged optimization and acceptance

1. **Baseline (this phase):** capture release cold, cache-reuse, and warm wall times, phase counters, work counts, failures, and peak memory if feasible. The report is accepted only when it is from the release profile, uses the bounded synthetic fixture, is isolated under the validated directory, and has 1,000/1,000 completion with zero failures. If the release build is infeasible, leave the baseline explicitly unmeasured.
2. **Bottleneck selection:** use repeated release runs of the same bounded workload to identify the stage that limits wall-clock throughput. Account for overlapping worker timings and nested image-processing counters. Do not choose a target solely because its accumulated stage time is largest.
3. **One targeted change per follow-up:** optimize the measured limiting stage while preserving bounded discovery/processing, the 1–2 worker ceiling, batched database writes, and thumbnail-only processing. Keep unrelated behavior and architecture unchanged.
4. **Optimization acceptance:** compare at least three release runs per variant on the same fixture and environment. Require at least a 10% median improvement in the targeted wall-clock outcome, zero import failures, unchanged image/feature correctness, no more than a 5% warm-pass regression, and no peak-memory increase beyond measurement noise. Re-run relevant safety and regression coverage for any later code change.

## Phase 1 measured results

**Completed:** two release-profile synthetic cohorts were measured on 2026-09-24. The benchmark utility generated all source JPEGs; no personal photo directory was accessed. The exact output and build root was validated absent before creation and is preserved at `D:\Code\Codex\photo-manager\test-data/.tmp/import-opt-20260924-68b40f5e`.

- Cargo was invoked by absolute path `C:\Users\13002\.cargo\bin\cargo.exe`. Cargo metadata reported and the run verified `D:\Code\Codex\photo-manager\test-data/.tmp/import-opt-20260924-68b40f5e\cargo-target` as the effective `target_directory`; the executable was taken from its `release` subdirectory. The offline, locked release build completed in 3m03s.
- The host reported 24 logical processors. The benchmark CLI exposes no worker override; the scanner automatically selected 2 workers under its 1–2 worker clamp.
- Both cohorts used generated JPEGs at quality 85. The 1,000 × 320×240 cohort and the separate 128 × 1280×960 cohort each used its own `run-*` data directory and therefore an independent database/cache.
- Peak process working set was captured from the Windows process handle: 45.1 MiB for the small-image cohort and 40.9 MiB for the larger-image cohort.
- All cold and cache-reuse passes completed every fixture with zero failures. Warm passes skipped every fixture with zero failures.
- Original stdout JSON files: `test-data/.tmp/import-opt-20260924-68b40f5e/report-1000x320.json` and `test-data/.tmp/import-opt-20260924-68b40f5e/report-128x1280.json`. Both are also embedded below. The benchmark resources are preserved under the same root.

| Synthetic workload | Cold wall | Cache-reuse wall | Warm wall | Peak working set | Cold / cache reuse / warm result                              |
| ------------------ | --------: | ---------------: | --------: | ---------------: | ------------------------------------------------------------- |
| 1,000 × 320×240    |  23.786 s |         19.145 s |   4.670 s |         45.1 MiB | 1,000 succeeded / 1,000 succeeded / 1,000 skipped; 0 failures |
| 128 × 1280×960     |   4.357 s |          3.104 s |   0.549 s |         40.9 MiB | 128 succeeded / 128 succeeded / 128 skipped; 0 failures       |

The 1,000-image result is slower than the older recorded release snapshot (16.3/14.9/3.7 s), but the old run lacks a controlled compiler, revision, and host record. Treat this as an observed difference, not a demonstrated regression. Each new cohort was run once, so these are single-run baselines rather than medians. The 128-image workload is a separate higher-resolution cohort and must not be compared by image count with the 1,000-image cohort.

Performance stage values below are cumulative worker-stage microseconds, not wall time. Parallel work overlaps; image-processing parent totals and nested decode/feature counters must not be summed. The complete JSON reports are preserved here as emitted by stdout:

### 1,000 × 320×240

```json
{
  "sourceDir": "D:\\Code\\Codex\\photo-manager\\test-data\\.tmp\\import-opt-20260924-68b40f5e\\run-1000x320\\source-fixtures",
  "dataDir": "D:\\Code\\Codex\\photo-manager\\test-data\\.tmp\\import-opt-20260924-68b40f5e\\run-1000x320",
  "generatedFiles": 1000,
  "coldWallMs": 23786.121000000003,
  "cold": {
    "taskId": "import-benchmark-cold",
    "libraryId": 1,
    "status": "completed",
    "discovered": 1000,
    "processed": 1000,
    "succeeded": 1000,
    "failed": 0,
    "skipped": 0,
    "missing": 0,
    "performance": {
      "discoveryUs": 9881,
      "ownershipLookupUs": 14707,
      "metadataLookupUs": 3876351,
      "fingerprintUs": 279354,
      "imageProcessingUs": 35205469,
      "exifUs": 40265,
      "sourceDimensionUs": 0,
      "decodeUs": 4230620,
      "sourceDecodeUs": 0,
      "thumbnailDecodeUs": 4230620,
      "resizeUs": 1159023,
      "featureAnalysisUs": 28447677,
      "thumbnailWriteUs": 1249922,
      "databaseWriteUs": 1345058,
      "processedFiles": 1000,
      "skippedFiles": 0,
      "failedFiles": 0
    }
  },
  "cacheReuseWallMs": 19145.0825,
  "cacheReuse": {
    "taskId": "import-benchmark-cache-reuse",
    "libraryId": 1,
    "status": "completed",
    "discovered": 1000,
    "processed": 1000,
    "succeeded": 1000,
    "failed": 0,
    "skipped": 0,
    "missing": 0,
    "performance": {
      "discoveryUs": 6515,
      "ownershipLookupUs": 13141,
      "metadataLookupUs": 3639760,
      "fingerprintUs": 252230,
      "imageProcessingUs": 27268994,
      "exifUs": 155972,
      "sourceDimensionUs": 111537,
      "decodeUs": 649918,
      "sourceDecodeUs": 0,
      "thumbnailDecodeUs": 649918,
      "resizeUs": 0,
      "featureAnalysisUs": 26330985,
      "thumbnailWriteUs": 0,
      "databaseWriteUs": 1006320,
      "processedFiles": 1000,
      "skippedFiles": 0,
      "failedFiles": 0
    }
  },
  "warmWallMs": 4669.7894,
  "warm": {
    "taskId": "import-benchmark-warm",
    "libraryId": 1,
    "status": "completed",
    "discovered": 1000,
    "processed": 1000,
    "succeeded": 0,
    "failed": 0,
    "skipped": 1000,
    "missing": 0,
    "performance": {
      "discoveryUs": 6171,
      "ownershipLookupUs": 12963,
      "metadataLookupUs": 3671295,
      "fingerprintUs": 0,
      "imageProcessingUs": 0,
      "exifUs": 0,
      "sourceDimensionUs": 0,
      "decodeUs": 0,
      "sourceDecodeUs": 0,
      "thumbnailDecodeUs": 0,
      "resizeUs": 0,
      "featureAnalysisUs": 0,
      "thumbnailWriteUs": 0,
      "databaseWriteUs": 645940,
      "processedFiles": 0,
      "skippedFiles": 1000,
      "failedFiles": 0
    }
  }
}
```

### 128 × 1280×960

```json
{
  "sourceDir": "D:\\Code\\Codex\\photo-manager\\test-data\\.tmp\\import-opt-20260924-68b40f5e\\run-128x1280\\source-fixtures",
  "dataDir": "D:\\Code\\Codex\\photo-manager\\test-data\\.tmp\\import-opt-20260924-68b40f5e\\run-128x1280",
  "generatedFiles": 128,
  "coldWallMs": 4356.6903999999995,
  "cold": {
    "taskId": "import-benchmark-cold",
    "libraryId": 1,
    "status": "completed",
    "discovered": 128,
    "processed": 128,
    "succeeded": 128,
    "failed": 0,
    "skipped": 0,
    "missing": 0,
    "performance": {
      "discoveryUs": 1223,
      "ownershipLookupUs": 2035,
      "metadataLookupUs": 550953,
      "fingerprintUs": 64101,
      "imageProcessingUs": 6849897,
      "exifUs": 44006,
      "sourceDimensionUs": 0,
      "decodeUs": 992965,
      "sourceDecodeUs": 0,
      "thumbnailDecodeUs": 992965,
      "resizeUs": 672657,
      "featureAnalysisUs": 3836595,
      "thumbnailWriteUs": 1278298,
      "databaseWriteUs": 154166,
      "processedFiles": 128,
      "skippedFiles": 0,
      "failedFiles": 0
    }
  },
  "cacheReuseWallMs": 3104.0208,
  "cacheReuse": {
    "taskId": "import-benchmark-cache-reuse",
    "libraryId": 1,
    "status": "completed",
    "discovered": 128,
    "processed": 128,
    "succeeded": 128,
    "failed": 0,
    "skipped": 0,
    "missing": 0,
    "performance": {
      "discoveryUs": 986,
      "ownershipLookupUs": 2147,
      "metadataLookupUs": 567740,
      "fingerprintUs": 52257,
      "imageProcessingUs": 4392197,
      "exifUs": 90533,
      "sourceDimensionUs": 37106,
      "decodeUs": 381946,
      "sourceDecodeUs": 0,
      "thumbnailDecodeUs": 381946,
      "resizeUs": 0,
      "featureAnalysisUs": 3872426,
      "thumbnailWriteUs": 0,
      "databaseWriteUs": 124030,
      "processedFiles": 128,
      "skippedFiles": 0,
      "failedFiles": 0
    }
  },
  "warmWallMs": 548.8680999999999,
  "warm": {
    "taskId": "import-benchmark-warm",
    "libraryId": 1,
    "status": "completed",
    "discovered": 128,
    "processed": 128,
    "succeeded": 0,
    "failed": 0,
    "skipped": 128,
    "missing": 0,
    "performance": {
      "discoveryUs": 945,
      "ownershipLookupUs": 1448,
      "metadataLookupUs": 417724,
      "fingerprintUs": 0,
      "imageProcessingUs": 0,
      "exifUs": 0,
      "sourceDimensionUs": 0,
      "decodeUs": 0,
      "sourceDecodeUs": 0,
      "thumbnailDecodeUs": 0,
      "resizeUs": 0,
      "featureAnalysisUs": 0,
      "thumbnailWriteUs": 0,
      "databaseWriteUs": 58574,
      "processedFiles": 0,
      "skippedFiles": 128,
      "failedFiles": 0
    }
  }
}
```

## Phase 2: Exact sRGB lookup

**Change:** `rgb_to_oklab` now reads the existing `srgb_to_linear(channel / 255)` results from a lazily initialized 256-entry `OnceLock` table. The formula, f64 values, OKLab conversion, feature and palette calculations, analysis version, and thumbnail boundary are unchanged. The production analyzer and converter keep their non-generic call signatures; the direct-formula converter exists only in tests.

**Equivalence coverage:** `srgb_lookup_matches_direct_oklab_conversion_bit_exactly` compares all 256 lookup entries to the previous formula by f64 bit pattern, then compares all three OKLab output components bit-for-bit against the direct formula across five RGB patterns for every byte value. The downstream analysis code receives the same exact channel-linearization values. The existing high-resolution bounded-thumbnail test also passed.

**Benchmark protocol:** The pre-change release executable at `test-data/.tmp/import-opt-20260924-68b40f5e/cargo-target/release/import-benchmark.exe` was run three times per cohort, followed by the rebuilt release executable. Both used `--images` with the same read-only generated fixture directories from that baseline root. Every run used a fresh `--data-dir` under the new output root, `test-data/.tmp/import-opt-after-20260924-a9136d2c`; all JSON reports and the new Cargo target are preserved there. The release build finished in 3m19s. The host had 24 logical processors; scanner worker selection remained automatic and capped at two.

All six runs for each build completed the expected cold and cache-reuse work, skipped all images on the warm pass, and reported zero failures. Times below are medians of three independent runs:

| Workload        | Pass / counter                            | Pre-change | LUT build | Change |
| --------------- | ----------------------------------------- | ---------: | --------: | -----: |
| 1,000 × 320×240 | Cold wall                                 |   23.691 s |  18.801 s | −20.6% |
|                 | Cache-reuse wall                          |   18.636 s |  17.311 s |  −7.1% |
|                 | Warm wall                                 |    4.721 s |   4.449 s |  −5.8% |
|                 | Cold feature analysis (cumulative)        |   28.170 s |  21.210 s | −24.7% |
|                 | Cache-reuse feature analysis (cumulative) |   25.615 s |  22.558 s | −11.9% |
| 128 × 1280×960  | Cold wall                                 |    3.785 s |   3.154 s | −16.7% |
|                 | Cache-reuse wall                          |    2.973 s |   2.383 s | −19.8% |
|                 | Warm wall                                 |    0.895 s |   0.524 s | −41.5% |
|                 | Cold feature analysis (cumulative)        |    3.391 s |   2.552 s | −24.7% |
|                 | Cache-reuse feature analysis (cumulative) |    3.671 s |   2.735 s | −25.5% |

Feature-analysis values are cumulative worker-stage times and may overlap; they are not added to wall time. The one-thousand-image cold wall and feature-analysis medians both improved by more than 10%. Its cache-reuse wall improvement was 7.1%, below that threshold; the 128-image cohort exceeded 10% for both cold and cache-reuse wall times. The warm medians did not regress.

The new runs' sampled peak working sets had medians of 39.3 MiB (1,000-image cohort) and 34.9 MiB (128-image cohort), lower than the phase-1 single-run readings of 45.1 MiB and 40.9 MiB. The old repeated-run working-set samples were not retained, so this is an indicative comparison rather than a repeated paired memory result.

**Verification:** `cargo fmt --all -- --check`, `git diff --check`, `cargo clippy --locked --offline --no-default-features --lib --tests`, the complete imaging test module (19 passed, including the bounded high-resolution test), the renamed bit-exact converter test (1 passed), and the locked offline release benchmark build all passed. No benchmark compilation overlapped with measurement.

**Remaining limitation:** The 1,000-image cache-reuse wall-time gain is below 10% despite the consistent improvement in feature-analysis time. The 1,000-image third new-build sample was slower than its first two samples; the reported medians account for that spread. All 256 sRGB channel values are bit-exact to the prior formula, the downstream formula is unchanged, and the sampled RGB combinations are bit-exact; no algorithm-version change was made.
