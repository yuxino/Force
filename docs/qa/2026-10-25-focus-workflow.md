# Focus workflow refinement — 2026-10-25

Source: `67ea2bd4998696d3a169ec703370c67082a9c65e`. Local signed debug preview and release build on macOS.
Acceptance used a dedicated temporary data directory; production records were
not used as test fixtures.

## Result

- Separate countdown/stopwatch modes, an always-visible countdown input with
  25/45/60 shortcuts, and direct task saving without starting a round.
- Compact actions and aligned task completion; timer meaning precedes digits.
  Running countdowns show an ending estimate; paused ones show elapsed/target.
- Canonical checks passed: formatting, strict workspace Clippy, 86 default
  tests and diff whitespace. Feature-enabled strict Clippy and all 64 app/UI
  tests passed (127 distinct tests including core/recovery coverage).
- Layout/event checks cover 1040 × 780 and 780 × 620, including the complete
  switch/start action while another timer context is displayed. Saving preserves
  mode/input and an existing running round; a second save-location click cannot
  create or complete another task. Starting uses the saved task identity.
- Actual signed native preview: direct 37-minute start, pause, invalid zero
  rejection, keyboard save to 40 minutes without resuming, new-task 25-minute
  reset, stopwatch mode display and save without timing, draft-switch dialog
  cancellation, ordinary key input after the dialog and after management
  close/reopen, Chinese glyphs from pasted fixture text, and a new countdown.
- The release bundle and existing installed app had the same complete designated
  requirement. Installation retains the bundle ID, signing identity and path.

## Evidence limits

The final native pixels were inspected at the default window size. Exact final
minimum-height native resizing was not established; minimum geometry is covered
by TestPlatform. No new actual IME composition/candidate-confirmation acceptance
was performed. Windows/Linux native interaction remains unverified. No public
release was requested or published.

GPUI 0.2.2 retains removed debug selectors between frames. Use live model state
and dispatched events for disappearance checks. The minimum-window start
regression now measures its full bounds, not just a clickable center.
