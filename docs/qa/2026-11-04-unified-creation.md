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

## Pending delivery and evidence limits

Automatic approval review requires confirmation before merging/pushing this
feature to main under the project's substantial-change rule. It also rejected
quitting the installed app because unsaved drafts are not proven to persist
on exit. The feature branch and verified production bundle are retained for
those next steps. Production records were not written for QA; no public release
was published.

Native resizing did not establish the minimum size; minimum-layout evidence is
TestPlatform-only. Real IME composition and Windows/Linux native interaction
were not exercised.
