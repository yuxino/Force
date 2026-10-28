# Force workbench redesign

## Direction

Rebuild the visual composition around the task, the time and the next action.
Use the Siamese cat brand mark. Neutral surfaces, strong type
contrast, aligned numbers, restrained rules and open lists give the small local
application a coherent identity. Do not add dashboards, account features,
illustrations, textures, gradients, marketing slogans or decorative cards.

The owner approved merging this substantial interface change after reviewing the
design. Preserve the running formal app and all production data during delivery.
Use short action labels and avoid repeating timer state in captions or log rows;
keep errors and meaningful distinctions visible.

## Shared language

- Keep the 1040 × 780 default / 780 × 620 minimum management window and separate
  floating cat clock: 88 × 128 points when compact, 244 × 152 when expanded and
  28 × 80 when edge-docked. Keep the 896-point content grid, 40-point insets,
  32-point column gap and 240-point task column.
- Use the existing system font with tabular time digits. Page titles are 32,
  task titles 28, main timers up to 104 (80 when compact), section labels 16,
  body/actions 14 and secondary information 13 points. Long timers and titles
  must remain within the real available width.
- Use neutral ink, soft white, readable gray and fine gray dividers. Keep the
  existing destructive semantic color. Main management actions remain 44 points
  high, fields 40–44 and most corner radii 4–6. Hover, keyboard focus and disabled
  contrast remain visible.
- Put brand, primary navigation, the single 新任务 action and secondary settings
  into one 68-point header. Give 新任务 a compact dark primary button; keep
  calendar 安排日程 outlined with a calendar icon. 新任务 and Cmd/Ctrl+N open the task form without
  requiring a calendar plan. Repeated requests focus the same draft; changed
  calendar drafts remain suspended and recoverable. Keep page headings and
  settings content on the same reading axes.

## Surfaces

Focus keeps its daily summary and task column stable across preparation,
running, paused, rest and completion. A thin daily progress line ties the summary
together. The operation column shares a left-aligned task heading, time area
and action row. Countdown progress may use a restrained functional scale;
stopwatch time must not imply an invented target. Round setup separates countdown
and stopwatch modes. Countdown has an always
visible minute input and compact 25/45/60 shortcuts; arbitrary durations do not
require a separate custom mode. Task metadata stays above this group. Setup uses a 24-point heading and
40-point fields, a compact 168-point tag picker, and aligned 80-point setting
labels. Timing mode and duration share one group with a segmented mode control
and bordered duration presets. New drafts
can be saved for later without starting. Primary actions have compact fixed
widths; keep task completion separate from the save position. Running shows
剩余时间 / 已用时间 before the digits, an ending estimate while running, and
elapsed/target while paused. Preserve the minimum-height start action even when
a different timer context is visible. Validation remains
beside its field; Save/Discard/Cancel behavior is preserved.

The task list uses continuous 64-point rows, subtle rules and a left selection
mark. Running status remains independent of selection. Preserve actual title and
status widths, tooltips, safe truncation and independent scrolling. Keep task
creation in the header rather than duplicating it in the sidebar; 保存任务 is a
secondary action that saves without starting a timer. Calendar 安排日程 remains
secondary and separate from task creation.

The clock shows time inside the cream-colored Siamese cat. Clicking the cat expands a single
black panel with its task title, status and primary action. Show 结束 only for an
active or paused session; do not reserve an empty action slot. Clicking the task
title opens the corresponding task detail or completion receipt, preserving
unsaved edits. Clicking the cat again collapses the panel without changing timing or
management visibility. Keep the clock free of hover tooltips and a separate open
button. Native dragging near the left or right screen edge docks it on
macOS/Windows; Linux retains free movement and resizing. Preserve model routing,
running-session priority and visible error feedback across all clock shapes.

Records read from yearly activity to the selected day to individual sessions.
Keep the real 53-week calendar and existing date/interval arithmetic. Move export
near the page heading; put date navigation and daily total together, followed by
search/list count and an open log. Duration columns align and expanded details
continue under their parent row without enclosing cards.

Settings remains one page, with the existing 160-point title column and 32-point
gap. Give sections a consistent rule and spacing; align fields, switches and tag
actions. Preserve tag edit protection, transient feedback and essential errors.

## Implementation and acceptance

Keep portable persistence and timer logic unchanged. Separate focus and record
rendering from the shared view helpers to let each surface use the same tokens
without growing the monolithic renderer. Reuse existing controls and model
methods; introduce no dependency, font download, storage migration or native
window-style change.

Create a self-contained interactive HTML design prototype using synthetic data,
rendered only in an isolated headless browser. Save default/minimum-size views,
calendar, records, contextual task details and clock previews. These are design
previews, not native screenshots. Refine the composition after inspecting the rendered images.

Update affected GPUI TestPlatform layout/event regressions against the accepted
new geometry while retaining behavior coverage, text bounds, independent scroll,
draft protection, title/validation edge cases and clock routing. Run canonical
checks, feature-enabled UI lint/tests and signed development packaging with the
existing deployment target and canonical caches. Do not launch or activate a
native window while the user's no-interruption instruction remains in force.
Native glyphs, hover, automatic scroll targeting, IME and OS window behavior
remain a separate acceptance boundary.

## Task creation refinement

The title-first entry and compact metadata grouping take cues from
[Reminders](https://support.apple.com/en-ca/guide/reminders/remndc729e28/mac);
[Calendar](https://support.apple.com/en-gb/guide/calendar/icalwr13-events/mac)
provides the reference for date-context creation. Force keeps its own neutral
visual language and two distinct operations: 新任务 opens an unscheduled task
draft, while 安排日程 opens the selected date’s existing plan editor.
Saving a task does not start timing. Mode changes retain custom minutes;
closing or changing views preserves the existing task/calendar draft rules.
The refinement changes control grouping and visual hierarchy, not persistence,
native window behavior, or timer routing.
