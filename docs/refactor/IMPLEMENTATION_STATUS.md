# Refactor Implementation Status

本文件是 Checkpoint A-F 的唯一阶段状态记录。每个 Checkpoint 完成后，在对应 commit 创建前后补齐记录；未完成的阶段不得填写为完成。

## Checkpoint A — Source Boundary + Nested Library

Status: BLOCKED_FOR_REVIEW

Commit: checkpoint A implementation snapshot plus verification fixes (see `git log`)

Migration: 0005 Library Source Identity and Hierarchy; 0006 Global Asset Identity and Ownership

Automated tests:

- Passed: `npm run format:check`
- Passed: `npm run lint`
- Passed: `npm run typecheck`
- Passed: `npm test` (10 tests)
- Passed: `npm run build`
- Passed: `npm run test:rust` (38 tests; Rust toolchain on PATH for the verification shell)
- Passed: `npm run clippy` (`-D warnings`)
- Passed: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- Passed: `npm run tauri -- info` (WebView2, MSVC, Rust, Cargo, rustup)
- Passed: `npm run tauri -- build --debug` (EXE, MSI, and NSIS bundles)
- Passed: import performance regression tests (shared 640px cache, bounded resize,
  full fingerprint reuse for small sources)
- Passed: import responsiveness safeguards (coalesced scan progress, batched job
  progress persistence, rebuildable thumbnail cache writes, throttled frontend refresh)

Manual verification: Not completed; the interactive Parent/Child/Grandchild desktop flow has not been run. The repository currently contains no tracked synthetic image fixtures beyond `test-data/README.md`; automated Rust integration tests cover the source-boundary and Parent/Child ownership flow.

Known issues:

- Checkpoint A remains blocked for review until the Manual Verification list in `checkpoint-a-library-safety.md` is completed in the desktop application.
- Import performance optimization Phases 1 and 2 are implemented in
  `docs/plans/0005-import-performance.md`; Release import speed was manually
  accepted, and the temporary profiling logs were removed after diagnosis.
- Preview generation now runs off the UI command path. Single-image preview requests
  the original source first and falls back to a bounded screen preview if the source
  cannot be read; the fallback uses a 1920x1200 default bound, a 15-second UI timeout,
  and a fixed centered canvas. The old floating zoom bar is replaced by a Lightroom-style
  Navigator in the right Information panel with a larger viewport frame, click/drag navigation,
  and a read-only current zoom ratio; the main canvas handles zooming and only displays the image.
- The post-clarification Library import/hierarchy milestone is implemented behind
  migration 0007: import can expose direct image-containing subfolders as
  independent Libraries, and drag/drop or the root-drop action persists a manual
  `parentLibraryId` with self/descendant cycle validation. This milestone does not
  change Checkpoint A's manual-review status or start Checkpoint B.
- The installed Rust toolchain is available at `C:\Users\666\.cargo\bin`; a new terminal should be opened if an existing shell does not yet include it in PATH.
- Checkpoint B 已实现并等待桌面端人工验证；Checkpoint C–F 仍为 `NOT_STARTED`。

## Checkpoint B — Manual Classification + Effective Filter

Status: IMPLEMENTED_PENDING_MANUAL

Commit: Checkpoint B implementation snapshot plus automated verification (see `git log`)

Migration: 0009 Manual Classification Overrides

Automated tests:

- Passed: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- Passed: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- Passed: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --all-features` (45 library tests + semantic benchmark/evaluation tests)
- Passed: `npm run format:check`
- Passed: `npm run lint`
- Passed: `npm run typecheck -- --pretty false`
- Passed: `npm test -- --run` (19 tests)
- Passed: `npm run build`

Manual verification:

Pending: DetailPanel single-asset editor, Auxiliary Tag ADD/REMOVE, Restore Auto, batch editor, Effective filters, FAILED versus UNKNOWN, reanalysis preservation, and source-file integrity.

Known issues:

- B cannot be marked `COMPLETED` until the manual checklist in `checkpoint-b-classification-filter.md` is run in the desktop app.
- The classification editor is collapsed by default, uses Chinese labels and select-only controls, and no longer accepts raw classification IDs as text input.
- Checkpoint A remains `BLOCKED_FOR_REVIEW`; this implementation does not silently change A's status.
- Objective Numeric Feature remains read-only and outside the Derived Classification Registry.

## Checkpoint C — Preview

Status: NOT_STARTED

Commit:

Migration:

Automated tests:

Manual verification:

Known issues:

## Checkpoint D — Semantic + Dominant Color

Status: NOT_STARTED

Commit:

Migration:

Automated tests:

Manual verification:

Known issues:

## Checkpoint E — Export Preview

Status: NOT_STARTED

Commit:

Migration:

Automated tests:

Manual verification:

Known issues:

## Checkpoint F — COPY Export

Status: NOT_STARTED

Commit:

Migration:

Automated tests:

Manual verification:

Known issues:

## Status Rules

- `IMPLEMENTED_PENDING_MANUAL` 表示自动化 Exit Criteria 已完成，但桌面端人工验证尚未完成；不能等同于 `COMPLETED`。
- 只有在对应文档的全部 Exit Criteria 通过后，Status 才能改为 COMPLETED。
- 如果实现中发现 Architecture Plan 与真实代码冲突，Status 保持 BLOCKED_FOR_REVIEW，并记录冲突，不得自行绕过。
- 每个 Checkpoint 完成后必须创建独立 commit，然后停止。
- 不得在同一个 commit 中混入下一个 Checkpoint 的业务代码。
- Rollback 不属于 Checkpoint F。
