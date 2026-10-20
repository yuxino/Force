# Force workspace and activity calendar — 2026-10-19

Base: `967d498`; current behavior is documented in
`docs/plans/interface-consistency.md`. This revision adds the real activity
calendar and corrects internal layout regressions. Work stays in the background
without controlling the screen or restarting the visible app. Formal replacement
is authorized with the existing identity, signing requirement, path and data.

## Current changes

- Records begin with 53 weeks × 7 days of real focus activity. Gray levels use
  absolute duration: zero, positive below 30 minutes, 30–59, 60–119 and 120+
  minutes. Tooltips show date/duration; clicking a day updates the records below
  without scrolling the grid away. Future cells are disabled and visually distinct.
- The model loads the series with one batch query over local-day boundaries,
  merging focus intervals and excluding rest/pause. It caches the result and
  updates today's cell on normal timer ticks instead of querying each cell.
- Reproduced a task-title width collapse to approximately 47 points inside a
  full-width sidebar button. The cause was the button's auto-width content
  wrapper, which outer-row containment tests did not detect. The text column now
  has an explicit 188-point width, and regression checks inspect actual internal
  title/status bounds.
- Date controls sit beside the date; the task form aligns with its heading.
  Sidebar separation uses whitespace instead of a full-height line. Compact
  durations show seconds below a minute and omit zero minutes after whole hours.
- The independent strip and 1040 × 780 management window retain the stable
  summary, current-operation area and task navigation, including the 780 × 620
  minimum size. Task selection does not start timers, starting preserves window
  visibility, and the strip continues to prioritize the running timer.
- Settings remains secondary, pinning lives in preferences, and records owns the
  only export action. Disabled controls retain explicit readable colors without
  whole-element opacity.

## Current revision verification

- `scripts/check.sh`: passed formatting, strict Clippy, 64 default tests
  (20 application, 42 core, 2 interruption cases), and whitespace checks.
- `cargo test -p force-app --features ui-test-support --locked`: all 27
  application cases passed, including 7 headless UI cases. Ordinary application
  cases overlap the default suite; the combined verification has 71 distinct tests.
- Default and minimum windows both passed internal short/long Chinese task-title
  width checks. The regression failed before the fix at 47 points; it passed
  after the production column used an explicit 188-point width.
- All 371 calendar cells fit within the grid at both window sizes. Real synthetic
  clicks changed date, daily totals, model history and newly rendered record-row
  nodes. The selected day and grid retained a nonzero scroll offset. Future-day
  disabled clicks and the model date guard both passed. Date arrows remained
  adjacent to the date label.
- Core/model regressions passed batched merged-focus totals, cross-midnight
  allocation, pause/rest exclusion, open intervals, empty/future ranges, settled
  timed sessions, local-day boundaries and 53 complete Monday-to-Sunday weeks.
- Existing workspace stability, keyboard/input, disabled-button contrast and
  shared-model two-window regressions passed.
- `cargo clippy -p force-app --all-targets --features ui-test-support --locked
  -- -D warnings`: passed. Verification used isolated temporary stores only.
- `scripts/build-macos.sh --release --production`: optimized build and strict
  signature verification passed with the existing Apple Development identity.
- Atomically replaced `/Applications/Force.app`; verified installed bytes,
  unchanged `app.yuxino.force` bundle identifier and designated signing requirement.
  Installed executable SHA-256:
  `9e854d9c2a19ea590c5156c4fce79bcb663ef821e0e0c91edf1ebf208438c1ab`.
  `--version` returned `Force 0.1.0`. The existing process remained running;
  new UI takes effect on the user's next relaunch.
- Removed temporary editing/install scripts and the duplicate packaged bundle.
  Preserved source, installed app, reusable Cargo caches and this evidence.

## Evidence limits

GPUI TestPlatform measures layout and dispatches synthetic events without using
the user's screen. It does not establish native pixels, real hover/pressed
appearance, pointer behavior, IME composition, dialog return, activation or
window-manager behavior. This revision has no native visual acceptance.

Production records are excluded from QA. Windows/Linux native builds and
interaction remain unverified locally; Linux management close minimizes the
window and restoration still needs native acceptance. No commit, push or public
release is included in this work.
