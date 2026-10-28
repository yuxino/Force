# Background quotes — 2026-10-28

- `scripts/check.sh` passed, plus strict Clippy and all 171 app/UI tests with
  `ui-test-support`. Both quotes cover all four pages, 699/700-point height
  boundaries, the default window and the 780 × 620 minimum window.
- Signed macOS development build, using a dedicated temporary database: the
  longer daily quote rendered completely on calendar, task, records and settings.
  It disappeared at the minimum window size and returned when enlarged. Task
  input, editing, timer start/pause and window hide/reopen remained usable.
- Native export cancellation exposed an existing direct GPUI activation call.
  A process sample showed recursive key-status callbacks waiting on the same
  window mutex. Export now reuses the existing safe manager-presentation path.
  After rebuilding, native cancellation followed by Cmd+N, typing and Backspace
  succeeded; input also worked after hiding and reopening the manager.
- Date alternation and the first quote's geometry were checked automatically;
  the native screenshot uses the second quote selected for this date. Windows
  and Linux native appearance, real IME composition and a real midnight rollover
  were not exercised in this task.
