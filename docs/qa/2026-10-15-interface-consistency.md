# Interface consistency — 2026-10-15

Visual pass based on main `d673fae`. The owning rules are in
`docs/plans/interface-consistency.md`.

Signed macOS development app, Apple Silicon, generated tasks and disposable data:

- Reviewed compact, setup, running, paused, completion, records and settings.
- Verified a one-hour timer, two-line task heading, long record title, long tag
  row, tag picker and recent-task button. Long text uses ellipses; adjacent
  duration and action controls remain visible.
- Checked empty/populated records, selected navigation and duration controls,
  disabled next-day navigation, destructive icons and transient tag feedback.
- Typed task/tag text, started and paused using keyboard actions, finished a
  segment and selected a long tag from its menu. Opening and canceling the native
  export dialog still allowed ordinary typing immediately afterward.
- Settings scrolls to its data/export/version/quit actions without moving the
  strip or navigation. Setup's primary action remains above the fixed footer.
- The optimized bundle passed strict signing checks and replaced the installed
  app with the same designated requirement. The installed bundle was checked
  with the disposable store for styled setup, long labels and ordinary typing.

`scripts/check.sh` passed formatting, strict Clippy, 4 app tests, 40 core tests
and diff whitespace. Unit tests do not prove native layout; native observations
above provide that evidence. No business-rule or storage changes were made.

Windows/Linux interaction, physical IME composition, external displays and the
upward-opening panel at a screen edge were not reverified by this visual pass.
