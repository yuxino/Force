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
