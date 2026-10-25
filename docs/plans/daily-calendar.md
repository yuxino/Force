# Daily calendar

Force opens on a calendar for planning what to do at specific times and reviewing
what actually happened. Calendar and Records are the primary destinations;
task/timer details remain available from the current activity and recorded events,
without a separate focus mode. The floating timer remains independent.

## Data and behavior

Scheduled occurrences live in a separate SQLite table associated with a task.
They hold a local date, start/end clock minutes, note, occurrence completion and
rescheduling link. Creating an occurrence and its new task is transactional.
Editing time or notes and checking an occurrence never rewrites recorded timer
intervals, pauses another timer or completes the global task. Moving an occurrence
to tomorrow leaves an original-day marker and a linked successor.

Actual sessions and completed tasks are read from existing durable records and
automatically appear on their real local date. Matching task identities relate
actual work to a planned occurrence; do not copy the same daily total into every
occurrence. Untimed task completion remains a completion event, not invented
focus minutes. The UI does not divide entries into active/passive categories.

The default calendar is a full-width month overview. Each day directly shows real
task names, completed tasks, and pending arrangements, with extra items counted
when space runs out. Hover reveals all titles and each task's actual investment.
A day header shows accumulated task investment, excluding rest; simultaneous
tasks sum independently here, unlike the merged focus statistic. Same-task rounds
and related plans share one entry; moved markers remain explicit. Query the
visible month in batches and update live durations from cached intervals.

Clicking a day opens a compact summary over the month, keeping the month grid and
its position visible. Selecting an arrangement shows its details and actions;
editing or adding opens a compact editor in the same context. Actual sessions and
untimed completions show read-only details. The explicit 查看当天时间轴 action
opens the existing day timeline. 返回月历 and the 日历 tab return to the month.
Month navigation and returning to today preserve the overview. Adding an
arrangement from another month starts on that month's first day. Date selection
supports future dates, with real planned-day and activity marks.

The editor groups title, date, times and label together; notes expand only when
needed. Existing linked-task titles are preserved. Save and Cancel remain visible
at the bottom of the popup, including at the minimum window size and with notes
expanded. Do not show a second Add action while the editor is open. Save validates
and writes the occurrence transactionally, closes the popup in place and selects
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

Focused GPUI regressions cover month context, six-row months, long titles and
overflow, minimum/default bounds, expanded notes and visible actions, draft
suspension and continuation, changed dates, explicit discard, read-only details,
validation errors, tag selection outside the popup and two-stage Escape. Existing
coverage retains hour-slot creation, related actual timestamps, completion,
rescheduling, failed-storage retry and local overlap widths.

Canonical checks and strict UI-enabled Clippy pass. The complete app suite has
103 passing tests; core coverage has 78 unit tests and 2 interrupted-process
integration tests.

Signed native macOS preview with disposable data has verified default and minimum
window composition, full detail titles, expanded notes with nearby actions,
close/continue and explicit Cancel, changed-date draft recovery, tag selection
outside the popup, save in place and timeline return. Ordinary key presses work
after the native draft-confirmation dialog and after hiding/reopening management.
With a hidden draft retained, the native timeline opens at 08:00 for a future day
and moves to 13:00 before a newly selected day's 14:00 arrangement. TestPlatform
does not execute GPUI's next-frame callback, so automated coverage checks the
scroll request and its consumption rather than claiming rendered scroll geometry.

GPUI tests establish layout bounds and dispatched events; they do not establish
native text shaping, IME composition or Windows/Linux interaction. Native IME and
physical Windows/Linux acceptance remain unverified. No public release is
included.
