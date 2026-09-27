# Large-library stress baseline (phase 0071)

## Goal and scope

**Execution status:** the mixed-format stress baseline was completed with the
dedicated Rust benchmark binary after a preliminary JPEG-only run. The runs
used separate fixture roots; a first mixed-format attempt failed its thumbnail
uniqueness assertion. Their roots and results are distinguished below. No
production scanner, database, IPC, frontend, dependency, or architecture
modules were changed.

## Run design

- Generate exactly 3,000 JPEG, PNG, and WebP images with 320×240 synthetic
  pixels in nested Chinese, Cyrillic, and spaced paths.
- Keep fixtures, isolated app data, reports, and the Cargo target in the
  corresponding checked-absent root beneath `test-data/.tmp`. The preliminary
  JPEG-only run used
  `large-library-stress-20260927-4f7a9c31`; the successful mixed-format retry
  used `large-library-stress-mixed-retry-20260927-c1726f2e`. The failed first
  mixed-format attempt used a separate root, documented below. Never use
  personal photos or write to another fixture.
- Record cold import, cancellation after the first committed 16-file batch,
  restart/resume, warm rescan, a paged filtered/sorted query, and library index
  removal. At the cancel callback, confirm 16 rows are query-visible before
  setting the cancellation flag.
- Label removal duration as database/index reconciliation only. The separate
  IPC thumbnail and preview cache cleanup is not called or timed by this
  standalone harness.
- Hash every synthetic source file before and after the run; require identical
  manifests and expected import/query/removal counts. Sample process working
  set externally where practical.
- Run relevant Rust tests, formatting, Clippy, and a locked/offline release
  build. Keep build artifacts and run reports under each run's root.

## Intended limits

Import, rescan, query, and index-removal latency are backend proxies. This
harness cannot establish desktop UI responsiveness or visually validate the
application; manual desktop acceptance is still needed for that claim. Report
cache cleanup as unmeasured and call out any unavailable checks or environment
blockers.

## Executed immediate baseline (2026-09-27)

Scope pivot: the immediate run used the existing read-only release executable;
no new benchmark code was executed.

Executable: `test-data/.tmp/import-lookup-opt-20260927-6c98d72a/cargo-target/release/import-benchmark.exe`

Arguments: `--data-dir test-data/.tmp/large-library-stress-20260927-4f7a9c31/run-3000 --generate 3000 --width 320 --height 240 --quality 90`

Exit code 0; stderr empty. The full process took 226.961 s including fixture
generation. Sampled peak working set was 46,268,416 bytes (44.12 MiB) over
1,984 samples. These are total-process metrics, distinct from scan wall times.

| Pass        | Discovered | Succeeded | Skipped | Failed | Scan wall |
| ----------- | ---------: | --------: | ------: | -----: | --------: |
| Cold        |      3,000 |     3,000 |       0 |      0 |  35.945 s |
| Cache reuse |      3,000 |     3,000 |       0 |      0 |  29.080 s |
| Warm        |      3,000 |         0 |   3,000 |      0 |   1.363 s |

All passes completed with zero missing files. Captured output is in
`test-data/.tmp/large-library-stress-20260927-4f7a9c31/benchmark.stdout.json`,
`benchmark.stderr.txt`, and `benchmark.metrics.json`.

The post-run SHA-256 manifest
`test-data/.tmp/large-library-stress-20260927-4f7a9c31/source-fixtures.sha256.json`
contains 3,000 source files; a subsequent hash check matched the manifest.
The generated set was JPEG-only, flat `generated-*.jpg` names, with 256 unique
hashes because the synthetic pattern repeats every 256 seeds. There was no
pre-scan manifest, so source immutability is not proven by before/after hashes.
Current source-path review gives limited read-only rationale: the generator
creates fixtures before the three `scan_library` calls; scans read/fingerprint
the source root and write derived thumbnails to the separate app-data cache,
with database updates in isolated app data. This is not a substitute for a
pre-run hash, and the executable was not matched to a build of the current
source tree. No separate rescan was run.

Read-only frontend audit: the reported targeted run passed 16 tests. While
removal of A is pending, switching to C can reset selection to B. No frontend
code was changed and those tests were not rerun here.

## Mixed-format stress retry (2026-09-28)

This retry is separate from the JPEG-only executable baseline above. The first
mixed-format attempt used
`test-data/.tmp/large-library-stress-mixed-20260927-7de4912a`: it generated
3,000 files, but the synthetic seed repeated every 256 images, yielding only
768 unique thumbnails. The uniqueness assertion failed, so that attempt has no
completed report. Its generated data was preserved.

The successful retry used
`test-data/.tmp/large-library-stress-mixed-retry-20260927-c1726f2e`. It scanned
3,000 images in nested paths containing Chinese and Cyrillic text and spaces:
1,000 JPEG, 1,000 PNG, and 1,000 WebP. The report and sampled working-set data
are in `report.json` and `working-set.csv` under that root.

| Operation             | Result                                                                |
| --------------------- | --------------------------------------------------------------------- |
| Cold import           | 35.903 s; 3,000 succeeded, 0 failed                                   |
| Cancellation boundary | 16 processed; all 16 were visible in the database before cancellation |
| Resume                | 35.769 s; 2,984 succeeded, 16 skipped, 0 failed                       |
| Warm rescan           | 1.563 s; 3,000 skipped, 0 failed                                      |
| Filtered query        | 18.691 ms; 1,000 total matches, 50 returned on the measured page      |
| Library index removal | 245.667 ms for database/index reconciliation; 3,000 assets removed    |

The run produced 3,000 thumbnails with a maximum edge of 320 pixels. The
before and after source manifests both contained 3,000 files and had SHA-256
`38cc43cd76f61a6bd2b234ba7ba87d13b223aa847d0507480dfbed73e0183773`; the
report marks the fixtures unchanged. Sampled peak working set was 52,432,896
bytes (50.0 MiB) across 474 samples. All reported scan passes had zero failed
files. The Rust binary tests, formatting check, Clippy, and locked/offline
release build passed.

This benchmark is a backend latency proxy. Index removal measures database
and index reconciliation only; full IPC thumbnail and preview cache cleanup
was not measured. It also does not measure actual desktop scrolling or filter
smoothness, which still requires desktop acceptance. A separate frontend audit
passed 16 targeted tests and found an unresolved selection issue: while
removal of library A is pending, selecting library C can reset selection to B.
No fix has been made for that issue.
