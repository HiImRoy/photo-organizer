# Actions command forwarding and artifact storage safety

## Goal

Correct the Windows CI command wrapper so native commands receive the exact
argument vector entered in the workflow, preserve streamed output and exit
codes, and reduce default Actions artifact storage without changing which CI
checks run.

## Scope and assumptions

- The observed failure is in PowerShell's Windows `.cmd` argument forwarding.
  Historical behavior is measured once against `HEAD` in a temporary checkout;
  the committed regression suite exercises only test-owned probes and the
  repaired explicit-array path.
- Exercise the wrapper with a test-owned temporary directory and a native
  executable that reports its received arguments. Do not run `npm ci` or alter
  `node_modules`; use only read-only npm commands for a real npm check.
- Diagnostic log artifacts should be opt-in for manually dispatched workflows
  and retained for 3 days. Logs remain visible in the Actions job interface.
- Installer artifacts remain gated to manual dispatch or version tags and are
  retained for 7 days. Both workflow checkouts must fetch Git LFS objects so
  committed model pointers resolve before fixed-hash validation. These changes
  affect future artifacts only; they do not remove existing artifacts or reset
  this month's billed storage usage.
- Do not delete remote artifacts/caches, change billing or repository defaults,
  change the canonical manual launcher, add dependencies, or commit/push.
- Keep all edits within the assigned wrapper, new regression script, workflow,
  this plan, and narrowly relevant testing/release documentation.

## Implementation steps

1. Reproduce argument binding and native argv behavior under PowerShell 7 and,
   when installed, Windows PowerShell 5.1. Record how the current wrapper
   handles no command, one argument, multiple arguments, Unicode/space-bearing
   values, `--`, stderr, and non-zero exits.
2. Keep the wrapper's `Command` parameter as `string[]` and split it into a
   typed zero/one/many native argument array. Change every workflow call to use
   an explicit `-Command @(...)` array so PowerShell's advanced-parameter binder
   cannot reinterpret native switches. Preserve logging, merged visible
   stdout/stderr, and exact native exit-code propagation.
3. Add a self-contained test script using temporary test-owned paths and a
   native argv probe. Cover zero/one/many arguments, spaces,
   Chinese/Cyrillic/emoji, `-p`, `-e`, `--all-targets`, literal `--`, stderr,
   and non-zero exits, and run it independently in CI after Node setup. Use
   only `npm.cmd --version` for real npm verification.
4. Add an opt-in `workflow_dispatch` boolean input for diagnostic log uploads,
   gate both log upload steps on that manual input, set log retention to 3 days,
   set installer retention to 7 days, and enable LFS on both checkouts. Keep
   validation and bundle triggers, commands, and required checks intact.
5. Update only the relevant CI/release descriptions in `docs/testing.md` and
   `docs/release.md`.
6. Review the diff and run changed-file formatting/diff checks, the wrapper
   regression script under available PowerShell versions, safe read-only npm
   verification, the frontend build, and
   `scripts/manual-build-start.ps1 -CheckOnly`. Do not run full Rust builds;
   unrelated Rust work is in progress.

## Acceptance criteria

- The wrapper passes exact argv boundaries (including zero/one/many arguments,
  spaces, Unicode, native switches, and literal `--`) and logs stdout/stderr
  while returning the native exit code, including failures.
- Tests do not invoke `npm ci`, write to `node_modules`, or touch personal
  photo directories.
- Ordinary push and pull request runs do not upload diagnostic log artifacts;
  a manually dispatched run uploads them only when explicitly opted in, for 3
  days. Installer artifacts retain for 7 days and remain manually/tag gated.
  Both checkouts enable LFS so the model hash checks receive real file contents.
- No checks are removed or weakened, no remote state is changed, and skipped or
  failed verification is reported accurately.

## Confirmed root cause and execution notes

- The original `if` expression assigned `$arguments` as a scalar when exactly
  one native argument remained. Splatting that scalar through a Windows `.cmd`
  shim split `ci` into `c` and `i`; an absolute-path fake `npm.cmd` using the
  original `HEAD` wrapper recorded `argv=["c","i"]`. npm interpreted `c` as
  its config alias and emitted the observed EUSAGE message. The old real
  `npm.cmd --version` path also received a malformed `-` token and returned
  `Unknown command: "-"`. A typed `[string[]]` array preserved `ci` as one token;
  the same fake recorded `argv=["ci"]`, and the repaired real read-only
  `npm.cmd --version` returned `10.9.0`.
- Each workflow caller now explicitly exits with `$LASTEXITCODE`: in the
  custom `pwsh -File` shell, a nested script's `exit` sets the caller's status,
  but the run file must itself exit with that status to fail the Actions step.
  The regression launcher follows the same contract and checks exit code 23.
- One early baseline attempt failed to select its fake shim and accidentally
  ran the repaired real `npm.cmd ci`, which installed 278 packages into
  `node_modules`. No further `npm ci` is run. The prior directory contents were
  not captured, so `node_modules` is left intact rather than guessed at or
  deleted; the permanent test invokes the fake shim by its absolute temp path.
- The first manual launcher `-CheckOnly` then failed only because that npm
  operation had removed `.photo-organizer-package-lock.sha256`. Recovery was
  permitted only after the recorded npm child exit was confirmed as 0, the
  lockfile had no Git diff, `npm ls --depth=0` passed, and the launcher's
  required `vite.cmd`, `tsc.cmd`, and `tauri.cmd` files were present. With the
  marker confirmed absent, only that ignored metadata file was restored using
  the launcher-equivalent uppercase SHA-256 of the unchanged `package-lock.json`
  (`29A0266E5839FC4F4AD3845DC7D0A5923E6346C54909110175356599743DEDA3`). A
  subsequent `-CheckOnly` passed without syncing, building, or launching.

## Verification record

- Passed: wrapper regression under PowerShell 7 and Windows PowerShell 5.1;
  frontend production build; Prettier; PowerShell AST parsing; and
  `git diff --check`.
- Passed after the marker-only recovery: manual launcher `-CheckOnly`. The
  original check failed because the marker was absent; its restoration basis
  and unchanged lockfile SHA-256 are recorded above.
- Not run: full Rust build/tests, remote Actions/LFS execution, or actionlint
  (not installed). No remote run was started.
