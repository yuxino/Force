# Unified creation dialog

The management header has one 新建 action (Cmd/Ctrl+N). Its centered dialog
contains 任务, 安排, 补记 and 随手记 tabs. Calendar day/timeline 新建 actions
open the same surface with 安排 and the selected day. The month toolbar and
records page do not duplicate creation actions. The floating cat still opens
任务 directly; Cmd/Ctrl+Shift+N opens 随手记 in the shared surface.

Reuse a 560-point width bounded by the window, a common heading and close
action, 40-point fields, scrollable content, a fixed validation area and a
right-aligned footer. Keep the 68-point navigation available. Activity and time
menus remain above the normal paint pass. Existing-object editing keeps its
specific title and does not show creation tabs.

Each type retains its own draft. Close, outside click, Escape and navigation
suspend; explicit Cancel discards the current type. Switching types suspends
the previous editor. First entry to an unused type carries its activity and
short description; it never overwrites a retained draft. Long or multiline
notes remain in 随手记 rather than becoming invalid task titles. Dates follow
the visible source editor or selected record day; backfill dates cannot default
to the future. A new arrangement otherwise starts today. Opening 新建 again
while it is visible focuses the same form; after closing it, 新建 starts at
任务 with the retained drafts available through their tabs.

A picker handles Escape before its parent. Tab stays inside the editor and
focus returns to the page on close. Starting a new note or backfill while an
existing object has unsaved edits offers Save, Discard or Cancel; failed saves
and stale asynchronous responses preserve that edit.

Task defaults remain count-up with no activity selected. Save creates an
unscheduled task without starting or pausing a timer; Start uses the existing
transactional switching path. Arrangements remain plans, not recorded work.
Backfills create completed work over explicit past intervals without touching
running timers. Notes remain point markers and never add focus time. No schema
or accounting-policy change is required.

Verify all four types and switching paths with temporary data at 1040 × 780
and 780 × 620, including retained drafts, date context, no activities, validation,
existing-object edits and independent running timers, then inspect the signed
native app. TestPlatform bounds and key events do not establish native pixels,
IME or other-platform interaction.

## Edge handle

The small edge handle and time tab follow [the edge handle design](edge-handle.md).
Their new-task action opens the same centered editor and preserves suspended drafts.
Native resizing must update content size and GPUI's content view, discard stale
queued requests, and preserve the input responder. Verify both screen edges,
real drag/release and expand/collapse using native frame/content/viewport sizes.
A dock child painted inside a free-size window cannot establish this behavior.
