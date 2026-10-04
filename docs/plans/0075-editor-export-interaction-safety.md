# Editor Export Interaction Safety

## Goal

Make editor export UI state accurately represent the asset and recipe used to
create an export plan, prevent duplicate confirmation and rollback execution,
and preserve access to rollback for a completed copy while the user continues
editing.

## Scope and assumptions

- Keep the change in `WorkflowWorkspace.tsx` and its focused Vitest coverage.
- Continue using the existing `chooseEditedCopyTarget`, `previewEditExport`,
  `executeEditExport`, `previewEditRollback`, and `executeEditRollback` API
  contracts; no backend or IPC changes are required.
- A recipe reset or edit invalidates pending plan work and any displayed plan.
- A completed export is a single in-memory record for the current `EditorView`.
  Recipe edits and resets preserve it; another successful export replaces it;
  closing the record only hides the rollback entry. Unmounting the editor drops
  this UI entry. Persistent export-history UI is outside this milestone.
- Keep editor action buttons on the existing shared rounded control geometry.

## Implementation steps

1. Track each plan request with a generation, asset ID, and immutable recipe
   snapshot. Invalidate it on every recipe update, reset, newer plan request,
   asset change, and component unmount. Ignore stale target-picker and preview
   responses, including stale errors.
2. Use a synchronous operation lock for export confirmation and rollback
   actions so rapid repeated clicks cannot invoke execution twice. Keep busy
   state visible to React and disable the related actions while locked.
3. Preserve completed export and rollback-plan state through recipe updates.
   Add an explicit action to dismiss the completed-export record, without
   touching the generated file.
4. Add focused tests for reset invalidation, out-of-order plan responses,
   duplicate confirmation prevention, and completed-export retention after a
   recipe edit. Correct the stale current-functionality statement about
   rollback execution.

## Verification

- Run the focused `WorkflowWorkspace.editor.test.tsx` Vitest suite.
- Run Prettier on changed files, ESLint on the changed TypeScript files, and
  the TypeScript project typecheck.
- Review the final diff and confirm no files outside the assigned set changed.

## Check record

- Passed: `npm run test -- src/components/WorkflowWorkspace.editor.test.tsx` (5 tests).
- Passed: `npm run typecheck`.
- Passed: ESLint on `WorkflowWorkspace.tsx` and
  `WorkflowWorkspace.editor.test.tsx` with zero warnings.
- Passed: Prettier check on the four assigned files and scoped `git diff --check`.
- Passed after documenting the backend rollback ownership constraint: Prettier
  and scoped `git diff --check` on `current-functionality.md` and this plan.
- The first check attempt found an incomplete test fixture, an unused mock
  parameter, and render-time ref access. These were corrected before the
  passing rerun.
- Not run: full Vitest suite and application build; the requested focused
  behavior, lint, type, and formatting checks passed.

## Risks

- Plan dialogs and previews can resolve after UI state changes; every async
  continuation must check both request generation and mounted/current asset
  state before mutating editor state.
- The frontend must not imply that dismissing a record deletes its output.
