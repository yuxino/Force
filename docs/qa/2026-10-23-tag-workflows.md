# Tag workflows and quiet UI checks — 2026-10-23

Application revision: `397670b` (interface/core changes in `a6ff598`), based on
`94dc8b6`. This report adds no application changes.

## Confirmed changes

- Archiving a tag no longer prevents continuing an existing paused task round.
  The same record, target and tag snapshot survive; starting a new round still
  requires an available tag. The regression failed with `ArchivedTag` before
  the fix and covers countdown/stopwatch, switching and later reassignment.
- Reopening the same tag preserves its input. Switching to another edit target
  protects new and renamed drafts with Save, Discard and Cancel. Invalid saves
  preserve the draft and destination; stale prompt replies cannot replace newer
  input. Comparing against the original edit value prevents an external rename
  from becoming an apparent user edit or being overwritten by an untouched field.
- Adding a tag no longer changes a saved task's selection. Deleting its tag
  retains that reference until the user explicitly picks another; it does not
  silently assign a default or create a false unsaved-change prompt. The first
  three tag draft/creation regressions were observed failing before repair.
- Task buttons and Return use the same title and available-focus-tag validation.
  Empty existing names, excessive length and NUL characters show field errors;
  untouched empty new inputs remain quiet. The name placeholder no longer
  implies Return always starts a timer.
- Running sidebar statuses use stronger neutral text. The tag being renamed
  uses the existing neutral row surface. Search with no matches offers a compact
  clear action that restores records and keyboard focus. Existing geometry and
  the black/white/gray visual system remain in place.
- macOS checks now use the same deployment target 15.0 as signed builds and CI,
  avoiding configuration-driven dependency rebuilds between those entry points.

## Verification

- Canonical `scripts/check.sh`: formatting, strict Clippy, 80 default tests
  (23 application, 55 core, 2 interrupted-process) and diff checks passed.
- Strict Clippy with `ui-test-support` and all 55 application tests passed. That
  suite includes the 23 default application tests, so the combined total is
  112 distinct tests. This round adds 9 UI regressions and 1 core regression.
- Default/minimum layout and actual TestPlatform event dispatch cover the tag
  choices, archived-tag pause/resume, validation, empty-search recovery, keyboard
  focus, and 44-point editing rows. At 780 × 620, wheel scrolling places Save and
  Cancel inside the visible viewport before clicking them. All data is synthetic
  and temporary; OS completion sounds remain disabled in test builds.
- Independent review covered asynchronous draft guards, external edits, task
  selection, archived snapshots, clock/target validation and transaction rollback.
- Canonical output directories were reused with low priority and two build jobs.
  Production and development release-mode bundles were signed without opening
  a native window.

## Installation and evidence limits

The formal bundle was atomically replaced after verifying its bundle identifier
and complete signing requirement match. Installed signature, binary hash and
background `--version` checks passed. Installed executable SHA-256:
`0595c89207a6bfccc2cfd058b97677c65b52247c7fc255291d55b696a340fd58`.

The existing formal process was neither quit nor restarted. The new binary will
be used on the next ordinary launch. Its still-running older bundle is retained;
the unused replaced bundle was removed after checking file use. Temporary test
databases are removed by the test harness. No public release was published.

The user required uninterrupted video playback. No native windows were opened
or activated, and no OS pointer or keyboard input was sent. No new screenshots
were taken: GPUI TestPlatform has no native pixel renderer. Native appearance,
real IME, window behavior and Windows/Linux interactions remain unverified.

The test platform also ignores native frame callbacks. Its failure to execute
`ScrollAnchor` is a test-backend limitation, not evidence of a native scrolling
bug. The existing production auto-scroll path was retained; the tests establish
wheel scrolling and viewport/action bounds, not automatic scroll targeting.
