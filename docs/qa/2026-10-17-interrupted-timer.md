# Interrupted timer recovery — 2026-10-17

Startup now restores the saved timer selection and its countdown/free-timer
configuration. Missing or stale selection falls back to a running timer before
paused rounds. Selection changes are saved immediately through an atomic private
state file; Unix writes also sync the containing directory. Invalid state is
reported without replacing the file or changing timer records.

An expired round no longer covers another running timer with its completion
screen. Completed rounds still use their original exact deadline.

## Validation

- `scripts/check.sh`: formatting, strict Clippy, 6 application tests, 40 core
  tests and 2 process-test entries passed. One process entry is the child helper;
  the parent kills it with its SQLite connection still open, then verifies saved
  tasks, running/paused rounds, targets, completion and a second reopen.
- Signed macOS development bundle passed strict signature verification.
- Native acceptance used a dedicated disposable database. A task created with
  ordinary keyboard input survived forced process termination and resumed its
  countdown. A newer paused fixture did not replace the running timer on restart.
- On the final build, a selected task paused at 24:56 survived forced termination
  and reopened at 24:56, retaining its title and paused status even with a newer
  paused fixture present. The selection checkpoint was already on disk before
  termination. Panel and pause shortcuts worked during acceptance.

No existing user records were read, recovered or modified. The formal app was
left running. This verifies forced process termination on Apple Silicon macOS,
not physical power loss or Windows/Linux native behavior. No release was made.
