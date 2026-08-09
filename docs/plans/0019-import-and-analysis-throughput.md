# Import and semantic-analysis throughput follow-up

## Status

Implemented; pending measurement on the user's real library.

## Evidence

- Cold image import is dominated by full-resolution decode and resize. The latest
  repository-owned measurement recorded roughly 28.8 seconds in decode and 7.5
  seconds in resize/orientation for five large JPEGs; fingerprinting and SQLite
  writes were each below 100 ms in aggregate.
- Semantic analysis exposes `classify_batch`, but the desktop task runner calls it
  once per asset. Each call also decodes the original file even though import has
  already produced a valid `grid-640-v1` cache image.

## Scope

1. Let semantic candidates carry a valid grid-thumbnail path when available, with
   an absolute-source fallback for missing or stale caches.
2. Process semantic candidates in a small bounded batch and retain per-asset
   persistence, source-fingerprint checks, cancellation, pause, and failure
   isolation.
3. Process fresh import image work with at most two workers. Keep discovery,
   ownership resolution, cache-hit decisions, SQLite writes, and progress ordering
   at the scanner boundary.
4. Preserve full-content BLAKE3 fingerprints, source read-only behavior, one
   640px cache, parent/child source-root pruning, and all existing classification
   semantics.

## Non-goals

- No change to Library hierarchy or asset ownership rules.
- No replacement of authoritative fingerprints with size/mtime keys.
- No unbounded worker pool or concurrent SQLite writes.
- No change to semantic taxonomy, thresholds, UNKNOWN/FAILED semantics, or manual
  overrides.

## Verification

- Compare semantic batch size 1 and bounded batch throughput on repository-owned
  fixtures.
- Verify cold import still creates the same cache dimensions and warm rescan
  performs no image processing.
- Run Rust tests, frontend tests, formatting, lint, type check, and production
  build.

## Implementation result

- Semantic analysis now prefers a current `grid-640-v1` thumbnail and falls back
  to the source image only when that cache is unavailable or fails.
- Semantic inference is issued in batches of up to eight images. If a batch fails,
  the runner retries individual images so one bad source does not fail the whole
  batch.
- Fresh import image work is processed by at most two scoped workers; ownership,
  cache-hit checks, progress persistence, and SQLite writes remain serialized.
- On the repository-owned five-image CPU benchmark, batch size 1 measured about
  402.6 ms/image and batch size 8 about 291.1 ms/image. The application path is
  expected to be faster still because it now feeds the 640px cache rather than
  reopening the original high-resolution files.
