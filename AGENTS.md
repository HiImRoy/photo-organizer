# Project Instructions

## Project goal

Build a local-first desktop photo management application.

The application scans a user-selected directory recursively, analyzes images,
stores virtual classifications in a local database, previews and sorts images,
and generates safe file organization plans.

The application must not modify the original directory during normal browsing
or analysis.

## Product principles

1. Classification results and physical directory structure must remain separate.
2. A single image may have multiple labels and continuous feature values.
3. All destructive or filesystem-changing actions require a preview.
4. Copy is the default organization operation.
5. Moving, renaming, overwriting, and rollback require explicit safety checks.
6. The application should work locally without requiring a cloud service.
7. Do not add accounts, cloud synchronization, or a remote backend unless requested.
8. Do not silently change architecture or add production dependencies.
9. Import, thumbnail generation, image decoding for analysis, feature extraction,
   and model inference must use application-owned thumbnails or bounded thumbnail
   derivatives. The original file may only be touched for filesystem metadata,
   fingerprinting, EXIF/embedded-preview metadata, and a controlled decoder path
   that emits the requested thumbnail size; a full-resolution source pixel buffer
   must never enter the import, analysis, or model pipeline.
10. Adjacent controls in the same action group must use one shared control
    specification. They must have the same height, baseline, padding rhythm,
    border thickness, corner radius, and spacing; primary/secondary variants
    may differ in color or emphasis only. A square button next to a rounded
    button, or buttons with visibly different heights or vertical alignment,
    is a UI defect and must not be introduced.

### UI control-group consistency invariant

This is a project-wide visual contract, not a preference for a single button
style. Every adjacent button group (dialog footers, toolbar actions, filter
controls, pagination and card actions) must use a shared component class or
shared tokenized variant for geometry. Before adding a new action, check the
computed height, border radius, border, line-height, and flex alignment against
its neighboring actions in both dark and light themes. Visual regression checks
must include mixed primary/secondary groups and narrow desktop widths.
The left navigation rail is a structural layout boundary, not a third card:
its outer shell must remain transparent and borderless when the direct
“图库” and “筛选” modules already provide their own single silhouettes.

Visible buttons must use the shared rounded control geometry (`--control-radius`)
and must not use square corners (`border-radius: 0`). This includes icon-only
buttons, close buttons, toolbar actions, filter controls, pagination, card
actions, workflow actions, and dialog footers. Circular selection/color
indicators are the only intentional exception and must be explicitly styled as
circular states rather than square buttons.

### Thumbnail-only processing invariant

This is a cross-checkpoint project rule, not an optimization that may be
relaxed for convenience. Every import, analysis, and decode path must consume
an application-owned thumbnail or a bounded thumbnail derivative. A source
image may be opened only to read metadata/fingerprint data or to ask a bounded
decoder for the target thumbnail; it must not be fully decoded, resized from a
full-resolution pixel buffer, or passed to a feature extractor or model. The
only explicit exception is a user-initiated `original` preview in the current
viewer, which is never reused by import, analysis, or model inference.

Implementation review must be able to identify the thumbnail input at every
import/analysis/decode boundary. Passing an original-image path to a feature
extractor, preprocessor, model, batch retry, or recovery task is a violation;
tests must cover the boundary with a high-resolution fixture and verify that no
full-resolution source pixel buffer is created.

## Tentative architecture

- Desktop shell: Tauri
- Frontend: React and TypeScript
- Image analysis: Python
- Image processing: OpenCV, Pillow, NumPy
- Metadata: ExifTool
- AI inference: ONNX Runtime
- Local database: SQLite
- Communication: typed IPC or a packaged local sidecar

This architecture is tentative. Significant changes require an ADR in
`docs/decisions/` explaining alternatives, costs, and migration impact.

## Engineering workflow

For complex features, create or update an execution plan under `docs/plans/`
before implementation.

Each implementation task must:

1. Inspect existing code and documentation.
2. State assumptions.
3. Keep the change limited to the current milestone.
4. Add or update tests for behavior changes. Documentation- or comment-only
   changes do not require new tests or code-test runs.
5. Select verification from the risk-based matrix below; do not treat every
   check as mandatory for every change.
6. Review the final diff.
7. Update relevant documentation.
8. Report checks that were not run and unresolved risks honestly.

### Risk-based verification

Use the smallest verification set that covers the changed behavior, and expand
it when a change crosses shared boundaries or raises regression risk:

- Documentation- or comment-only changes: check formatting on changed
  documents and run `git diff --check`. No new tests, application tests, lint,
  type checks, or builds are required unless the change also alters executable
  code or a generated/configuration contract.
- Isolated CSS or a small UI component: check formatting on changed files and
  run the related Vitest suite when component behavior or DOM contracts change.
  For visual changes, inspect the affected view at the relevant theme and
  viewport sizes. A local fix does not automatically require every viewport in
  the full visual matrix.
- Shared TypeScript logic: check changed-file formatting, lint and typecheck,
  and run the affected tests. Include every known consumer when shared behavior
  changes.
- Rust, IPC, database, import, or analysis changes: run the relevant Rust tests
  and static checks; include affected frontend tests when an IPC or user-facing
  contract is involved. Keep filesystem tests within `test-data/` or test-owned
  temporary directories.
- Startup or packaging changes: preserve the manual launch contract below and
  verify `scripts/manual-build-start.ps1 -CheckOnly` and the frontend build.
- Escalate frontend changes to the full frontend suite when shared UI/state or
  cross-feature behavior makes targeted coverage insufficient. Escalate Rust,
  IPC, or database changes to all-target Rust tests when shared contracts,
  migrations, or safety boundaries are involved. Run full static checks/builds
  for cross-module, dependency/configuration, startup/packaging changes, or
  when the affected area cannot be isolated confidently. CI and release
  validation retain their full required checks; this local matrix does not
  reduce them.

When a check fails, first isolate and rerun the failing test or command to
distinguish a reproducible failure from an intermittent or environment issue.
Do not hide a failure by disabling or weakening its test, suppressing its
output, or repeatedly rerunning until it passes. Fix a reproducible failure or
report it with the evidence. Always identify checks that were not run.

### Manual launch contract

`启动 PhotoOrganizer.cmd` is the canonical manual build-and-launch entry for
desktop acceptance testing. It must remain usable from a fresh Windows terminal.
Any change to Node/Rust prerequisites, package scripts, Tauri configuration,
startup environment variables, WebView2 workarounds, data paths, bundled model
resources, or build/start commands must update this entry, its implementation in
`scripts/manual-build-start.ps1`, and the README instructions in the same task.

Changes that affect startup or packaging are not complete until
`scripts/manual-build-start.ps1 -CheckOnly` and the frontend build have been
verified. Do not replace this entry with a browser-only development command.

## Safety constraints

- Never operate on personal photo directories during development.
- Use only fixtures under `test-data/` for filesystem tests.
- Never overwrite an existing target file.
- Never delete original files.
- Do not implement permanent deletion in the MVP.
- File operations must support dry-run mode.
- File operations must be logged before execution.
- Paths containing Chinese, Cyrillic, spaces, and Unicode must be tested.
- Metadata must not be written back to original images in the MVP.

## MVP scope

The MVP includes:

- Select a root directory.
- Recursively scan JPEG, PNG, and WebP files.
- Read basic metadata.
- Generate and cache thumbnails.
- Store assets in SQLite.
- Extract brightness, contrast, saturation, and dominant-color features.
- Classify broad semantic categories such as portrait, landscape, product,
  architecture, animal, food, screenshot, document, and unknown.
- Filter, group, and sort by metadata and extracted features.
- Preview target folder structures and generated filenames.
- Copy selected files into a new directory.
- Detect naming conflicts.
- Record operations and support rollback of generated copies.

The MVP excludes:

- Face identity recognition.
- Cloud synchronization.
- Video.
- Permanent deletion.
- Automatic modification of originals.
- Full professional RAW development.
- Model training.
- Subscription or account systems.

## Definition of done

A task is complete only when:

- Required behavior is implemented.
- Verification appropriate to the change has passed: behavior changes include
  relevant tests; documentation/comment-only changes need no code tests; lint,
  type checks, visual checks, and builds are required when the matrix says they
  cover the changed area.
- No original test fixture is unexpectedly modified.
- User-visible behavior is documented.
- The final diff has been reviewed.
- Any skipped checks and unresolved failures are reported honestly.

## Agent delegation and ownership

- For all subsequent code-development work and trivial execution tasks, the
  main agent must delegate the actual implementation or execution to a
  subagent configured with model `gpt-6-luna` and `reasoning_effort=max`.
- The main agent remains responsible for planning, scope and boundary
  definition, review, integration, and final acceptance/verification.
  Delegation does not transfer accountability.
- Before parallel work starts, explicitly assign file ownership and
  non-overlapping responsibilities to each subagent. A subagent may edit only
  its assigned files; coordinate shared files sequentially to prevent
  concurrent overwrites.
- If a subagent with the required model and reasoning setting is unavailable,
  report that limitation truthfully and stop or request direction as
  appropriate. Never claim that delegation occurred, and never silently
  substitute another model or reasoning setting.
