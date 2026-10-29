# Arrangement editor acceptance — 2026-10-29

Source: `0b69f70d0846351c1897dc2fa4c4245d38a4ed44` ([PR #34](https://github.com/yuxino/Force/pull/34)).

## Automated checks

- Canonical `scripts/check.sh` passed: format, strict workspace Clippy, 61 app
  tests, 125 core tests, two process-interruption tests and diff checks.
- Strict UI Clippy and all 257 feature/application tests passed. Local checks
  used the canonical macOS deployment target of 15.0.
- Arrangement regressions cover normal and 780 × 620 bounds, centered placement,
  expanded notes, validation/actions, explicit dates and today defaults, stale
  month caches, draft confirmation, activity save/reopen/cancel and archived
  activities. Task regressions preserve separate dirty drafts and enforce the
  existing completed-task reopen requirement.
- Core regressions preserve finished classification snapshots and exact interval
  identities/timing, and prove atomic rollback and stale-baseline rejection.

## Native macOS evidence

A signed Force Dev preview used disposable data, independently of production.
It exercised the centered wider editor, default today, ordinary `x` input,
expanded notes with visible actions, saving a new arrangement, changing its
activity from 工作 to 学习 and reopening it, and moving it to another date by
editing the date field.

The release-mode local app was signed, compared against the installed app's
complete designated requirement and bundle identity, installed at the existing
formal location, byte-verified against the build, and launched. The management
window showed the new 添加安排 entry. Production records were not edited for QA.

## Boundaries

Native resizing did not establish the minimum window size; minimum bounds and
scrolling remain TestPlatform evidence. Real IME composition and Windows/Linux
native interaction were not exercised. No public release was published.
