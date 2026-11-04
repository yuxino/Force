# Unified creation acceptance — 2026-11-04

Source: `5b947e9cfd7a66022ca7ecc1577a7d4627f3f664`.

## Automated evidence

- Canonical `scripts/check.sh` passed: formatting, strict workspace Clippy,
  62 app tests, 125 core tests, two interruption tests and diff checks.
- All 292 UI/application tests and strict UI-feature Clippy passed.
- Regressions cover four creation types at 1040 × 780 and 780 × 620,
  independent drafts, first-entry field/date transfer, existing-object edit
  protection, hidden-window reopening, focus, independent running timers,
  storage-failure rejection and single-line Return handling. Expanded-note
  and error cases use real scrolling with fixed footer assertions.

## Native macOS evidence

Signed Force Dev used disposable data. The native window showed the single
header entry and all four types, carried activity/description between fresh
forms, preserved drafts, accepted ordinary keyboard input, and saved a note
and a 30-minute backfill with separate calendar representations. The final
build also rejected Return without an activity while preserving the draft
and usable picker. Preview evidence is kept outside Git.

Signed debug and release bundles were rebuilt from the source above. The new
production bundle's identifier and complete designated requirement match the
installed app. Strict signature verification passed with access to the local
certificate trust state.

## Delivery and evidence limits

The user explicitly approved merging to main and updating the formal app. The
feature was fast-forwarded into local main. The release bundle replaced the installed
app at its existing location with the same bundle identity and complete signing
requirement. Strict installed-signature and executable byte checks passed. The
updated native app launched, showed the unified entry and four types, and the
pre-existing task draft was restored without saving or starting a timer.
Production records were not written for QA; no public release was published.

The initial pushes were blocked by GitHub internal server errors during its
[Git Operations incident](https://www.githubstatus.com/incidents/smtdswd1h2w4).
A later retry succeeded and live remote main matched `aed3750`. The user then
explicitly requested publication. Version 0.1.5 retains the verified application
changes; release notes are in `docs/releases/v0.1.5.md`.

The disposable QA data, preview bundle, temporary logs, obsolete incremental
state, installation rollback and duplicate release bundle were removed after
verification. Source, current build dependencies, installed app and external
preview evidence remain.

Native resizing did not establish the minimum size; minimum-layout evidence is
TestPlatform-only. Real IME composition and Windows/Linux native interaction
were not exercised.

## v0.1.5 publication

Release source: `a4250bdbb78ab4557dcc1727a916a44f952d960b`. Canonical local
checks and all 292 UI tests passed again with version 0.1.5. Main and tag CI
passed on macOS, Windows and Linux.

[Release workflow](https://github.com/yuxino/Force/actions/runs/41820931354)
succeeded and published [v0.1.5](https://github.com/yuxino/Force/releases/tag/v0.1.5)
as the latest stable release. Downloaded all four packages; every SHA256 matched
SHA256SUMS, Windows/Linux archives were readable, both DMGs verified, and both
Mac bundles had version 0.1.5, the expected identifier and valid signatures.
The published bilingual notes match the repository source. This does not add
Windows/Linux native-interaction or real IME acceptance.

The installed local app was also updated to 0.1.5 with its existing signing
requirement, byte-verified and launched. Its original task draft was restored
without saving or starting a timer. Installation staging, duplicate bundles,
downloaded verification packages and task-only logs were removed afterward.
The merged task branch was removed after verifying remote main, worktree use
and the absence of an open task PR.
