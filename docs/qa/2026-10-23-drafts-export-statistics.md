# Drafts, CSV and statistics — 2026-10-23

Application revision: `53874c8`, based on `b3e16be`. This report adds no
application changes.

## Changes and verification

- Completing an inactive task clears its form without prompting to save the old
  title as a new task. The regression covers tasks with and without finished
  records and verifies that history remains intact.
- The initial new-task form protects drafts before New, task selection and global
  rest. Cancel retains the draft; Save stores it without starting a timer.
  Both new event regressions failed before their respective fixes.
- CSV snapshots cap expired countdowns at the exact unpaused deadline without
  changing stored sessions. A 60-second timer previously exported 900 seconds
  after a delayed tick; it now exports 60 seconds with the correct completed
  status and end timestamps. Tests cover pauses, later intervals, early manual
  completion, unexpired timers, stopwatch mode and equivalence with real settling.
- Today reuses one statistics snapshot for both summaries. Historical dates still
  load separately. The regression includes live timing and distinct daily totals.
- `scripts/check.sh` passed: formatting, strict Clippy, 77 default tests and diff
  checks. Strict Clippy with `ui-test-support` passed. Its 44 application tests
  include the 23 default application tests: 98 distinct tests overall.
- Independent final diff review found no additional actionable regression.

## Statistics measurement

Optimized current core code, existing release dependencies, synthetic local
SQLite stores, 20 warmups and 41 alternating AB/BA rounds; each sample runs 20
operations. Both variants returned identical values.

| Single-day fixture | Two reads, median | One read and reuse, median |
| --- | ---: | ---: |
| 1,000 intervals / 100 sessions | 394.83 µs | 208.69 µs |
| 10,000 intervals / 1,000 sessions | 3,170.06 µs | 1,533.86 µs |

This reduces the measured statistics path by 47–52%, with 4 SELECTs / 2
transactions becoming 2 SELECTs / 1 transaction. It does not establish overall
UI or CPU improvement. Concurrent local work caused tail-latency variation.

## Local delivery and evidence limits

The signed development and optimized production bundles built successfully.
The formal installation was replaced by an atomic bundle exchange after matching
its bundle identifier and complete designated requirement. Strict installed
signature verification and background `--version` passed. Installed executable
SHA-256: `3b4ceaf16f7fa1a991f0730a41f2202dcec5f99f879be7cf7a6aee8060688b76`.

The user requested that testing not interrupt video playback. Interactive native
acceptance was stopped; the running formal process was neither quit nor relaunched.
The new binary takes effect on the next normal launch. Native acceptance of the
changed flows, real IME and Windows/Linux interactions remains outstanding;
headless tests are not evidence of native pixels or keyboard composition.
Only disposable stores were used for testing. Production records were not opened
or modified for acceptance. The previous bundle is retained while its executable
is still in use; disposable benchmark/data files and proven orphaned fixture
caches are removed. No public release was published.
