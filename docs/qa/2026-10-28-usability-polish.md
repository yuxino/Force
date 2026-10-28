# Usability polish acceptance

## Changes

- Task operations and the task list have independent fixed scrollbar rails in
  their existing gutters. Each rail owns a distinct element ID and is usable on
  hover, including after the scrolling animation fades. The summary and list
  heading remain fixed; task text keeps its existing width.
- The day timeline has fixed horizontal and vertical scrollbars.
- Analysis bars and daily rows open that date's records through the existing
  date/navigation methods. Search, classification filters and suspended drafts
  retain their existing behavior. Daily rows provide the keyboard entry point.
- Analysis summary columns use the actual content width. Table headings and
  values share explicit column widths, including the trailing disclosure arrow.
- The cat uses a short setup hint; the expanded hint remains specific. Docked
  progress appears only for a real countdown target.

## Evidence

- The final isolated branch passed all 212 feature-enabled app tests. Focused
  cases cover both window sizes, column alignment, long totals, navigation with
  drafts and filters, unchanged record CSV, both timeline edges and independent
  scrollbar dragging.
- The drag regression failed with shared scrollbar IDs and passed after each
  rail received its own ID.
- Canonical checks and feature-enabled Clippy passed before the final analysis
  sizing and hover refinements. The integrating branch must rerun them after
  combining the concurrent activity and record-editor changes.
- A signed macOS development bundle with synthetic data was launched. Native
  checks confirmed full-width analysis summary rules and aligned table columns,
  bar navigation to October 27 and row navigation to October 26, ordinary
  typing/backspace, task-list wheel scrolling, direct track clicking and thumb
  dragging while the left draft and summary remained in place.

## Boundaries and integration

Native checks used the default window size. Minimum-size, long-number and
timeline-edge evidence here is TestPlatform coverage; it does not establish
native minimum-size, IME, Windows or Linux acceptance. Docked cat rendering also
remains part of combined native acceptance. No production records were used for
these tests and this branch did not replace the formal application.

Concurrent checkouts must serialize use of a shared Cargo target directory
through build, test execution and bundle copying. Cargo's individual build lock
does not protect a test executable or binary after another checkout rebuilds
the same target. The integrating chat owns the shared target and final signed
installation after this branch's preview exits.

## Production packaging verification

`--production` must force release mode: debug binaries intentionally select the
development data directory, even inside a bundle named Force.app. A signed bundle
and matching identifier alone do not establish a production build. Verify the
running executable opens the production database and restores its active session
after installation.
