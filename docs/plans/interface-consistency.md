# Force workspace interface

## Windows and navigation

Keep the 328 × 68-point floating timer independent of the normal management
window. Management starts at 1040 × 780 points, with a 780 × 620 minimum. Opening,
resizing or navigating management must not move the strip. Starting or resuming
a timer preserves the windows the user has open; closing management is explicit.
macOS and Windows hide management on close; Linux currently minimizes it.

The primary navigation has 专注 and 记录. 专注 supports current work; 记录 supports
date-based review. Settings is a secondary gear control. Strip pinning belongs
in settings; CSV export appears only in records. Both windows share one model
and store. Inputs belong to management, and strip actions dispatch to that window
after the current window borrow ends.

The strip prioritizes the running session. Browsing a paused task in management
must not replace the running timer shown on the strip or retarget its controls.
Without a running session, the strip may show the selected paused session or
completion state.

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
  task, duration, tag and start
  together. Running uses a 72-point tabular timer, visible 暂停计时 / 继续计时 and
  结束本次计时 actions, and a secondary rest/return action. Completion shows a compact
  receipt and next actions without removing task navigation or daily progress.
- One task list includes unfinished tasks and unfinished legacy sessions without
  a task association. Do not duplicate it as recent tasks and other timers.
  Each row shows the title and current state or accumulated investment. Clicking
  a row selects it for inspection; starting/resuming requires its explicit action.
  Keep 64-point task rows and an explicitly sized 214-point text column. Align
  the sidebar heading with the Today goal above, without an extra left inset.
  Measure title/status bounds and verify their actual native glyphs. Shape
  single-line labels against final bounds with fresh text runs; GPUI 0.2.2
  built-in ellipsis can keep shortened runs from an earlier width measurement.
  Complete titles remain available in tooltips; two-line operation headings wrap.
- The sidebar contains the single visible 新建 entry. Selected inactive tasks
  can be removed from the list while retaining their records. Ending a round
  remains distinct from completing its task.
- Selection synchronizes task, tag and timing mode and resets the left operation
  scroll. Resuming focus or rest follows the same atomic switching policy.
  Ending another timer must not hide the running one.

## Styling and settings

Use the system font, white surfaces and neutral black/gray hierarchy. The header
is 68 points high. Today uses a 28-point heading; records/settings use 32-point
page headings. Section headings are 16 points, body and control labels 14 points,
and secondary metadata 12–13 points. Compact task/summary durations show seconds
below one minute and omit zero minutes after whole hours. Keep necessary labels and errors visible;
optional explanations use contextual help. Routine removal icons stay neutral.

Settings have a 160-point section-title column, a 32-point gap and an elastic
content column, with 36 points between groups. Timing preferences contain strip
pinning, completion sound and daily goal. Pinning and sound each use a
keyboard-accessible button around a passive switch indicator. Tags use 44-point
rows and a list capped at 240 points. The tag input, type choice and 添加 / 保存
stay on one row at the minimum width. Local data exposes the folder; export is
owned by records.

Single-line inputs and settings controls are 40 points high; main start and
running actions are 44 points. Set input styled height explicitly because
`Input::h` affects multiline editors only. Reuse field, button and semantic-color
helpers. Current choices retain neutral hover, pressed and focus feedback.
Do not clip focus rings when truncating labels. Toggle double-clicks must not
undo the first click.

Disabled controls remain legible without whole-element opacity. The disabled
start action uses `#626262` text and icon on `#e9e9e9`; enabled start uses its dark
primary style. `gpui-component` 0.5.1 hardcodes red button hover text, so labels
and icons need explicit semantic foregrounds. Do not register duplicate hover
handlers.

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
Keep previous/next day buttons beside the date and search at the trailing side.
Activity uses local midnight boundaries and merged focus intervals, excluding
pause and rest. Load the whole series with one batch query when records refresh,
cache it in the model, and update today's cell from the normal statistics tick.
A day change or storage refresh reloads the series; never query once per cell.

## Input and records

- Tag menus open from pointer, Return or Space and use arrows, Return and Escape
  within an opaque list capped at 240 points. Selection uses stable tag IDs.
  Navigation, hiding management, completion and changed options dismiss stale
  menus. Restore prior focus without overriding a deliberate focus change.
- Button and Return share validation. Save requires changed, valid input; show
  errors beside the field. Preserve drafts across navigation. Goal restore uses
  the saved value without a database write. Tag validation covers duplicate names
  across both types, invalid characters and removed edit targets. Scroll the tag
  form into view when editing or recovering from an empty focus-tag list.
- Escape first closes local editing/details/help, then hides management. Feedback
  stays dismissible in the content flow without covering controls.
- Records provide date navigation, 今天, search, daily focus and sessions with
  investment on the selected date, including running and paused rounds.
  Daily focus includes ongoing investment and is independent of search. List
  entries use actual interval overlap without the recent-history API's 500-item
  cap. Retain zero-time start/completion events on their event date. A paused
  gap alone does not place a record on that date. Reset scrolling/details when
  date or search changes. Mark rest explicitly;
  rows show 当日 time and, when different, 整段 time. Cross-day time
  ranges include dates; unfinished rounds show their state and cannot be deleted.
  Expanded details show daily/total investment, excluded pause time, full task,
  tag, dated start/end and timing mode. Following today advances at local
  midnight; browsing another historical date stays fixed.
  Long values wrap, and details close when the record disappears.

## Acceptance

Check default/minimum sizes, long task and tag names, fixed summary/sidebar
bounds across setup/running/completion, independent scrolling, selection without
starting, explicit start without window hiding, running-strip stability, empty
records, validation, keyboard actions, dialogs and close/reopen. Measure the
internal task title/status widths and the full activity grid, selected day and
future-cell behavior. Use temporary data only.

GPUI TestPlatform establishes layout bounds and dispatched events, not native
pixels, real hover appearance, window-manager behavior or IME composition.
Before native click checks, place the development strip clear of management
controls: an app-window screenshot can omit a separate floating window that
still intercepts clicks.

Native acceptance needs the signed app and a real input method, including normal
key presses after dialogs and reopening. Preserve pre-GPUI AppKit initialization
and deferred keyboard callbacks. macOS evidence does not accept Windows/Linux
interaction; Linux minimization/restoration still needs native verification.
