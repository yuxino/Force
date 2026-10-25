# Daily calendar

Force opens on a calendar for planning what to do at specific times and reviewing
what actually happened. Calendar and Records are the primary destinations;
Task/timer details remain available from the current activity and recorded events,
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

Month selection supports future dates, with real planned-day and activity marks.
The selected day's clock grid has proportional time placement and readable
overlap lanes. A compact editor handles title, date, times, label and note;
existing linked-task titles are preserved. Date/selection changes protect drafts,
and ordinary navigation or hiding preserves them. Starting a plan uses existing
transactional task switching and keeps a paused round's recorded target.

No external calendar sync, inferred completion in other applications, reminders,
background automatic starts, accounts or network services are introduced.

## Validation

Canonical checks and strict UI-enabled Clippy pass. The calendar change has
76 core unit tests, 2 interrupted-process integration tests, and 83 app tests.
UI regressions cover minimum/default bounds, hour-slot creation, draft prompts,
related actual timestamps, completion, rescheduling and local overlap widths.

Signed native macOS acceptance used disposable data: ordinary keyboard input,
create/save/restart, start/end timing, complete a task and return to the calendar,
move an arrangement to tomorrow, and inspect default/minimum window layouts.
The formal app retains the same bundle and signing requirement. GPUI tests do
not establish native IME composition or Windows/Linux interaction; those remain
unverified. No public release is included.

The month sidebar pairs date navigation with the current timer, next arrangement
and completion progress. Empty clock slots create an arrangement directly.
Scheduled blocks retain their planned times; related actual intervals also remain
visible at their real times. Starting an arrangement keeps the calendar visible.
Use ordinary arrangement, timer and record wording; do not introduce a focus mode.
