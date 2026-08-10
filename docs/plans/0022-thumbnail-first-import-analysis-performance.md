# Thumbnail-first import and analysis performance

## Status

Completed — thumbnail-first implementation, isolated benchmarks, regression
tests, documentation, and frontend/Rust verification are complete.

## Problem statement

Cold import is still allowed to decode a full-resolution source image in order
to create the first application thumbnail. That work is necessary only when no
usable preview cache exists; it must not be repeated for basic-feature
reanalysis, recovered jobs, or semantic classification. Semantic classification
already receives the `grid-640-v1` cache path, but the worker uses a small batch
and can turn one missing cache path into a batch failure followed by one model
retry per item.

## Baseline evidence

- The scan progress payload already measures discovery, fingerprint, image
  processing, decode, resize, feature, thumbnail-write, and database stages.
- Repository documentation recorded a five-image cold-import sample dominated by
  source decode and resize rather than BLAKE3 or SQLite.
- On the current machine, the pre-change release TinyCLIP benchmark over 48
  repository PNG fixtures measured approximately 65.6 images/second at batch 8,
  95.3 at batch 16, 97.2 at batch 32, and 98.4 at batch 64. This established
  that batch 8 was leaving throughput on the table, while the benchmark did not
  yet represent 640px photographic thumbnails.
- The benchmark and all planned scan measurements use only `test-data/`,
  `src-tauri/icons/`, or isolated temporary directories. No personal photo
  directory is read or modified.

## Scope

1. Make a current cached thumbnail authoritative for reprocessing: decode the
   cache for basic features and read only source metadata/dimensions; never
   decode source pixels again.
2. Reuse a valid embedded JPEG EXIF thumbnail when one exists, falling back to a
   single source decode only when no usable preview is available.
3. Keep semantic input strict: only a current `grid-640-v1` file may reach the
   model; missing cache entries fail visibly without invoking a full-resolution
   fallback or a retry storm.
4. Increase semantic batch size within a bounded memory-safe limit and batch
   SQLite state/result writes so each group does not commit once per image.
5. Preserve full BLAKE3 fingerprints, read-only source behavior, cache
   invalidation by source size/mtime, and per-item failure isolation.
6. Add timing/regression tests and document cold, warm, cache-reuse, and semantic
   batch measurements.

## Non-goals

- No RAW/HEIC/video support or new production image-processing dependency.
- No change to the 640px cache contract or model quality thresholds.
- No filesystem mutation outside application cache/database directories.
- No semantic analysis of original source pixels.

## Verification

- Rust tests cover cached-thumbnail reprocessing, embedded-thumbnail fallback,
  strict semantic cache paths, missing-cache isolation, and cold/warm scan
  invariants.
- Release benchmark compares semantic batch sizes on repository fixtures.
- Frontend typecheck, lint, format, tests, and production build pass.
- Rust tests/all targets and clippy pass when the local Rust toolchain is
  available.
- Final diff and generated benchmark outputs are reviewed; benchmark outputs
  remain ignored under `benchmark-output/`.

## Implemented evidence (2026-08-10)

- Isolated release import benchmark with two generated 4000x3000 JPEG fixtures:
  cold import was approximately 360ms, including 223ms source decoding and
  117ms resizing; fingerprinting was approximately 5ms and database writes
  approximately 29ms.
- The same isolated database after removing only the basic-feature rows reused
  the cached thumbnails in approximately 204ms: source pixel decode was 0ms,
  cached-thumbnail decode was approximately 14ms, source-dimension reads were
  approximately 4ms, and no resize or thumbnail write occurred.
- A warm scan of the unchanged pair was approximately 141ms and performed no
  image decode, resize, or model work.
- Release TinyCLIP throughput after the change was approximately 76.8 images/s
  at batch 8 and 87.7 images/s at the application batch size 32 on the 48 PNG
  fixtures, with zero failures. The run is still subject to model/runtime
  variance; the key correctness guarantee is that semantic candidates are
  cache-only and are persisted per batch.
- Verification passed: Rust 57 library tests plus all binary-target tests,
  Rust formatting, Clippy with warnings denied, frontend format/lint/typecheck,
  36 frontend tests, and the production build.
