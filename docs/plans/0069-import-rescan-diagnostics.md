# Import rescan diagnostics (phase 3)

## Goal

Measure the existing scanner's rescan decision path before selecting another
performance optimization. Keep this phase diagnostic: it does not change the
SQLite lookup query or alter scan scheduling, cache policy, or import results.

## Scope and assumptions

- Restrict implementation to `src-tauri/src/scanner.rs`,
  `src-tauri/src/models.rs`, and this plan.
- Preserve `metadataLookupUs` as the wire field and as an aggregate for file
  metadata collection, existing-asset SQLite lookup, cache-path existence
  probes, and other metadata-related lookup work (including the existing
  `identity_key` calculation). Preserve the already separate
  `ownershipLookupUs` counter. The aggregate's timing boundary is intentionally
  widened: the old counter included the snapshot, identity-key calculation,
  and SQLite lookup, but stopped before the cache-path existence probe. New
  values include that probe, so the aggregate is wire-compatible but not
  strictly comparable to the pre-phase-3 aggregate.
- Add `fileMetadataUs`, `existingAssetLookupUs`, and `cacheProbeUs` to the
  serialized performance object, using the existing camel-case wire convention.
  Existing clients can ignore the new fields and continue reading
  `metadataLookupUs`.
- Measure each stage at its current boundary. Do not restructure or combine
  filesystem operations or database queries just to make the numbers cleaner.
- Count cold imports, reanalyzed existing assets, and skipped existing assets
  only where the current scanner has an unambiguous decision. Keep counts in
  the existing performance/result contract; do not add per-file diagnostic
  records or database schema changes. Expose `coldFiles` and
  `reanalyzedFiles`; retain the existing `skippedFiles` counter for skipped
  unchanged assets.
- Any new interruption/rescan fixture must be generated beneath a uniquely
  named `test-data/.tmp/import-rescan-<id>` subtree. Use
  `tempfile::Builder::tempdir_in` with a unique prefix under the canonical
  `test-data/.tmp` parent; its atomic creation and RAII cleanup must be scoped
  to that new child. An existing scanner `setup()` may be reused only if its
  source and app data already stay within a test-owned temporary directory
  under `test-data/`. Never inspect or modify personal photo directories or
  baseline fixture contents.
- The import pipeline remains thumbnail-bounded. Timing instrumentation must
  not introduce a source full-resolution decode or alter cancellation behavior.

## Execution steps

1. Inspect scanner timing and scan-state transitions, then identify the exact
   filesystem snapshot, existing-asset lookup, cache probe, ownership lookup,
   and skip/reanalyze/cold decision boundaries.
2. Add the three stage counters while continuing to accumulate their time into
   `metadataLookupUs`; leave ownership timing separate.
3. Add narrow reason counts if they can be derived directly from current
   scanner decisions without expanding storage or retry behavior.
4. Add focused tests for counter accumulation and cancellation followed by a
   restarted scan. Put any newly generated source and application data beneath
   a uniquely named `test-data/.tmp/import-rescan-<id>` subtree owned by a
   `TempDir`. Interrupt after the first committed 16-result batch, then verify
   restart skips those 16 and imports the remaining fixtures. Verify generated
   source files remain byte-for-byte unchanged and that import/analysis
   consumes only bounded thumbnails.
5. Run the focused scanner tests, Rust formatting, and the requested Clippy
   check; review the diff and report any unavailable or failing checks.
6. Run a bounded release import benchmark against existing generated source
   fixtures from plan 0068 in read-only mode. Before creating benchmark output,
   verify a new, unique `test-data/.tmp/import-rescan-diagnostics-<id>` root is
   absent. Put all benchmark data and build output beneath that root, preserve
   the 0068 sources, and record cold, cache-reuse, and warm values for the
   split counters and wall time. Treat 0068's old `metadataLookupUs` values as
   historical context only; do not present them as a like-for-like comparison
   with this widened aggregate. If a release rebuild is infeasible, record the
   concrete limitation.
7. Report the exact wire fields added so the frontend can consume them without
   relying on this implementation's internal timing layout.

## Acceptance criteria

- `metadataLookupUs` remains present and is at least the sum of the three new
  counters; it also includes `identity_key` work. Its added cache-probe time
  widens the historical aggregate boundary.
- `ownershipLookupUs` remains independent and retains its existing meaning.
- New fields serialize as `fileMetadataUs`, `existingAssetLookupUs`, and
  `cacheProbeUs`; old payload fields and scan behavior remain compatible.
- Reason counts use `coldFiles`, `reanalyzedFiles`, and the existing
  `skippedFiles` field.
- Focused tests demonstrate nonzero stage timings for exercised stages,
  correct reason counts where exposed, and cancellation/restart after one
  committed database batch.
- A bounded release benchmark records the new stage splits for all three passes
  using only the preserved synthetic 0068 fixture(s) and fresh output data.
- Test-owned source fixtures are unchanged after scanning, no original files
  are written, and no full-resolution source pixel buffer enters the pipeline.
- No lookup-query optimization is included in this phase.

## Risks and limitations

- Per-stage counters are accumulated elapsed time and may overlap across
  concurrent scan workers; they are diagnostic work totals, not wall time.
- Very fast stages can report zero microseconds at clock resolution. Tests
  should verify accounting relationships and stage attribution without relying
  on brittle minimum durations where practical.
- If the current scanner cannot distinguish cold imports from reanalysis
  without changing its state model, omit those reason counters and document
  the limitation rather than inferring a reason from incomplete data.

## Bounded release benchmark

Run on 2026-09-27 using the generated 1,000-image, 320-pixel source fixture at
`test-data/.tmp/import-opt-20260924-68b40f5e/run-1000x320/source-fixtures`.
The fixture directory was read-only for this run; file count and SHA-256 hashes
matched before and after (1,000 files). Each benchmark pass used a fresh
application data directory beneath
`test-data/.tmp/import-rescan-diagnostics-20260924-74bd1fce/run-1000x320-20260927`.

Built from `src-tauri` with the absolute Cargo executable and
`CARGO_TARGET_DIR` set to
`test-data/.tmp/import-rescan-diagnostics-20260924-74bd1fce/cargo-target`:
`cargo build --locked --offline --release --no-default-features --bin import-benchmark`.
The build completed successfully in 1m04s. The resulting executable was
2,836,480 bytes, last written at 2026-09-27 18:24:55 local time.

| Pass        |    Wall ms | Processed / succeeded / skipped | Cold / reanalyzed | Failed | `fileMetadataUs` | `existingAssetLookupUs` | `cacheProbeUs` | `metadataLookupUs` | `ownershipLookupUs` |
| ----------- | ---------: | ------------------------------: | ----------------: | -----: | ---------------: | ----------------------: | -------------: | -----------------: | ------------------: |
| Cold        | 15712.2661 |                 1000 / 1000 / 0 |          1000 / 0 |      0 |            72117 |                 2792356 |              0 |            2869277 |                9364 |
| Cache reuse | 12699.4788 |                 1000 / 1000 / 0 |          0 / 1000 |      0 |            61789 |                 2768302 |          71483 |            2905796 |               11264 |
| Warm        |  3786.6409 |                    0 / 0 / 1000 |             0 / 0 |      0 |            65916 |                 2902685 |          76261 |            3050426 |                9505 |

Existing-asset lookup dominates these totals at 2.77–2.90 s per pass, while
file metadata collection takes 0.062–0.072 s.

For each pass, `metadataLookupUs` is at least the sum of the three split
counters; the remaining aggregate time includes identity-key and other lookup
work. `ownershipLookupUs` remains separate. The executable report is preserved
at `test-data/.tmp/import-rescan-diagnostics-20260924-74bd1fce/run-1000x320-20260927/report.json`.
Peak working set sampled at 100 ms intervals was 41,496,576 bytes (39.6 MiB).

The optional 128-image by 1280-pixel fixture was not run; this bounded result
covers only the 1,000-image by 320-pixel fixture. Timing and sampled working
set are specific to this machine and run. The historic phase-2
`metadataLookupUs` numbers are not directly comparable because this phase adds
cache-probe time to that aggregate. No source fixture was modified and all
three passes completed with zero failed files.

Frontend follow-up (per frontend agent report): the panel displays the three
timing splits and cold/reanalyzed/skipped counts, remains compatible with
legacy progress payloads, and its 12 component tests passed.
