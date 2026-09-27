# Import rescan lookup optimization (phase 4)

## Goal and assumptions

Reduce rescan time by reusing SQLite resources for existing-asset lookups. The
completed 0069 diagnostics measured this query at approximately 2.77–2.90 s per
1,000 images, compared with 0.06–0.07 s for file metadata and approximately
0.07 s for warm cache-path probes. This is the measured target for this phase.

Keep the existing lookup SQL and result semantics, including its global
`asset_identity_key` match and thumbnail/feature joins. Keep
`Repository::find_existing_asset` available to other callers. A scan may reuse
one SQLite read connection and a cached prepared statement, but every lookup
must run as its own autocommit statement: do not retain a read transaction or
cache asset results in memory. Separate connections must continue to see
committed scanner writes and other writers' commits between batches. This also
preserves rescan restart behavior and cross-library asset ownership.

The scanner's thumbnail-only boundary, cancellation flow, write batching, and
all unrelated APIs remain unchanged. Do not modify `models.rs`, frontend files,
the 0069 diagnostic document or its output root, fixtures, or the prior
benchmark executable. The worktree already contains unrelated changes in
`scanner.rs`, `models.rs`, and frontend files; preserve them. Do not commit.

## Owned paths and benchmark inputs

- Code: `src-tauri/src/db.rs` and `src-tauri/src/scanner.rs`.
- This execution plan: `docs/plans/0070-import-rescan-lookup-optimization.md`.
- New generated/test/benchmark output root:
  `test-data/.tmp/import-lookup-opt-20260927-6c98d72a`.
- Read-only benchmark source fixture:
  `test-data/.tmp/import-opt-20260924-68b40f5e/run-1000x320/source-fixtures`.
- Read-only prior release executable:
  `test-data/.tmp/import-rescan-diagnostics-20260924-74bd1fce/cargo-target/release/import-benchmark.exe`.

The new output root was checked absent before this plan was created. Create
only that root for this phase's generated data, test-owned files, build target,
and reports. Keep the fixture source and old executable read-only; record their
before/after SHA-256 checks. Keep tests within the new root. Never access
personal photo directories.

The prior executable was present before implementation and had SHA-256
`B92448CF4C24027B22DBF4E71D6BE9C8B8AFBADA4728B0C274A500FA93324753`.
Recheck it before and after all old-version runs.

## Implementation steps

1. Add an `ExistingAssetLookupSession` in `db.rs` that owns one configured
   SQLite connection. Use `prepare_cached` for the current existing-asset
   query, dropping each statement guard after the row lookup so SQLite can end
   the implicit read transaction. Preserve the existing repository method and
   query shape.
2. Create one session for each public scan invocation and pass it through its
   sequential library scopes. Route only existing-asset reads through it;
   writes, ownership lookup, cache checks, progress, cancellation, and
   thumbnail processing keep their present paths. Include session setup in the
   existing lookup timing once per scan, then measure each statement as before.
3. Add focused Rust coverage for equivalence with the existing lookup result,
   a committed insert/update becoming visible on a reused session between
   lookups, Unicode path identity keys, and interruption after a committed
   batch followed by a successful restart. Preserve source fixture bytes.
4. Run the focused Rust tests, `cargo fmt --all -- --check`, and the requested
   offline/no-default-features Clippy command. Run `git diff --check` and review
   the full assigned-file diff without reverting or editing unrelated changes.
5. Benchmark the old and new release executables on the same read-only 1,000 ×
   320×240 JPEG source fixture, using three independent fresh app-data
   directories per executable. The new build must use the same flags as 0069:

   ```powershell
   cargo build --locked --offline --release --no-default-features --bin import-benchmark
   ```

   Keep the new Cargo target and all run outputs beneath the owned output root.
   Each executable run uses `--images <fixture> --data-dir <fresh-run-dir>` and
   reports cold import, cache reuse, and warm rescan. Sample process working
   set if practical. Record medians for wall time and the split lookup,
   metadata, cache, and ownership counters; verify expected counts and zero
   failures. Hash the source fixture before and after. Never write to the 0069
   root or rebuild/replace its old executable.
6. Update this plan with implementation details, test/check results, benchmark
   data, memory, and limitations.

## Acceptance criteria

- Existing lookup SQL semantics and the public `find_existing_asset` API are
  preserved.
- The scan reuses a read connection and cached statement without a long-lived
  read transaction; writes committed between lookup batches are visible.
- Cancellation/restart, cross-library lookup, cache probes, and bounded
  thumbnail processing retain their existing behavior.
- Focused Rust tests, formatting, and Clippy pass, or concrete failures and
  limitations are reported.
- Three old and three new release runs use the same read-only fixture and
  fresh app-data; all report zero failures and expected pass counts.
- The old executable and fixture hashes are unchanged. All generated outputs
  remain in the new owned root, and the final assigned-file diff is reviewed.

## Implementation results

Added `Repository::existing_asset_lookup_session`, which owns one configured
SQLite connection and calls `prepare_cached` for the unchanged global identity
query. The statement guard is released after each row lookup, so each call
ends its implicit read transaction and subsequent calls see committed writes.
The existing `Repository::find_existing_asset` method remains available and
uses the same SQL and result mapping.

Each public scan invocation creates one session and reuses it through its
sequential scope(s), including all scopes in a tree scan. The scanner still
performs a fresh lookup for each discovered image; it does not memoize rows or
filter by library ID. Connection setup is counted once in
`existingAssetLookupUs` and `metadataLookupUs`; per-image cached-statement
execution remains in the existing lookup counter. No schema, IPC, dependency,
thumbnail-processing, or cancellation-flow changes were made in this phase.

### Existing cancellation-flow review

The cancellation checks already present in the working-tree diagnostics change
were preserved. They are correct and necessary at two callback boundaries:
`flush_pending_image_work` rechecks cancellation after publishing its forced
progress update so it does not start a queued image batch after the callback
requests cancellation; after the final pending-work flush, the scope rechecks
the flag and enters `cancel_scan_scope` rather than completing the library and
reconciling unvisited files as missing. `cancel_scan_scope` persists completed
results and seen-asset writes before marking the job cancelled.

The duplicate restart test added during this phase was removed because the
existing `scanner::tests::interrupted_scan_can_restart_and_finish_without_changing_sources`
test exercises the same committed-batch and restart path through the new lookup
session. It passed in the final all-target run: it cancels after a committed
16-result batch, verifies the restart skips those 16 and imports the remaining
18, and checks that source bytes are unchanged. The distinct database test
verifies API/session lookup equivalence, Unicode identity keys, and visibility
of committed inserts and updates through one reused session. The existing
`scanner::tests::cancellation_does_not_mark_unvisited_assets_missing` test also
passed. The forced-progress cancellation branches were reviewed and left
unchanged; there is no isolated test specifically for cancellation requested
during the final partial-batch progress callback.

## Verification

- Focused test command: `cargo test --manifest-path src-tauri/Cargo.toml
  --locked --offline --no-default-features --lib
  existing_asset_lookup_session` — 1 passed.
- Full Rust boundary check:
  `cargo test --manifest-path src-tauri/Cargo.toml --locked --offline
  --no-default-features --all-targets` — 157 library tests, 1 semantic
  benchmark test, and 3 semantic-evaluate tests passed; import-benchmark had
  no unit tests. Total: 161 passed, 0 failed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` — passed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline
  --no-default-features --lib --tests` — passed without warnings.
- `git diff --check` — passed.

## Release benchmark

Built the new comparator with the 0069 command and profile:
`cargo build --locked --offline --release --no-default-features --bin
import-benchmark`. Build time was 68.8 s. The new executable is
`test-data/.tmp/import-lookup-opt-20260927-6c98d72a/cargo-target/release/import-benchmark.exe`
(2,844,160 bytes; SHA-256
`5C21D689F124042B66A037D274B1B43894ECBB79ECD5710DF98E55A43165D80F`).

Ran three old and three new invocations against the same read-only 1,000 ×
320×240 JPEG fixture. Each invocation used fresh app-data, reported cold
import, cache reuse, and warm rescan, and was sampled for working set at
100 ms intervals. All six invocations discovered 1,000 files in every pass;
cold and cache reuse each succeeded on 1,000 with zero failures, while warm
skipped 1,000 with zero failures.

Medians across the three runs (stage counters are cumulative elapsed work and
are not additive wall-clock components):

| Version | Pass | Wall ms | `fileMetadataUs` | `existingAssetLookupUs` | `cacheProbeUs` | `metadataLookupUs` | `ownershipLookupUs` |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Old | Cold | 17,294.37 | 86,597 | 3,016,324 | 0 | 3,108,363 | 11,870 |
| Old | Cache reuse | 14,488.21 | 71,656 | 2,987,449 | 83,588 | 3,144,515 | 10,912 |
| Old | Warm | 3,955.99 | 70,071 | 3,011,907 | 83,657 | 3,170,710 | 10,426 |
| New | Cold | 11,781.62 | 51,212 | 8,758 | 0 | 64,545 | 3,728 |
| New | Cache reuse | 9,388.77 | 49,138 | 18,388 | 49,810 | 122,878 | 4,502 |
| New | Warm | 405.01 | 45,268 | 23,016 | 46,519 | 118,402 | 4,228 |

Median wall-time reductions were 31.88% cold, 35.20% cache reuse, and 89.76%
warm. Existing-asset lookup time fell by 99.71%, 99.38%, and 99.24%
respectively. On the new build, the lookup stage includes one connection setup
per scan. Median sampled peak working set was 41,254,912 bytes old and
40,722,432 bytes new (about 1.3% lower, within measurement noise).

The fixture contained 1,000 files / 11,668,827 bytes. Its sorted per-file
SHA-256 manifest digest was
`DE8E7194FF7359A551AB84047A6BA92CDE026EA7D70787394CB12527C66E3BCE` before and
after measurement. The old executable remained 2,836,480 bytes with SHA-256
`B92448CF4C24027B22DBF4E71D6BE9C8B8AFBADA4728B0C274A500FA93324753` before
and after its runs. All six JSON reports and the new executable/build outputs
are under the new owned output root. The old executable remains in its 0069
root and was used read-only; no source fixture or 0069 output was modified.

## Final review and limitations

The optimization exceeds the 10% median target for cold and cache-reuse wall
time and does not regress warm time. The scan performs no source writes and
keeps analysis on its existing bounded-thumbnail path; the all-target suite's
high-resolution bounded-thumbnail test passed. Timing and working-set results
are specific to this machine; per-stage counters can overlap across workers.
No 128-image/high-resolution cohort was run. The pre-existing worktree changes
in `models.rs`, frontend files, and the 0069 plan were preserved. No commit was
created.
