# macOS keyboard input — 2026-10-15

Fixes applied to the GPUI app based on main `7e05f25`:

- Removing the titled window style reset AppKit's first responder from GPUIView
  to GPUIPanel. Restore the GPUI view after styling and fail configuration if
  AppKit rejects it. Before the fix, the caret and paste worked while ordinary
  typing did not; native responder inspection confirmed the reset.
- GPUI's custom view does not opt into `needsPanelToBecomeKey`. Allow clicks to
  make the nonactivating panel key, including after native dialogs. Without this,
  closing the save panel required collapsing/reopening Force before typing worked.
- Global keyboard callbacks tried to update the window already borrowed by key
  dispatch. Defer handle-based access; this restores the new-task, timer, settings
  and panel shortcuts without duplicating the native-menu action implementation.

## Native checks

Signed development app on Apple Silicon, using generated tasks and a dedicated
disposable data directory:

- Ordinary Latin letters, numbers, spaces, selection, cursor movement, deletion,
  Unicode clipboard text and Return to start a task.
- Task, goal, tag and record-search inputs; goal save and invalid-zero feedback;
  tag creation and rename; matching and empty search results.
- Start, pause, resume and finish; completion panel and saved record; a paused
  timer retained the same remaining time after quitting and reopening.
- CSV export produced a readable file with the generated task; canceling native
  save and tag-removal dialogs allowed typing immediately afterward, without
  collapsing the panel.
- Command-N opened task setup and focused its input; Command-P paused;
  Command-comma opened settings. Escape, Command-Shift-M and Command-Q were also
  exercised. Search-page Return did not start the hidden task draft.
- Settings values and renamed tags persisted across restart.
- The signed optimized bundle was installed in place with the same designated
  requirement. In the installed bundle, Command-N focused setup and ordinary
  keyboard typing displayed the expected text using the disposable store.
- Native menu action “休息 5 分钟” still started a break after deferring callbacks.

The input repair does not change storage, timing or portable business rules.
Full macOS checks cover formatting, strict Clippy, 4 app tests, 40 core tests and
diff whitespace. Native focus behavior is covered by the interaction checks
above, not by the Rust unit tests.

Physical Chinese/Japanese IME composition, Windows/Linux interaction, external
displays and long-duration use are not claimed by this run. Unicode clipboard
text is not an IME-composition test.
