# Import Performance Optimization Plan

Status: IMPLEMENTED_PENDING_MANUAL

## Goal

Reduce first-import latency without weakening source integrity, cache invalidation, or
organization/export fingerprint checks.

## Scope

1. Generate one bounded thumbnail image per source file and reuse that in-memory image
   for both the grid cache and basic imaging feature extraction.
2. Version the thumbnail and imaging algorithms so existing cache/results are rebuilt
   deterministically rather than mixed with the old pipeline.
3. Keep the full-content BLAKE3 fingerprint as the authoritative source fingerprint.
   A fast metadata key may be used only as a provisional cache/index key; it must not
   replace the authoritative fingerprint used by semantic results, export preview, or
   COPY safety checks.
4. Reduce repeated source reads where it is safe to do so, while bounding memory use
   for unusually large images.
5. Add regression coverage for shared thumbnail dimensions, feature extraction, cache
   invalidation, and fingerprint preservation.

## Explicit non-goals

- Do not modify original source files.
- Do not replace full-content fingerprint checks with size/mtime-only checks.
- Do not start Checkpoint B or change classification semantics.
- Do not add unbounded image-processing threads.

## Phase 2 responsiveness scope

The first implementation removed the duplicate thumbnail resize/read path, but it
did not yet address the per-file coordination overhead that can make the desktop
window appear frozen during a large import. The next implementation is limited to:

1. Coalesce scan-progress events to a bounded time cadence while still emitting
   start, completion, cancellation, and failure states immediately.
2. Persist scan-job progress in batches instead of opening a SQLite connection for
   every processed file.
3. Treat generated thumbnails as rebuildable application cache and avoid forcing a
   disk flush for every thumbnail file.
4. Coalesce frontend asset/library refreshes so progress updates do not repeatedly
   reload the grid and sidebar.

This phase does not change source ownership, asset identity, scan completeness, or
the authoritative full-content fingerprint.

## Phase 3 measurement scope

Before changing image or database algorithms again, record cumulative microsecond
timings for directory discovery, library ownership lookup, metadata/cache lookup,
full-content fingerprinting, EXIF/decode, resize, feature extraction, thumbnail
encoding/write, and SQLite writes. The snapshot is attached to throttled scan
progress events and written to the application log at scan completion so a real
library import can identify its dominant stage without touching source files.

## Latest log diagnosis (2026-08-08)

The latest desktop scan log is a cold import of 5 successful JPEGs:

```text
discovery=0ms ownership_lookup=20ms metadata_lookup=22ms fingerprint=27ms
image_processing=37061ms exif=1ms decode=28756ms resize=7459ms
feature_analysis=510ms thumbnail_write=324ms database_write=46ms
successful_images=5 skipped=0 failed=0
```

The corresponding five most recently seen database assets are approximately
4,113–4,864 pixels wide, 2,742–3,648 pixels high, and 3.46–4.30 MB each
(76.28 MP and 20.20 MB in total). The current timing establishes the following
ranking:

1. Decode: 28.756 s, about 77.6% of `image_processing`.
2. Resize/orientation/RGBA conversion: 7.459 s, about 20.1%.
3. Feature analysis: 0.510 s, about 1.4%.
4. Thumbnail encoding/write: 0.324 s, under 1%.

Fingerprinting, ownership lookup, metadata lookup, and SQLite writes are all
below 100 ms in aggregate for this scan. The fingerprint is therefore not the
cause of the observed import delay. The existing shared 640px buffer already
avoids a second feature-analysis resize; changing the feature algorithm will not
address this bottleneck.

This log was produced by the current `target/debug/photo-organizer.exe`. No
Release executable was present in `src-tauri/target/release` during diagnosis,
so the first comparison must establish an optimized Release baseline. The
current scanner also processes candidates serially, and the `resize` timer
includes full-resolution EXIF orientation transforms and RGBA conversion, so
the next measurement must split those substeps before selecting an algorithm.

## Targeted follow-up plan (P0 completed; mainline resumed)

### P0 — Establish a trustworthy baseline

- Build and run the desktop app in Release mode against a repository-owned,
  synthetic fixture with the same 4K JPEG dimensions and a cold cache.
- Record wall time, per-file time, CPU usage, peak working set, and the existing
  cumulative stage timings. Repeat the same import with a warm cache and verify
  that it performs zero image processing for unchanged assets.
- Add slow-file diagnostics before changing the pipeline: format, source bytes,
  source dimensions, orientation, cache-hit/miss, decode, orientation,
  downsample, feature, encode, and database durations. Log only relative path or
  file name, never a full personal source path.

P0 implementation status:

- The optimized Release executable has been built at
  `src-tauri/target/release/photo-organizer.exe`.
- A deterministic repository-local fixture is available at
  `benchmark-output/import-performance-fixture-20260808/` with five
  `4608x3456` JPEGs (about 79.6 MP total). The directory is ignored by git and
  contains no user media.
- Temporary `build_profile` and per-image slow-processing diagnostics were used
  for the baseline and have now been removed from the application log path.
  Stage timings remain available through scan progress for the UI.
- The user confirmed that Release import speed is now acceptable. No decoder or
  concurrency change is being added in this round; the remaining P1-P3 work is
  retained as a regression follow-up if a larger real-world library exposes a
  new bottleneck.

### P1 — Remove unnecessary full-resolution pixel work

- Evaluate a decoder-level bounded output path for JPEG (native DCT/downsample
  where the selected decoder supports it) and an equivalent bounded path for
  PNG/WebP where available.
- If decoder-level scaling is unavailable, evaluate a small, explicitly approved
  decoder dependency or keep the current fallback; do not silently add a
  production dependency. Any dependency choice requires an ADR covering license,
  memory, format compatibility, and migration cost.
- Fold EXIF orientation into the bounded decode/thumbnail path where possible.
  Do not rotate the full-resolution image merely to produce a 640px cache.
- Split timings into `decode`, `orientation_transform`, `downsample`,
  `rgba_conversion`, `feature_analysis`, and `thumbnail_write` so the next
  decision is evidence-based.

### P2 — Add bounded image-processing concurrency

- After P1 is measured, process at most two image jobs concurrently. Keep
  ownership resolution, cancellation checks, and SQLite writes deterministic and
  serialized at the scanner boundary.
- Enforce a memory budget so two large decoded images cannot create an
  unbounded working-set spike. Do not create one task per discovered file.
- Preserve Parent/Child prune behavior, Most Specific owner resolution,
  full-content BLAKE3 fingerprinting, and source read-only guarantees.

### P3 — Regression and acceptance gates

- Compare cold and warm imports on the same Release binary and fixture before
  and after each phase; report total, per-file p50/p95, CPU, and peak memory.
- The first optimization gate is at least a 3× cold-import wall-time reduction
  versus the Release baseline, with no source hash changes. If P1 cannot reach
  that gate, stop and reassess the decoder before adding concurrency.
- Warm rescan must continue to skip unchanged assets without fingerprinting or
  image processing; cancellation, missing detection, cache invalidation, and
  thumbnail dimensions must remain correct.
- Manual verification must use the Release executable for speed judgments. The
  Debug executable may be used for UI debugging but must not be used to accept or
  reject import-performance work.

This follow-up plan does not start Checkpoint B and does not change any
classification semantics.

## Acceptance criteria

- A new asset produces one cache image whose actual dimensions match its cache spec.
- Basic imaging features are derived from the same resized pixel buffer used for the
  cache, with no second resize of the decoded source.
- Existing old-version cache/results are invalidated through explicit version changes.
- Source fingerprint remains full-content BLAKE3 and remains available for semantic and
  organization safety checks.
- Unchanged assets continue to skip hashing and image processing when their metadata,
  versions, and cache are valid.

## Implementation snapshot

- `THUMBNAIL_SPEC` is now `grid-640-v1`; the actual cache dimensions are bounded and
  no longer upscaled for small source images.
- `ANALYSIS_VERSION` is now `basic-color-v3`; features sample the shared 640px buffer
  instead of creating a separate 320px image.
- Sources up to and including 32MB are read once and reuse the same bytes for full BLAKE3 hashing,
  EXIF parsing, and image decode. Larger sources retain streaming fingerprinting to
  bound memory use.
- Phase 2 coalesces scan events and job progress, throttles frontend refreshes, and
  avoids per-thumbnail physical disk flushes.
- Phase 3 exposes cumulative scan-stage timings in scan progress; the temporary
  final timing log used during diagnosis has been removed. Single-image preview
  requests the original source first, falls back to a bounded screen preview on
  read failure, and keeps heavy preview generation on a blocking worker.
- Latest diagnosis showed that the earlier slow result came from the Debug build's
  full-resolution image processing, not fingerprinting or SQLite. Release import
  speed was manually accepted, and the temporary diagnostic log code has been
  removed. P1-P3 remain available only as a future regression path.
- Rust and frontend validation pass; source-integrity tests remain green.
