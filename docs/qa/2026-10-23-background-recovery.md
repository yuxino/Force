# Background recovery and task-start checks — 2026-10-23

Application revision: `5d438df`, based on `7f4fbed`. This report adds no
application changes.

## Confirmed fixes

- A temporary damaged database, recovery and an actual test-platform click on
  Retry reproduced goal-draft loss. The repaired path preserves both valid and
  invalid edited text while an untouched field adopts the recovered saved goal.
- A second SQLite connection updating the selected task reproduced an older
  window overwriting its title and tag on Start. Start now reads the saved task
  without an implicit metadata write. The event regression verifies the saved
  task, running session, selected tag and visible input value.
- Injected session/interval insert failures reproduced orphaned new tasks after
  a failed start. Task creation and the first round now share a transaction,
  including switching prior timers. Tests verify rollback preserves existing
  focus/rest, invalid duration and clock rollback save nothing, and retry works.
  Repeating a legacy round uses the same atomic path.
- Unit/headless timer completion no longer calls the OS sound APIs. Normal app
  completion sound remains governed by the existing preference.

## Verification

- Canonical `scripts/check.sh`: formatting, strict Clippy, 79 default tests and
  diff checks passed. Strict Clippy with `ui-test-support` passed; its 46 tests
  include the 23 default application tests, giving 102 distinct tests overall.
- The suite exercises the actual Force view tree with simulated clicks, typing,
  scrolling, prompt choices and default/minimum sizes. It covers focus, records,
  settings, task switching, rest return, drafts, validation, storage retry, date
  selection, search state and menu placement. Data comes only from temporary
  synthetic stores. The new failure regressions were observed failing first.
- Independent final review found no additional actionable regression. Long-text,
  minimum-width, calendar and tag-menu inspection did not justify visual changes.
- Builds used the canonical target directories, deployment target 15.0, reduced
  build concurrency and low process priority. Current signed release-mode
  development and production bundles were created without opening windows.

## Local installation and limitations

The formal bundle was atomically replaced after checking its identifier and full
signing requirement match the existing installation. Installed signature checks,
executable hash comparison and background `--version` passed. Executable SHA-256:
`09eb2a4a67081f6df1669a4275316d4f6c81e2c1debacdc531229d741c8b7023`.

The user required uninterrupted video playback. No native preview window was
opened, no OS pointer/keyboard input was sent, and the existing formal process was
not quit or restarted. The new binary applies on the next normal launch.

No new screenshots were taken: GPUI 0.2.2 TestPlatform does not render pixels and
this setup has no isolated graphical session. Headless events establish neither
native appearance nor real keyboard/IME behavior. macOS visual acceptance and
Windows/Linux native interactions remain unverified. Production data was not
opened for acceptance. No public release was published and no remote push was
attempted in this continuation.

Current release bundles replace the previous development/packaging copies.
Temporary test databases are removed by the test harness. The previous formal
bundle remains only while its executable is used by the existing process.
