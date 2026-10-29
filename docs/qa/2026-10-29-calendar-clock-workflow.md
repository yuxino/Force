# Calendar and cat-clock workflow acceptance — 2026-10-29

## Automated checks

- `scripts/check.sh` passed: formatting, workspace Clippy, 61 application tests,
  117 core tests and two process-interruption tests.
- The feature-enabled application/UI suite passed all 250 tests; feature-enabled
  Clippy also passed. Counts describe this verification run.
- TestPlatform covers the 780 × 620 minimum window. These bounds and event
  checks do not establish native rendering or input-method acceptance.
- A regression also checks that opening a prepared task from the cat suspends
  an open note editor while preserving both drafts and restoring duration focus.

## Native macOS checks

A signed Force Dev app used a separate test data directory at the default
1040 × 780 management-window size. Production data was not used for acceptance.

- Edited a calendar record while preserving its two recorded intervals, and
  edited an untimed task completion.
- Cancelled completion deletion, then entered an ordinary `x` in its editor.
  Repeated cancellation followed by ordinary `x` input for a point-in-time note.
  Verified `Cmd+Return` saving.
- Inspected the cat clock when idle, prepared, counting up, paused and saved;
  exercised “再计一次”. The readout stays below the cat, and saved rounds retain
  their recorded duration.
- Inspected the `24:00:00` readout and the “设置时长” state with an empty countdown
  duration.

## Boundaries

Native resizing to 780 × 620 was attempted but not established; minimum-window
acceptance remains headless only. Edge docking has no native interaction proof
from this run. A 100-hour readout was not inspected natively. Real IME composition
and candidate confirmation, and Windows/Linux native behavior, were not tested.
Ordinary `x` input and pasted Chinese text are not IME acceptance.
