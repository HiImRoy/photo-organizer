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
- Interactive desktop speed verification remains pending.
- Rust and frontend validation pass; source-integrity tests remain green.
