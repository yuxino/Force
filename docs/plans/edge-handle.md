# Compact edge handle

The dock acts as a small handle rather than a second floating cat. Idle and
prepared tasks show a 20 × 24-point transparent cartoon paw grip. Running,
paused and saved rounds show a 44 × 24 neutral edge tab with one short time,
explicit 分/时 unit and a reserved pause/saved mark. Failure and invalid duration
show a short action instead. Do not stack a paw, time and Start button.

Clicking opens controls inward with an 8-point gap. Keep the handle's top and
screen-edge position fixed while expanding; use the existing native resize queue.
The dock's expanded viewport is 216 × 104. An idle menu contains only 新任务 in
40 points of height. Task controls use a 164 × 96 panel with title, status,
complete clock and the existing pause/resume/end/repeat actions. Keep long names
truncated with full-title help and include complete elapsed or remaining time.
Hover does not expand the dock. Escape and the handle close it without changing
the timer. Dragging away restores the free cat, retaining the right anchor when
leaving a right-hand expanded panel. Free cat layout and idle click remain intact.

Use one semantic presentation class for both layout and native dimensions.
Cache the class, not the number of elapsed seconds: normal ticks repaint without
resizing. Shape changes reshape only a collapsed dock; an open or closing panel
uses the current class when it finishes closing. The native minimum is 20 × 24.

Verify both edges and idle, prepared, countdown, count-up, paused, saved, invalid
and error states. Check long clocks/99+ hours, rapid toggles, presentation changes
during disclosure and actual native frame/content/viewport agreement. Use isolated
data for interactions; headless bounds alone do not prove native pixels or IME.
