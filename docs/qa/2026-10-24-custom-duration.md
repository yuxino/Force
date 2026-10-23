# Custom duration and new-task interaction — 2026-10-24

Tested application source: `e57a9af98e195d752decb6ac6df3a6e616d43f90`.

## Automated evidence

- Canonical `scripts/check.sh` and strict UI-feature Clippy passed on macOS.
- 125 distinct tests passed: 61 core, 2 process-recovery, and 62 app/UI tests
  (the latter includes the default 23 app tests).
- Focused coverage includes custom 1/37/1440-minute starts, invalid input,
  button/Return parity, paused/running target changes, unlimited-to-countdown,
  state and interval preservation, reopen, expiry rejection and write rollback.
- Event/layout checks cover default/minimum sizes, repeated New preserving the
  draft, editor cancellation, selection changes and hide/reopen preservation.

## Native evidence

Signed release-shaped development app on Apple Silicon, using disposable data:

- Ordinary numeric keys entered 37 minutes and Return started that countdown.
- Running edit changed 37 to 30 minutes without resetting elapsed time.
- Paused edit changed 30 to 21 minutes; the original 15-second interval and
  paused state remained unchanged. The temporary database confirms a 1260-second
  target and the same single 15-second interval.
- Zero input displayed a field error and Return left the saved timer unchanged.
- Closing/reopening management retained a 20-minute draft; ordinary keys and
  Return then saved 21 minutes.
- Default and minimum windows displayed custom input and active editing. Primary
  actions remain visible in the normal compact timer; expanded editing scrolls
  with the operation area. Repeated New preserved a 42-minute draft, with no
  redundant sidebar New action.
- Formal bundle signature and complete designated requirement match the existing
  installation. Installed executable SHA-256:
  `db12dec31bf4c214f6fb42e4cc8c67b7d2120e46854c62238616e265b4b12ddc`.
  The installed executable launched with the disposable data directory and its
  mapped executable was verified at the formal installation path. The previous
  running process is retained until the user reopens Force; its old bundle is
  kept only while that process uses it.

No real user records were used for acceptance. This verifies macOS rendering and
ordinary numeric input, not a new IME-composition or Windows/Linux native pass.
No public release was published.
