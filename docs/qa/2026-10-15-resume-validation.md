# Floating timer completion — 2026-10-15

Completed the interrupted floating timer UI pass on `feat/floating-focus`.
The compact strip is 328 × 68 points. The empty setup panel now uses 400 points:
at 360 points its primary start button was partly covered by the footer.
Existing-task setup pages retain scrolling and keep the primary button visible.

Interaction changes preserve unfinished task drafts when an older timer ends,
clear hidden input focus when changing pages, and only handle Return on the
visible input's page. Starting a break now pauses focus and settles old rest
sessions in one transaction, reusing an existing break without resetting it.

## Validation

- `scripts/check.sh`: formatting, strict Clippy, 4 application tests, 40 core
  tests and whitespace checks passed. Failure-path tests assert active timers
  as well as completed records, including a rejected break insert after earlier
  transaction updates.
- Signed macOS development bundle built and passed strict signature verification.
- Native checks on Apple Silicon covered zero/one/two recent tasks, long titles,
  list scrolling, setup, start, pause, resume, finish, completion actions, records
  and settings. The empty setup page's full start button was verified after the fix.
- Repeating the break menu action retained the same session and remaining time.
  A new draft survived ending the previous timer; switching to Records and pressing
  Return did not start the hidden task input. Dedicated fixture database inspection
  confirmed one focus session, one break and the expected interval boundaries.
- Quitting and reopening the test data restored the paused timer at the same
  remaining time; resuming and finishing continued that session.

Native acceptance used generated tasks in disposable data directories. The QA
launcher was pinned to its data directory because a UI observation after quitting
can automatically relaunch a bundle without its original arguments. One such
relaunch opened the existing development store; no timer or record action was
performed in that instance, and it was stopped before continuing isolation checks.
The installed development and preview apps remained running.

## Boundaries

Windows/Linux runtime interaction, minimum macOS version, external displays and
long-duration real usage were not tested. The repository has no configured remote;
this work was not pushed, merged or released. The cross-platform CI configuration
exists but has not run for this local commit.
