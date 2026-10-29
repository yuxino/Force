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
  40 × 36 when idle at an edge, 56 × 36 when timed and 228 × 104 with dock controls open. Keep the 896-point content grid, 40-point insets,
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
stopwatch time must not imply an invented target. Round setup defaults to stopwatch mode. Countdown alone reveals the minute
input and compact 25/45/60 shortcuts; arbitrary durations do not require a
separate custom mode. Setup puts the full-column activity picker first, followed
by an optional description. No activity is preselected. A blank description
uses the chosen activity name, while distinct existing titles stay intact.
Use a 24-point heading, 40-point fields and aligned labels. Timing mode and duration share one group with a segmented mode control
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

The compact clock places a neutral readout below the Siamese cat's paws. Show the
complete clock at a stable baseline and retain the real duration when paused or
saved. Idle reads 开始; invalid duration reads 设置时长 without a placeholder dash.
Clicking a prepared or active cat reveals one black panel with the specific
description first and its activity below when different. Time/state remain on
the readout below the cat. The title has a detail chevron and a full-name tooltip;
saved focus rounds also expose a quiet 详情 beside 再计一次. Show 结束计时 only for an active or paused session.
The saved-round action 再计一次 starts a new segment for that same activity;
clicking its title opens the task detail. Keep other paused tasks unchanged.
Clicking the cat again collapses the panel without changing timing or management
visibility. Keep the clock free of hover tooltips and a separate open button.
Native dragging near either screen edge docks it on macOS/Windows; the dock
uses a small paw grip while idle/prepared and a single 36-point-high tab with 分 or 时 units while timing. See [the edge handle design](edge-handle.md). Linux retains free movement and resizing. Preserve
running-session priority and visible errors across all clock shapes.

Calendar actual-record details offer local editing and 删除计时记录. Completion
markers have a separate editor for their description, activity and time; deleting
a completion hides its task and arrangements while keeping recorded intervals.
Existing notes can be deleted from their editor. All confirmation cancellations
return keyboard focus, and moving a record or completion to a different day
reveals the saved result there. Ending a timer saves the segment and shows a
compact receipt, without a new-rest step or an automatic task completion.

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

The original title-first entry and compact metadata grouping took cues from
[Reminders](https://support.apple.com/en-ca/guide/reminders/remndc729e28/mac);
[Calendar](https://support.apple.com/en-gb/guide/calendar/icalwr13-events/mac)
provides the reference for date-context creation. Force keeps its own neutral
visual language and two distinct operations: 新任务 opens an unscheduled task
draft, while 安排日程 opens the selected date’s existing plan editor.
Saving a task does not start timing. Mode changes retain custom minutes;
closing or changing views preserves the existing task/calendar draft rules.
The refinement changes control grouping and visual hierarchy, not persistence,
native window behavior, or timer routing.

## Activity-first refinement

Activities are the shared names used by task creation, records and settings.
Selecting an activity is sufficient to save or start; an extra description is
optional. Keep calendar plans distinct and preserve their titles. Presentation
uses the existing neutral grid and shared classification-color resolver. User
data classification cleanup is an explicit, backed-up local operation, never an
automatic migration for every installation.
