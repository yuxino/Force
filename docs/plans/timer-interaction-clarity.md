# Consistent timer interactions

Keep the existing two-window layout, neutral styling, tasks and durable session
model. A task can have several saved timer records; pausing and resuming retain
the same record, ending saves it, and repeating creates a new one.

Use the same terms across management, floating strip and native menus:
开始计时 / 暂停计时 / 继续计时 / 结束本次计时. Completion says 本次计时已保存,
and its repeat action is 再计一次. Completing a task removes it from the task
list while retaining records; explain this with contextual help.

Preparation identifies 新建任务 or 任务设置, and 计时方式 offers fixed countdowns
or 不限时. Countdown numbers explicitly identify 剩余时间; unlimited timers
identify 已用时间. Sidebar durations distinguish 本次 from 任务累计.

Browsing another task or creating one never starts it or pauses the current
one. Show the actual running task with a shared 返回计时 action when browsing
another task, records or settings. Returning selects the current timer without
changing its intervals. Starting or resuming a different task explicitly says
切换并开始计时 / 切换并继续计时; existing atomic switching preserves prior time.

Acceptance covers default/minimum widths, navigation back from records/settings,
new-task browsing with a running timer, switching both ways, pause/resume,
completion/repeat/rest, and the same terms on the strip and native menu. Use
only isolated development data. GPUI layout tests do not establish native pixels.

## Reduce ambiguous choices

Task metadata saves independently of starting a timer. Show 保存/撤销 only for
changed existing tasks, and protect unsaved changes when selecting another task.
New-task Return starts; changed existing-task Return saves without starting.
Completing an inactive task uses the same 完成任务 action as the timer receipt.

Rest has two controls: continue the original focus task (or end rest if none),
and pause/resume rest. Its receipt describes rest, never the next task's time.
Global rest follows the actually running task; a local rest action follows the
viewed task. With no return task, end rest before opening a new-task form.
Keep legacy standalone rest reachable but label it as rest, not a focus task.

Only create focus tags in settings; retain existing rest tag data and editing.
Goal actions appear only when changed. Record expansion adds dated boundaries,
pause time when present, and mode; it does not repeat the row's title, tag and
durations. Expanded titles wrap so long names remain readable.

Routine feedback uses one compact transient toast without shifting the workspace.
Task/goal saves are quiet on success; values and edit actions update in place.

## Failed starts and concurrent edits

Starting an existing task reads its saved title and tag; it must not implicitly
save an older form snapshot over another instance's edits. Explicit task Save
remains the metadata-edit action. New-task creation and its first timer round
share one transaction, including pausing existing focus and ending rest. Any
failure preserves the draft and previous timers without leaving an empty task.
Apply the same rule when repeating a legacy round without an associated task.
