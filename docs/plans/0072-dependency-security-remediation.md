# 0072 — Dependency Security Remediation

## Scope

Address six currently open GitHub dependency security alerts across npm and
Rust. A read-only Dependabot query confirmed these three Rust alerts:

- Alert #5, [GHSA-7gcf-g7xr-8hxj](https://github.com/advisories/GHSA-7gcf-g7xr-8hxj):
  `serde_with` 3.17.0, patched in 3.21.0.
- Alert #3, [GHSA-r6v5-fh4h-64xc](https://github.com/advisories/GHSA-r6v5-fh4h-64xc):
  `time` 0.3.45, patched in 0.3.47.
- Alert #2, [GHSA-wrw7-89jp-8q8g](https://github.com/advisories/GHSA-wrw7-89jp-8q8g):
  `glib` 0.18.5, patched in 0.20.0.

The npm agent owns the other three open alerts: #8 `js-yaml` (GHSA-2883-xcg3-v3hh),
#7 `vitest` (GHSA-82fw-gwwq-j7x9), and #6 `@vitest/mocker` (GHSA-82fw-gwwq-j7x9).
The auto-dismissed `nanoid` alert is separate from the six open alerts and is
also part of the npm agent's review.

Trace each affected package to its direct or transitive dependency path and
identify manifest constraints. Apply the smallest compatible dependency updates
within `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`. Record why any alert
cannot be safely resolved within the current architecture. Review the separate
auto-dismissed `nanoid` alert for relevance, without changing JavaScript package
manifests or lockfiles owned by another agent. Integrate the npm and Rust
results with a final cross-stack dependency/security review after both phases
complete.

## Assumptions and boundaries

- Work on the existing `main` checkout. This agent phase does not commit; the
  main agent will commit the combined changes.
- Exclusive file ownership for this task is `src-tauri/Cargo.toml`,
  `src-tauri/Cargo.lock`, and this plan. Do not edit npm-owned files.
- Do not force a major Tauri/GTK architecture upgrade or alter production
  features to address a dependency that is absent from the Windows target.
  Determine whether `glib` is limited to the Linux target and document any
  incompatible residual alert with evidence.
- Preserve local-first behavior and existing production architecture.
- Do not access personal photo directories; keep any filesystem checks scoped
  to repository files and test-owned data.
- Use the installed Cargo executable through the available script/PATH or its
  known installation path. Request escalation only if network access or an
  inaccessible Cargo cache is required.

## Execution steps

1. The npm agent audits and updates its owned manifests/lockfiles for alerts
   #6–#8 and reviews the auto-dismissed `nanoid` alert.
2. Inspect the Rust manifests, lockfile, target-specific dependency graph, and
   repository state; map alerts #2, #3, and #5 to dependency paths and
   constraints.
3. Review upstream advisories and determine affected platforms and practical
   applicability.
4. Update Rust manifest/lockfile entries only where the patched versions are
   compatible with the existing Tauri and platform architecture.
5. Run relevant Cargo static checks and tests where feasible, using the
   installed toolchain and available cache/network access.
6. Consolidate both agents' dependency results, record cross-stack verification,
   and review the final diff and `git diff --check`; report resolved alerts,
   residuals, and checks not run.

## Rust audit results

- `serde_with` was transitive through `tauri-utils` 2.9.3, used by Tauri,
  `tauri-build`, code generation, macros, plugins, and runtimes. The existing
  `^3` constraint permits 3.21.0. The lock now resolves `serde_with` and
  `serde_with_macros` to 3.21.0, with their required `darling` 0.23.0,
  `bs58` 0.5.1, and `tinyvec` 1.13.3 dependencies.
- `time` was transitive through `cookie` 0.18.1 (Tauri, runtimes, Wry, and
  `tauri-plugin-log`) and `plist` 1.8.0 (`tauri-utils`). The lock now resolves
  `time` 0.3.47 with `time-core` 0.1.8, `time-macros` 0.2.27, and
  `num-conv` 0.2.1.
- The project's MSRV is Rust 1.88. `time` 0.3.47, `time-core` 0.1.8,
  `time-macros` 0.2.27, `serde_with` 3.21.0, its macros, and the updated
  `darling` crates declare Rust 1.88, so the updates match the project MSRV.
- `glib` 0.18.5 is used by the Linux Tauri stack through GTK-rs 0.18, including
  `gtk` 0.18.2 and `webkit2gtk` 2.0.2. Its GTK-rs parents require `glib ^0.18`
  (or `^0.18.0`): `atk` 0.18.2, `cairo-rs` 0.18.5, `gdk` 0.18.2,
  `gdk-pixbuf` 0.18.5, `gio` 0.18.4, `gtk` 0.18.2, `pango` 0.18.3, and
  `webkit2gtk` 2.0.2. The reverse tree reaches these through Tauri's `muda`
  0.19.3, `tao` 0.35.3, and `webkit2gtk`/Wry runtime dependencies. Therefore
  0.20.0 cannot satisfy the existing graph. The advisory describes undefined
  behavior in `VariantStrIter` that can cause a null pointer dereference. The
  Windows target tree contains no `glib`; leave this alert unresolved for Linux
  until upstream GTK/Tauri dependencies can move to a compatible stack. Runtime
  use of `VariantStrIter` was not independently established in this audit.
  Upstream corroboration: the current [Tauri dev manifest](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/Cargo.toml)
  still specifies `gtk = "0.18"`, and the [Tauri 2.12 release log](https://github.com/tauri-apps/tauri/releases)
  records downloading `glib` 0.18.5. This supports leaving the Linux residual
  documented without forcing a Tauri/GTK upgrade.
- No Rust manifest change was needed; only the scoped Cargo lockfile changed.
- The two targeted Cargo updates also consolidated duplicate Windows support
  lock entries (`windows-core` 0.62.2 and its `windows-result` 0.4.1/
  `windows-strings` 0.5.1 dependencies). No manifest or Rust source changed;
  Windows-host checks passed.
- Passed on the Windows host: `cargo check --locked`; `cargo test --locked`
  (169 library tests, 2 large-library benchmark tests, 1 semantic benchmark
  test, and 3 evaluation tests; 175 passed, 0 failed).
- Passed: `cargo clippy --locked --all-targets --all-features -- -D warnings`
  and `cargo fmt --all -- --check`.
- Passed: locked reverse dependency queries for updated `serde_with` 3.21.0 and
  `time` 0.3.47; the Linux-target `glib` path query; and the Windows-target
  query, which printed no `glib` dependency.
- Passed: Cargo metadata confirms the updated `time`, `serde_with`, and
  `darling` crates declare Rust 1.88 MSRV. `git diff --check` and the new plan's
  trailing-whitespace check passed.
- npm phase completed (reported by its owner): normal `npm ci`, `npm audit`
  (0 findings), all 145 tests, typecheck, targeted lint, and production build
  passed. Full lint reports three pre-existing generated-JavaScript errors
  under `test-data/.tmp`. Node 22.12 emitted an engine warning because the
  project declares 22.13.
- Cross-stack verification outcome: Rust check, tests, Clippy, and formatting
  passed; npm's production build and 145 tests passed. No npm-owned files were
  edited here. Full release packaging was not run.

## Verification

- Confirm dependency resolution and affected package versions with Cargo.
- Passed: Windows-host `cargo check --locked`; `cargo test --locked` (175
  passed, 0 failed); `cargo clippy --locked --all-targets --all-features -- -D
  warnings`; `cargo fmt --all -- --check`; dependency-tree and MSRV checks;
  `git diff --check`; and plan whitespace check.
- Passed in the npm phase: `npm ci`, `npm audit` (0 findings), 145 tests,
  typecheck, targeted lint, and production build. Full lint retains three
  pre-existing generated-JavaScript errors in `test-data/.tmp`; Node 22.12
  warns against the declared 22.13 engine.
- Linux compilation was not run; GTK/WebKit system dependencies are not
  available on this Windows host. The Linux Cargo dependency-tree query did
  complete successfully.
- Final scoped diff reviewed. Full release packaging was not run; the main
  agent owns the combined commit.

## Acceptance criteria

- The six open alerts across npm and Rust are correctly split between the npm
  and Rust phases; all three Rust alerts have documented paths and an explicit
  resolution or evidence-based residual explanation.
- Any compatible patched Rust packages are updated with minimal manifest and
  lockfile changes.
- The auto-dismissed `nanoid` alert is acknowledged without crossing file
  ownership.
- Verification results, skipped checks, and remaining risks are reported. This
  agent phase leaves the combined commit to the main agent.
