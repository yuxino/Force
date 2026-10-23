# Workbench redesign — 2026-10-23

Application revision: `b20d6db`, based on `70e94ef`. This is a substantial design
change on `feat/force-workbench-redesign`; merging main and replacing the formal
app await the owner's explicit approval.

## Changes

The geometric wordmark and navigation share the content grid. Focus uses an
open, left-aligned task/timer composition, a quiet daily summary, continuous task
rows and a countdown scale based on actual elapsed time. Early completion keeps
its real fraction; unlimited sessions have no target scale. Complete time strings
fit their final width, including unusually long hour values.

The strip gives the task its full first line and reserves three action slots on
the second. Records group date and daily time, move export beside the heading and
align title, duration and actions. The final minimum-window review reduced the
space above records so the first task and its action enter the initial viewport.
Settings use shared section rules, fields and aligned rows.

Focus and record rendering moved out of the monolithic view. No core, model,
storage, dependency or platform-window implementation changed. An independent
review compared the relocated operation callbacks and found no semantic change.
It also caught Ghost hover clearing selected underlines; navigation and duration
now use a shared custom variant with a stable border across pointer states.

## Verification

- Canonical `scripts/check.sh`: formatting, strict Clippy, 80 default tests
  (23 application, 55 core and 2 interrupted-process), and diff checks passed.
- Strict Clippy with `ui-test-support` and all 57 application tests passed.
  These include the 23 default application tests: 114 distinct tests in total.
- Default/minimum layout checks cover title/status columns, daily-goal extremes,
  record columns, ordinary countdowns, long Chinese headings and 100-hour
  stopwatches. Actual TestPlatform events cover pause, finish, repeat, drafts,
  settings and independent scrolling. The countdown marker remains fixed while
  paused and retains its fraction after an early finish. Strip actions keep their
  positions across empty, running, paused and completed states.
- Text-system tests cover complete time strings through narrow/wide layouts,
  including the maximum signed 64-bit time value, without ellipsis or stale runs.
- An isolated headless browser produced 12 local HTML design-preview images:
  default/minimum focus, setup, pause, rest, completion, records, settings and
  strip states. Prototype navigation, task input, timing choices, search recovery,
  settings and strip-slot checks passed with no page errors or horizontal overflow.
  The images use synthetic data and are **not native application screenshots**.
- A release-mode Force Dev bundle was built with the existing signing identity,
  using the canonical output and two low-priority build jobs. Strict signature,
  development bundle identifier and background version checks passed. Executable
  SHA-256: `d91918377f9b2506e605d2bd0d3de5852e1b77ff18677a32a4795f81c0a81844`.

## Limits and delivery state

The owner required uninterrupted video playback. No native app or visible
browser window was opened, no OS input was sent, and no formal process was
restarted. The formal executable remains unchanged at SHA-256
`0595c89207a6bfccc2cfd058b97677c65b52247c7fc255291d55b696a340fd58`.
Its running previous bundle remains in use and is preserved.

GPUI TestPlatform validates geometry and events with simulated fonts; it does not
render native pixels, execute native frame callbacks or accept IME composition.
HTML previews demonstrate the design direction, not exact GPUI raster output or
all native confirmation dialogs. Native hover, glyphs, automatic scroll targeting,
input methods and Windows/Linux interactions remain to be checked without
interrupting the owner's current desktop use. No public release was published.
