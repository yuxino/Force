# Daily calendar

Force opens on a calendar for planning what to do at specific times and
reviewing what actually happened. Calendar and Records are the primary
destinations; task/timer details remain available from the current activity and
recorded events, without a separate focus mode. The floating timer remains
independent.

## Data and behavior

Scheduled occurrences live in a separate SQLite table associated with a task.
They hold a local date, start/end clock minutes, note, occurrence completion and
rescheduling link. Creating an occurrence and its new task is transactional.
Editing time or notes and checking an occurrence never rewrites recorded timer
intervals, pauses another timer or completes the global task. An explicit
activity change updates the linked unfinished task and its unfinished session
metadata in the same transaction as the arrangement. Finished session snapshots
and all interval identities/timestamps stay unchanged; related plans use the
task activity. Completed tasks must be reopened before their activity can
change. Moving an occurrence to tomorrow leaves an original-day marker and a
linked successor.

Actual sessions and completed tasks are read from existing durable records and
automatically appear on their real local date. Matching task identities relate
actual work to a planned occurrence; do not copy the same daily total into every
occurrence. Untimed task completion remains a completion event, not invented
focus minutes. The UI does not divide entries into active/passive categories.

The default calendar is a full-width month overview. Each day directly shows
real task names, completed tasks, and pending arrangements, with extra items
counted when space runs out. Hover reveals all titles and each task's actual
investment. A day header shows accumulated task investment, excluding rest;
simultaneous tasks sum independently here, unlike the merged focus statistic.
Same-task rounds and related plans share one entry; moved markers remain
explicit. Query the visible month in batches and update live durations from
cached intervals.

Clicking a day opens a compact summary over the month, keeping the month grid
and its position visible. Selecting an arrangement shows its details and
actions; editing or adding opens a wider, centered editor above the calendar.
Actual sessions and untimed completions retain their detail and correction
actions. The explicit 查看当天时间轴 action opens the existing day timeline. 返回月历 and
the 日历 tab return to the month. Month navigation and returning to today preserve
the overview. Adding an arrangement from the month toolbar defaults to today,
independent of the month or day last inspected. Day-summary and timeline actions
retain their explicit date. The editor date can be changed directly. Date
selection supports future dates, with real planned-day and activity marks.

The editor groups title, date, times and activity together; notes expand only
when needed. Its 560-point width is bounded by the window; fields scroll within
a fixed-height dialog while validation and actions stay visible. The header
remains available for navigation. A saved arrangement retains an activity
picker; selection remains a draft until Save. Both arrangement and original task
activity are compared before writing, so stale edits and storage failures cannot
commit a partial activity change. Existing linked-task titles are preserved.
Save and Cancel remain visible at the bottom of the dialog, including at the
minimum window size and with notes expanded. Do not show a second Add action
while the editor is open. Save validates and writes the arrangement and any
explicit activity change transactionally, closes the popup in place and selects
the saved date. A failed save keeps every field and its error available for
correction. Explicit Cancel discards the draft and closes the popup.

Close, outside click, Escape, ordinary date or record inspection, and navigation
to Records or Settings suspend a changed draft without prompting or saving it.
继续草稿 restores the same editor and original month selection; changing its date
field does not change the draft's identity or lose its target date. An unchanged
editor closes without creating a suspended draft. Opening a different editor or
starting a plan protects a changed draft with Save, Discard or Cancel; saving an
old draft must not redirect the newly requested destination. A tag menu handles
its own Escape first; the next Escape closes the popup. Closing or saving the
popup returns keyboard focus to the calendar. Escape at the month root hides
management while preserving suspended drafts and running timers.

The day timeline keeps its proportional clock placement, readable overlap lanes
and real actual intervals alongside scheduled blocks. Its sidebar pairs date
navigation with the current timer, next arrangement and completion progress.
Empty clock slots create an arrangement directly. Starting an arrangement uses
existing transactional task switching, keeps the calendar visible and preserves
a paused round's recorded target. A contextual 返回计时 action returns to the
current running or paused round without changing timing.

No external calendar sync, inferred completion in other applications, reminders,
background automatic starts, accounts or network services are introduced.

## Validation

GPUI regressions cover centered normal/minimum-window bounds, stable placement
across calendar weeks, note scrolling, persistent validation/actions, today and
explicit-date entry, draft suspension/confirmation, month-cache refresh,
activity selection/save/reopen/cancel, archived activities, completed-task
restrictions and preserving independent or edited task drafts. Core tests cover
atomic activity updates, stale baselines, rollback, finished classifications and
exact intervals.

See [the arrangement-editor acceptance record](../qa/2026-10-29-arrangement-editor.md) for this change's checks and native evidence limits. TestPlatform
establishes layout bounds and dispatched events; it does not establish native
pixels, IME composition or Windows/Linux interaction.
