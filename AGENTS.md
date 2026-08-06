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
4. Add or update tests.
5. Run relevant tests, lint, formatting, and type checks.
6. Review the final diff.
7. Update relevant documentation.
8. Report unresolved risks honestly.

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
- Relevant automated tests pass.
- Type checks and lint pass.
- No original test fixture is unexpectedly modified.
- User-visible behavior is documented.
- The final diff has been reviewed.
