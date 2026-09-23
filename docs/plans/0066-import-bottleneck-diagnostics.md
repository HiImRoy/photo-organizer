# Import bottleneck diagnostics

## Goal

Expose the existing cumulative `ScanProgress.performance` measurements in the
background scan details so a slow import can be attributed to discovery,
metadata/ownership lookups, source reads and fingerprinting, image processing,
or database writes. Clearly label these as overlapping accumulated worker-stage
times, never as wall-clock time, and do not reveal source paths.

## Execution plan

1. Verify the Rust and TypeScript performance fields have matching semantics;
   avoid changing the serialized IPC shape unless an actual missing measurement
   prevents diagnosis.
2. Address the existing successful-scan auto-dismiss in `App.tsx`: it clears
   terminal progress after 700 ms, before users can inspect the measurements.
   Retain the latest terminal scan and its diagnostics until the user closes the
   existing status control or a new scan replaces it. Verify that the close
   action is clearly labeled and that a subsequent task remains visible. The
   user authorized edits to `App.tsx` and `App.test.tsx` for this behavior. Do
   not add a separate history store or alter task persistence.
3. Add a compact diagnostics block in scan task details, visible while running
   and while terminal progress is retained. Handle absent/zero legacy performance
   data quietly. Separate parent totals from nested image stages so the UI does
   not double-count. The diagnostic does not include `currentPath` or any other
   source path.
4. Add focused component coverage for missing fields, stage formatting, and
   nested/overlapping time labels; run targeted Vitest, formatting, and TS
   lint/typecheck per the local risk matrix.
5. Run the existing import benchmark using a bounded synthetic fixture and an
   explicit `--data-dir test-data/.tmp/<unique-name>` output. First confirm that
   exact directory does not exist. The benchmark's generated source, app data,
   database, and thumbnails must stay under it. Capture its JSON report from
   stdout in this plan rather than creating an unbounded output location.
   Record cold, cache-reuse, and warm wall times plus principal counters,
   fixture bounds, and the limitation that synthetic timings do not establish
   the user's real-library bottleneck.

## Safety and acceptance

- Never access personal photo directories; benchmark inputs and all generated
  outputs must stay under `test-data/`. Use a unique path whose non-existence
  was checked before running; cleanup, if needed, may target only that exact
  newly created path. Otherwise preserve it and report its location.
- Successful-scan performance details are not observable until terminal
  progress remains available for inspection and the close/replacement behavior
  is covered by a regression test.
- Keep the thumbnail-only processing boundary intact.
- Main-agent review and commit are outside this execution scope.

## Results

- The existing `ScanProgress.performance` wire shape is sufficient; no Rust or
  TypeScript schema changes were needed. Diagnostics show discovery,
  metadata/ownership lookup, file fingerprinting, image-processing total with
  thumbnail-decode and feature-analysis children, and database writes. The
  panel explicitly says these are cumulative worker-stage times that may
  overlap and are not wall-clock duration. Missing `performance` is silently
  omitted, and diagnostics contain no source path.
- Successful terminal progress now stays available until dismissed or replaced
  by a later scan. The close action has an explicit accessible name and tooltip.
- The rapidly updating diagnostics group explicitly sets `aria-live="off"` to
  suppress inherited polite announcements. Its named group and text remain in
  the accessibility tree for deliberate navigation and reading; the component
  test checks both the off setting and the readable metrics.
- Targeted component suite: 11/11 passed. The two App retention/dismissal tests
  passed. In the full affected test-file run, one unrelated existing library
  drag-and-drop test failed once and passed when isolated and rerun; all other
  tests passed.
- Changed-file Prettier, ESLint, TypeScript typecheck, and `git diff --check`
  passed. No full frontend suite or production build was run.
- Isolated benchmark in Cargo's unoptimized `dev` profile: 16 generated JPEGs,
  each 1280×960 at quality 85, created
  under the previously absent `test-data/.tmp/import-bottleneck-20260923-c9e7a54b`.
  This 16-image sample only validates the measurement path; debug timings are
  not comparable with the desktop release build or historical release
  benchmarks. No release rebuild was attempted.
  Cold: 3633.2 ms wall, image processing 6720.5 ms cumulative, thumbnail decode
  161.3 ms, feature analysis 3622.0 ms, fingerprint 11.2 ms, metadata lookup
  126.5 ms, database writes 30.3 ms. Cache reuse: 3387.3 ms wall, image
  processing 6184.3 ms cumulative, thumbnail decode 2493.2 ms, feature analysis
  3651.3 ms, fingerprint 9.9 ms, metadata lookup 137.4 ms, database writes
  27.1 ms. Warm: 251.0 ms wall, 16 skipped, no fingerprint or image processing,
  metadata lookup 131.4 ms, database writes 12.5 ms. The cumulative image stage
  exceeds wall time because parallel workers overlap. These synthetic local
  fixture timings do not identify the user's photo-library bottleneck.
- Browser visual inspection could not be completed: the Playwright CLI package
  fetch was denied by the environment's network restriction (`EACCES`), and the
  available Windows computer-use skill requires a `node_repl` interface that is
  not exposed in this task. The exact benchmark output directory was removed
  after recording results; it had been confirmed absent before the run.
