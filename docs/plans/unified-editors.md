# Unified creation dialogs

The header's 随手记 and 新任务 actions and the calendar's 添加安排 action use one
centered editor shell above the current page. Keep the 68-point navigation
available; only one editor is visible at a time. Reuse a 560-point width bounded
by the window, a common header/close action, 40-point fields, scrollable content,
a fixed validation area and a right-aligned footer. The minimum window keeps
all actions visible. Activity and time menus remain above the normal paint pass.

Each editor owns its draft. Close, outside click, Escape and navigation suspend;
explicit Cancel discards. Switching editors suspends the previous editor and
reopening restores its fields. A picker handles Escape before its parent.
Keyboard focus stays in the editor while tabbing and returns to the page on close.

New tasks open independently of task/timer selection and default to count-up
without an activity. Save creates an unscheduled task and opens its task details without starting or
pausing a timer; Start uses the existing transactional
switching path and protects unsaved task edits. Existing task/timer details stay
in their contextual workspace. Point notes never count as work. Arrangement
creation keeps today's default and explicit-date contextual actions.

Verify the same three entry points and switching paths with temporary data at
1040 × 780 and 780 × 620, then inspect the signed native app. TestPlatform bounds
and key events do not establish native pixels, IME or other-platform interaction.

## Edge cat peek

The dock is a 48 × 100-point transparent cat peek, with the existing full-body
cat slightly cropped against the chosen screen edge. Keep the free 88 × 128 and
expanded 244 × 152 shapes. Remove the enclosing panel, portrait thumbnail and
vertical progress stripe. Put a compact duration and explicit unit below the
paws; reserve a stable status slot for pause or saved marks and place countdown
progress beneath the readout. Idle keeps the visible 开始 action. Preserve actual
paused/saved duration and error priority.

Hover stays quiet. Click opens the existing controls toward the inside of the
screen while the edge cat and its readout stay in place, or opens a new task
when idle. Dragging away restores the free cat; leaving the right expanded shape
preserves its right anchor before clamping to the screen. Keep each rendered shape explicitly sized
while native resize is pending. Native resizing must update content size and
GPUI's content view, discard stale queued requests, and preserve the input
responder. Verify both screen edges, real drag/release, expand/collapse and a
return to the free shape with actual native frame/content/viewport measurements.
A TestPlatform resize or a dock child painted inside a free-size window cannot
establish this behavior.
