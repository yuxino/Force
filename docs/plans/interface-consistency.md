# Force workspace interface

## Windows and navigation

Keep the floating cat clock independent of the normal management window. It is
88 × 128 points when compact, 244 × 152 when expanded and 28 × 80 when docked.
An idle cat opens a new task directly. With a task, clicking the cat toggles its controls; dragging near either screen edge docks it
on macOS/Windows. Linux retains free movement and resizing. These local states
must not pause, resume or retarget the active timer. Management starts at
1040 × 780 points, with a 780 × 620 minimum. Opening, resizing or navigating
management must not move the clock. Starting or resuming a timer preserves the
windows the user has open; closing management is explicit.
macOS and Windows hide management on close; Linux currently minimizes it.

The primary navigation has 日历 and 记录. 日历 is the stable scheduling and
review surface; 记录 supports date-based detail. Task/timer details are contextual,
without a third primary navigation destination. Settings is a secondary gear
control. Clock pinning belongs in settings; CSV export appears only in records.
Both windows share one model and store. Inputs belong to management, and clock
actions dispatch to that window after the current window borrow ends.

The clock prioritizes the running session. Browsing a paused task in management
must not replace that timer or retarget its controls. Without a running session,
the clock may show the selected paused session or completion state. Keep the
compact readout inside the cream-colored Siamese cat. Its expanded black panel contains the
task title, status and one primary action: 开始, 暂停, 继续, 查看本轮,
查看任务 or 打开 Force as appropriate. Show 结束 only when the active or
paused session can be modified. Clicking the task title opens its management
detail without replacing the selected task or its draft. A short click animates
the drawing while its hitbox remains still; dragging does not trigger that
feedback. Click the cat again to collapse it; do not add a duplicate collapse button.
Do not add a separate open button or hover tooltips to the clock.

## Stable focus workspace

The fixed Today summary shows the date and real daily focus against its goal,
with one progress line. It remains visible outside the scrolling content. Do not
split goal and percentage into additional statistic cards.

Below the summary, use a flexible left operation area, a 32-point gap and a
240-point task sidebar. Content is at most 896 points wide inside a centered
976-point wrapper with 40-point horizontal insets. The left operation area and
task list scroll independently; the Today summary and sidebar heading stay fixed.
Use spacing between columns without a full-height dividing line.

- Setup, running and completion replace only the left operation area. Its
  content fills the left column and keeps the same left edge and width in all
  three states, aligned with the heading above. Setup keeps
  task name first, followed by a compact tag row. Setting labels share an
  80-point column and a 12-point gap; the tag picker is 168 points wide. Group
  timing mode and duration below a fine rule. Use one 168-point segmented
  倒计时 / 正计时 control, and an editable minute field with bordered 25/45/60
  shortcuts beside it. Keep the 24-point setup heading and 40-point fields;
  group Start and the outlined Save action beneath the settings. Mode switches retain that input. New task
  drafts offer 保存任务 beside 开始计时; saving creates no timer. Keep compact
  primary actions rather than stretching them across the column. Running uses a
  left-aligned tabular timer up to 104 points (80 in
  compact windows), fitted to the final column width without losing hours, visible 暂停 / 继续 and
  结束计时 actions, and a secondary rest action in the same row. Completion also keeps all round
  and task actions visible in one row at minimum width. Countdown uses a restrained
  scale based on elapsed/target time; an early finish keeps its actual fraction,
  and unlimited sessions show no invented target. Rest has only a primary
  继续任务 / 结束休息 action and secondary pause/resume. Returning to focus ends
  paused rest as well, atomically; global rest follows the running task, while
  a task-local rest follows the viewed task. Completion shows a compact
  receipt and next actions without removing task navigation or daily progress.
- One task list includes unfinished tasks and unfinished legacy sessions without
  a task association. Do not duplicate it as recent tasks and other timers.
  Each row shows the title and current state or accumulated investment. Clicking
  a row selects it for inspection; starting/resuming requires its explicit action.
  Give the running status a stronger neutral foreground and weight; selection
  continues to identify the task being inspected, independently of what is running.
  Keep 64-point task rows and an explicitly sized 214-point text column. Keep
  continuous rows with fine dividers and a left selection mark. Align the
  sidebar right edge with the summary above, without an extra left inset.
  Measure title/status bounds and verify their actual native glyphs. Shape
  single-line labels against final bounds with fresh text runs; GPUI 0.2.2
  built-in ellipsis can keep shortened runs from an earlier width measurement.
  Complete titles remain available in tooltips; two-line operation headings wrap.
- The header always offers a compact dark 新任务 button (Cmd/Ctrl+N), without
  requiring a calendar plan. Calendar creation uses a secondary outlined
  安排日程 button with a calendar icon, including day and popover views.
  Opening 新任务 preserves task drafts and suspends any changed calendar draft.
- The task sidebar contains selection only; do not duplicate the header entry.
  Repeated menu/shortcut requests focus the same draft without clearing its fields.
  Selected inactive tasks can be removed from the list while retaining their records. Ending a round
  remains distinct from completing its task.
- Selection synchronizes task, tag and timing mode and resets the left operation
  scroll. Resuming focus or rest follows the same atomic switching policy.
  Ending another timer must not hide the running one.

## Styling and settings

Use the system font, white working surfaces and soft-white navigation chrome and neutral black/gray hierarchy. The
68-point header aligns the geometric wordmark, two destinations and settings
with the content grid. The global 新任务 action is quiet; primary emphasis belongs
to the current task action. Date navigation sits beside its date. Tag menus match
their triggers, and open menus have no redundant trigger tooltip. Today uses a 14-point heading, 13-point date, 20-point
accumulated time and a full-width two-point goal line. Task headings and calendar dates are 28 points;
setup, records and settings use 26-point page headings. Section headings are 16 points, body
and control labels 14 points, and secondary metadata 13 points. Compact task/summary durations show seconds
below one minute and omit zero minutes after whole hours. Keep necessary labels and errors visible;
optional explanations use contextual help. Routine removal icons stay neutral.

Settings use a readable column of at most 560 points, aligned to the main
content left edge. Stack section titles above their rows, with 20 points between
groups. Keep labels and their controls on the same row. Timing preferences contain clock
pinning, completion sound and daily goal. Pinning and sound each use a
keyboard-accessible buttons with directly clickable switches. Knob and outer
button actions must each toggle once. Timing rows use 48 points. Tags use 40-point
rows and a list capped at 240 points. The tag input and 添加 / 保存
stay on one row at the minimum width. New labels are focus tags; existing rest
labels remain editable. Highlight the row currently being renamed with the
existing neutral surface. Re-selecting it preserves its draft; switching edit
targets protects changed text with Save, Discard or Cancel. Adding a label must
not change a saved task's label selection, including an archived label. Compare
tag edits with their original input so external renames do not become apparent
user changes or get overwritten by an untouched field. Daily-goal 保存 / 撤销
appear below the goal row only while changed, so typing never moves the input.
The footer places version/update on one row and folder/quit maintenance actions below.
Local data exposes the folder; export is owned by records.

Single-line inputs and settings controls are 40 points high; main start and
running actions are 44 points. Set input styled height explicitly because
`Input::h` affects multiline editors only. Reuse field, button and semantic-color
helpers. Current choices retain neutral hover, pressed and focus feedback. Month cells use
a subtle neutral hover and no duplicate day-content tooltip over the day popup.
Do not clip focus rings when truncating labels. Toggle double-clicks must not
undo the first click.

Disabled controls remain legible without whole-element opacity. The disabled
start action uses `#626262` text and icon on `#e9e9e9`; enabled start uses its dark
primary style. `gpui-component` 0.5.1 hardcodes red button hover text, so labels
and icons need explicit semantic foregrounds. Do not register duplicate hover
handlers. Navigation and duration underlines must use a custom variant with a
stable border color: Ghost variants clear it during hover/press, exactly when
a newly selected choice must remain identifiable.

## Calendar context editing

Keep the month visible while viewing a date or editing an arrangement. Use a
360-point anchored panel with intrinsic height; optional notes expand inside its
scrolling fields. Keep Save and Cancel together immediately below the form.
Closing, Escape, outside clicks and navigation suspend a changed draft; show
继续草稿 on the month. Explicit Cancel discards it. Selecting a different editor
protects changed work with Save, Discard or Cancel. The timeline is an explicit
secondary view with a visible return to the month. Actual records stay read-only.

Keep the anchored parent in GPUI's normal paint pass. Input and tag menus defer
their own drawing; GPUI 0.2.2 rejects nested deferred draws. Verify an actual tag
menu selection outside the parent panel, followed by Escape, in the native app.
Read-only detail titles wrap rather than relying on an unconstrained canvas line.

## Activity calendar

Records begin with a real-data 53-week calendar: seven rows per week from Monday
to Sunday, including disabled future padding through the last Sunday. Use gray
levels for zero, positive time below 30 minutes, 30–59, 60–119, and 120+ minutes.
Any positive time must be visually distinct from zero. Do not normalize levels
against the user's busiest day or generate sample activity.

Each cell is at most 12 points square, with flexible columns, 2-point horizontal
gaps and 3-point vertical gaps. The full grid fits the 700-point content width at
the minimum window size. Month labels, 13-point secondary text and a compact
少–多 legend identify the range without a large enclosing card. Tooltips show
the actual date and focus duration. The selected day has a clear outline;
future dates use a separate pale surface and cannot be selected.

Selecting a cell updates the date and records below without moving the calendar.
Keep previous/next day buttons beside the date and daily total. Put the count
and search in a separate list toolbar; export remains beside the page heading.
Rows share a flexible title column, a 112-point duration column and an 80-point
action column, separated by 20-point gaps.
Activity uses local midnight boundaries and merged focus intervals, excluding
pause and rest. Load the whole series with one batch query when records refresh,
cache it in the model, and update today's cell from the normal statistics tick.
A day change or storage refresh reloads the series; never query once per cell.

## Input and records

- Tag menus open from pointer, Return or Space and use arrows, Return and Escape
  within an opaque list capped at 240 points. Selection uses stable tag IDs.
  Navigation, hiding management, completion and changed options dismiss stale
  menus. Restore prior focus without overriding a deliberate focus change.
- New tasks can be saved without starting, including when their round duration
  is invalid; only starting validates that duration. Saving preserves mode and
  minutes and must not pause another round. Keep 完成任务 away from the former
  保存任务 position so a second click cannot complete a freshly saved task.
- Task edits save independently of starting. 保存 / 撤销 appear only for changed
  saved tasks; Return saves that edit without starting a timer. Protect unsaved
  existing-task edits and new-task drafts, including the initial empty workspace,
  before task/session switches or global rest. Completing a saved task clears its
  old form directly; its title must not become a new unsaved draft. Completing an
  inactive task and a finished round uses one 完成任务 action.
- Button and Return share validation. Save requires changed, valid input; show
  errors beside the field. Require an available focus label for a new task round
  or metadata save; continuing a saved paused round retains its label snapshot.
  Empty edited names and invalid characters show beside their fields, while
  pristine empty new forms stay quiet. Preserve drafts across navigation. Goal
  restore uses the saved value without a database write. Tag validation covers duplicate names
  across both types, invalid characters and removed edit targets. Scroll the tag
  form into view when editing or recovering from an empty focus-tag list. Storage
  retry preserves edited goal text, including invalid drafts; only an untouched
  field follows the newly loaded saved goal.
- Escape first closes local editing/details/help, then hides management. Feedback
  uses one compact transient toast without moving the page. Task/goal saves
  are quiet on success; changed controls and values provide confirmation.
- Records provide date navigation, 今天, search, daily focus and sessions with
  investment on the selected date, including running and paused rounds.
  A search with no matches offers a compact clear-search action that restores
  input focus. A date without records keeps its ordinary empty state.
  Daily focus includes ongoing investment and is independent of search. Reuse
  the same statistics snapshot when the selected date is today. List
  entries use actual interval overlap without the recent-history API's 500-item
  cap. Retain zero-time start/completion events on their event date. A paused
  gap alone does not place a record on that date. Reset scrolling/details when
  date or search changes. Mark rest explicitly;
  rows show 当日 time and, when different, 整段 time. Cross-day time
  ranges include dates; unfinished rounds show their state and cannot be deleted.
  Expanded titles wrap. Details add dated start/end, nonzero pause time and
  timing mode without repeating row labels or durations. Following today advances at local
  midnight; browsing another historical date stays fixed.
  Long values wrap, and details close when the record disappears.

## Design review before native implementation

Begin structural UI changes with a relevant real-product reference and a short
interaction contract: what remains visible, what each action opens, how work is
saved or retained, and the exact close/return target. Use one current owning
description; remove conflicting old navigation or layout instructions before
reusing an existing container. Available width or height does not justify
stretching a small edit into an entire workspace.

Build and review one coherent clickable prototype before changing the native
composition. Reuse the same synthetic data at 1040 × 780 and 780 × 620: empty
dates, long names, busy dates, plans, actual work and untimed completions. Walk
through direct review, create/save in place, edit/cancel, invalid-input recovery,
and returning from details to the current task. Check all exits, changed-date
drafts, actual text, action proximity and keyboard-focus restoration. Fix the
observed failure before moving to the next stage.

Repeat those same journeys in the signed native app, including ordinary keys
after dialogs and reopening. Compare prototype and native screenshots at the
same size. Compilation, test counts and container bounds support correctness;
they do not establish a usable composition or complete task flow. State which
journeys were actually covered instead of declaring the design accepted from a
passing automated suite.

## Acceptance

Check default/minimum sizes, long task and tag names, fixed summary/sidebar
bounds across setup/running/completion, independent scrolling, selection without
starting, explicit start without window hiding, running-clock stability, empty
records, validation, keyboard actions, dialogs and close/reopen. Keep the full
start/switch action inside the minimum-height viewport when
the running-timer context is present. Measure the
internal task title/status widths and the full activity grid, selected day and
future-cell behavior. Use temporary data only.

GPUI TestPlatform establishes layout bounds and dispatched events, not native
pixels, real hover appearance, window-manager behavior or IME composition.
Before native click checks, place the development clock clear of management
controls: an app-window screenshot can omit a separate floating window that
still intercepts clicks.

Native acceptance needs the signed app and a real input method, including normal
key presses after dialogs and reopening. Preserve pre-GPUI AppKit initialization
and deferred keyboard callbacks. macOS evidence does not accept Windows/Linux
interaction; Linux minimization/restoration still needs native verification.

Background event/layout acceptance uses temporary data and sends no OS input.
Unit/headless timer completion stays silent; the normal application retains its
configured sound. When uninterrupted desktop use is required, do not start or
activate native preview windows. GPUI 0.2.2 TestPlatform has no native pixel
renderer, so this path supplies no screenshots or native visual acceptance.

Launch macOS native previews normally in the foreground. Do not use `open -g`
to avoid desktop activation: two signed GPUI 0.2.2 launches stalled in reentrant
key-window notifications, while a subsequent foreground launch of the same
isolated preview responded normally. If activation would interrupt the user,
defer native acceptance and use headless checks instead.

The debug bounds map retains removed selectors between frames: a retained
selector does not establish that a control remains rendered. Assert current model
state and actual dispatched actions instead. Its platform frame callback is also
a no-op: `on_next_frame` and `ScrollAnchor`
callbacks are not exercised by ordinary headless refreshes. Use simulated wheel
events to check viewport bounds and controls; verify automatic scroll targeting
natively instead of changing production behavior to satisfy this test limitation.

## Visual roles and stable editing

White canvas, #fafafa navigation/outside-month surfaces, #e8e8e8 dividers,
#c9c9c9 control boundaries, #f7f7f7 selected rows, #f2f2f2 hover and #e7e7e7
press states have separate roles. Secondary operations are white outlined
buttons without a shadow; quiet actions share a custom neutral variant instead
of the library Ghost's fixed darkening. Timing mode selection uses a white selected segment inside a neutral group;
duration presets use compact bordered choices with a clear selected state. Neither resembles disabled
Start. The task sidebar heading is subordinate at 14 points.

Calendar editors reserve two lines for validation so bottom-snapped title and
time fields do not jump when typing clears an error. Fields scroll within the
popup, while validation and Save/Cancel remain visible outside the scroll area.
Expanding notes reveals and focuses the note field; validation reveals its first
invalid field, including after resuming a draft. A fixed error slot is functional
space, not an invitation to add helper prose to pristine forms.

In the day timeline, pending tasks are separated by horizontal rules rather than
individual boxes. Hide an hour label only while the current-time label overlaps
it; retain the hour grid line and its click target.
