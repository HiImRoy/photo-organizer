# Large-library responsiveness

## Goal

Keep large-library removal and filtering responsive while preserving library
ownership, shared-cache, cancellation, and original-file safety semantics.

## Plan

1. Bound asset-removal working memory with keyset batches and remove redundant
   per-asset database work.
2. Replace per-asset preview-cache directory scans with one private-cache pass;
   keep cache cleanup best-effort and constrained to the application cache root.
3. Inspect filter query plans on isolated temporary databases and optimize only
   a demonstrated safe hotspot.
4. Add regression coverage for large batches, shared cache references, Unicode
   paths, and unchanged source files; run targeted Rust checks and review diff.

## Implemented

- Library removal reads assets in 256-row keyset batches and reuses prepared
  update/delete statements. Thumbnail references are checked in bounded SQL
  batches after all reassignments/deletions, preserving shared caches.
- Post-commit preview cleanup makes one pass over the app-owned `previews`
  directory and matches the sorted deleted asset IDs. The result retains this
  compact ID list (O(number of deleted assets)) until cleanup; full paths and
  fingerprints are no longer retained. Thumbnail cache paths are deduplicated.
- Cache cleanup rejects app-data/cache roots that are symlinks or Windows
  reparse points, requires cache roots to be direct app-data children, and only
  removes regular files inside those roots.
- Asset-count SQL omits feature-table joins unless the active filter reads those
  features. Asset list/query and library removal work runs on Tauri's blocking
  pool.
- Frontend text search waits 250 ms after the latest edit and invalidates older
  asset-query generations so stale results cannot replace newer search results.
  The grid and asset cards are memoized; stable grid handlers and a selected-ID
  set reduce avoidable card renders and repeated linear selection checks.
- The thumbnail data-URL cache now uses LRU eviction with limits of 256 entries
  and 32 Mi characters total. A single source larger than the character budget
  is returned to its caller but is not retained. The existing screen-preview
  cache remains LRU-bounded to 48 entries. These caches hold data URLs, so
  eviction drops string references without creating object URLs.
- Library removal completion filters the latest library list and checks the
  latest browse source before falling back to the first remaining library. A
  newer library, Favorites, or collection selection survives an in-flight
  removal unless that selected library is itself removed.

## Verification

- `cargo test --lib db::tests::`: 34 passed.
- Targeted IPC cache cleanup tests: 2 passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --lib --tests -- -D warnings`: passed.
- `git diff --check`: passed. Full all-target Rust tests were not run.
- `npx vitest run src/components/thumbnailSource.test.ts`: 7 passed, including
  unique-source LRU eviction, repeated access, failed-request retry, and the
  aggregate character budget.
- Targeted text-search debounce test in `src/App.test.tsx`: 1 passed; 72 other
  tests in that file were skipped. TypeScript typecheck passed. Targeted ESLint
  and Prettier checks passed for the frontend files touched in this milestone.
- Library-removal selection regression coverage in `src/App.test.tsx`: 4 passed;
  the full App test file passed (77 tests). Targeted ESLint, Prettier, and
  TypeScript typecheck passed for the removal-race patch.
- The full frontend Vitest suite was not run.

## Residual risk

The gallery still loads pages continuously and retains loaded asset records and
mounted card state. Each mounted card can keep its thumbnail data URL after the
shared LRU cache evicts it, so total memory can still grow as users scroll
through a very large library. Virtualization or page-window eviction remains
future work.

## Constraints

No personal photo directories, no source-file deletion, and no schema or
dependency change. Work for this milestone spans the Rust backend and frontend
search, rendering, and thumbnail-cache paths documented above.
