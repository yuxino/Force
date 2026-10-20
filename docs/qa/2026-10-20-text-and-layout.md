# Force interface polish — 2026-10-20

This revision builds on the management workspace changes based on `967d498`.
The complete workspace and text/layout fixes are being submitted together after
review. UI regression checks are also included in the macOS CI job. This
verification used the development bundle; the current text/layout fixes have not
replaced the production installation.

## Changes

- Reproduced the supplied task-text failure in a freshly built native development
  app: the full row showed only `Rev` from an English title and `计` from its state.
  An explicit 188-point box did not fix the actual glyphs.
- GPUI 0.2.2 reuses mutable text runs across ellipsis measurements. A zero-width
  pass shortens a run to the three UTF-8 bytes of `…`; a later wider pass then
  renders only three bytes of the original text. Single-line labels now shape
  against final bounds with fresh runs. Task rows, records, tags and the strip
  share this path. Two-line operation headings wrap without that ellipsis path.
- Setup, running and completion fill the same operation column. Removed the
  30-point shift on state changes and the extra sidebar inset. The task heading
  aligns with the daily goal, rows are 64 points high, and their text has 214
  points of available width. Labels and tag selection share a left alignment.
- Native review also found the tag menu displaced below its control. Its
  measurement canvas now starts at the control's top-left. The regression failed
  before the fix with a 51-point gap from trigger bottom to the first option.
  The final native build confirmed that the menu sits directly below its
  visible control.
- Updated the interface contract and native-text verification guidance. The text/layout fixes
  do not change persistence, dependencies or platform-window logic.
- Pre-merge review found a legacy focus-resume path that could leave rest or
  another focus running. It now uses the same atomic focus-switch policy as
  saved tasks while retaining the original session, elapsed time and timer mode.
  A transaction failure rolls back every affected timer.

## Verification

- `scripts/check.sh`: formatting, strict workspace Clippy, 68 default tests and
  whitespace checks passed.
- `cargo test -p force-app --features ui-test-support --locked`: 31 application
  tests passed. Including the overlapping default suite, 78 distinct tests passed.
  The new menu-anchor case covers default and minimum window sizes. The text
  regression checks UTF-8 run coverage directly because TestPlatform ignores
  font runs and correct boxes do not prove correct native glyphs.
- Strict Clippy with `ui-test-support` passed.
- The new legacy-resume model regression failed against the previous fallback
  (two active timers instead of one), then passed after the fix. Three additional
  core cases cover free/timed rounds, invalid/elapsed states, and a SQLite trigger
  rejecting the final interval insertion to verify complete transaction rollback.
- A signed `Force Dev.app` was built with the existing private-key-backed identity
  and passed strict bundle verification. The production executable checksum
  remained unchanged.
- Native macOS checks used a dedicated temporary store. Verified full short
  Chinese and English task labels; long Chinese/Japanese/English titles with
  emoji; correct sidebar and tag ellipses; setup, running and completion; records
  and settings; ordinary keys, cursor editing, Return, and input after cancelling
  a native save dialog and reopening management. The save dialog was cancelled.
- The task-specific patch and native focus screenshot are retained with the chat
  artifacts. Temporary editing files and the temporary test store were removed
  after stopping the development test process. The signed development app and
  reusable project build cache are retained for review and further development.

## Evidence limits

The minimum 780 × 620 window and independent scrolling have headless layout/event
coverage. Native resizing to that exact size was not established by this run.
The Chinese input above was pasted; actual IME composition, candidate confirmation
and dialog-return composition were not accepted. Immediate typing in the same
management window after dialog dismissal was not independently established;
typing after reopening was verified. Windows/Linux native interaction, the full
strip at screen edges, notarization and formal installation were not tested.
The production application and its data were not used for QA.
