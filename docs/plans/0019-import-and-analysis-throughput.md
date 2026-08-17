# Import and semantic-analysis throughput follow-up

## Status

Implemented baseline; superseded by the strict thumbnail-only contract in
`docs/plans/0036-thumbnail-only-decode.md`. Plan 0022 is also historical and
must not be read as permission to decode source pixels.

## Evidence

- The historical cold-import baseline was dominated by full-resolution decode and resize. The latest
  repository-owned measurement recorded roughly 28.8 seconds in decode and 7.5
  seconds in resize/orientation for five large JPEGs; fingerprinting and SQLite
  writes were each below 100 ms in aggregate.
- Semantic analysis now receives the current `grid-640-v1` cache path. The
  remaining throughput work is batch sizing, cache-miss isolation, and grouped
  SQLite persistence rather than source-image decoding.

## Scope

1. Require a current grid-thumbnail path for semantic candidates; do not fall
   back to the absolute source during semantic analysis.
2. Process semantic candidates in a bounded batch and retain per-asset source
   checks, cancellation, pause, and failure isolation while grouping SQLite
   persistence by batch.
3. Process fresh import thumbnail work with a bounded two-to-four worker pool. Keep
   discovery, ownership resolution, cache-hit decisions, SQLite writes, and
   progress ordering at the scanner boundary.
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

- Semantic analysis now requires a current `grid-640-v1` thumbnail. Missing or
  unreadable cache files fail the semantic item instead of reopening the source.
- Semantic inference is issued in batches of up to 32 images. If a batch fails,
  the runner retries individual thumbnails so one bad cache entry does not fail
  the whole batch. A missing cache path is failed before the model is invoked.
- Semantic results for one batch are committed in one SQLite transaction.
- Fresh import thumbnail work is processed by a bounded two-to-four scoped worker
  pool; ownership,
  cache-hit checks, progress persistence, and SQLite writes remain serialized.
- On the repository-owned five-image CPU benchmark, batch size 1 measured about
  402.6 ms/image and batch size 8 about 291.1 ms/image. The application path is
  expected to be faster still because it now feeds the 640px cache rather than
  reopening the original high-resolution files.

## 2026-08-10 follow-up (implemented)

The first implementation still allowed a missing/stale thumbnail to fall back
to the original source during semantic retry. That was both slower and contrary
to the thumbnail-first contract. The follow-up now:

1. Require a current `grid-640-v1` cache row for semantic candidates.
2. Keep batch and per-image retry on that cache path only; never reopen the
   original source from the semantic worker.
3. Raise the bounded import image-worker limit based on available CPU, while
   keeping ownership resolution and SQLite writes serialized.
4. Include tests that prove semantic candidates and retry paths use the cache
   and that cold import still keeps exact source fingerprints and read-only
   sources.

## 2026-08-10 thumbnail-first follow-up

- A valid cache is now reused for basic-feature reprocessing. The source is read
  only for EXIF and dimensions; `source_decode_us` remains zero.
- JPEGs with a valid EXIF embedded preview use that preview for first import,
  avoiding a primary-pixel decode. Other formats still perform the single decode
  required to create the first application thumbnail.
- The old 4000x3000 JPEG measurements with 223 ms source decode are historical
  pre-0036 evidence. Current acceptance is the isolated WIC bounded-decode
  measurement recorded in plan 0036, where `source_decode_us` remains zero.
- Release TinyCLIP over 48 repository PNG fixtures measured 76.8 images/second
  at batch 8 and 87.7 images/second at batch 32 after prompt-token caching. The
  benchmark is a model throughput smoke test, not a claim about photographic
  classification quality.
