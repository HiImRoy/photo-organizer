# ADR 0004: Explicit import tree with manual Library hierarchy

状态：Accepted

## Context

PhotoOrganizer originally derived the initial Library hierarchy from explicitly
imported SourceRoots, but did not let users arrange already imported Libraries.
The product now needs an import-time choice for exposing image-containing
subfolders as Libraries and a safe way to reorganize those Library nodes.

## Decision

- The selected import root always creates one Library.
- When the user enables `includeSubfolders`, every directory containing a
  supported image becomes an additional Library SourceRoot. Empty directories
  remain invisible.
- When the option is disabled, the root Library keeps the existing recursive
  scan behavior and child directories do not become Sidebar nodes.
- SourcePath is used only to derive the initial parent for newly imported
  Libraries. A manual drag/drop changes only `parentLibraryId` and persists
  across rescans and restarts.
- The domain layer rejects self-parenting, missing parents, and cycles.
- No hierarchy operation moves, renames, deletes, or otherwise modifies a
  source file or directory.

## Alternatives considered

1. Keep all subfolders as an invisible scan implementation detail. This keeps
   the old model but cannot support users who need source-derived child scopes.
2. Expose the complete filesystem tree as navigation. Rejected because it makes
   disk structure the primary information architecture and creates virtual
   nodes for folders that were never imported as Libraries.
3. Allow unrestricted parent edits without cycle validation. Rejected because
   recursive browse/count queries and ownership reconciliation require an
   acyclic hierarchy.

## Consequences

- The Library table needs to persist whether a parent relation is still
  source-derived or was manually changed, so future imports do not overwrite
  user organization.
- Structured imports scan each Library SourceRoot separately under one user
  visible task; parent scans still prune explicit child SourceRoots.
- The UI gains a drag/drop interaction and a root drop target.
