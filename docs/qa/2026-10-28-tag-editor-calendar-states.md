# Tag editor and calendar appearance acceptance

## Changes

- Tags open a compact editor only when adding or editing. Actions stay below
  the fields; save/cancel collapse the editor and invalid drafts remain visible.
- Calendar month rows, day summaries and time blocks use the current tag color.
  Clearing a live color clears the display; archived tags use available session
  snapshots. Historical names, classifications and totals are unchanged.
- Visible states distinguish running, paused, recorded, completed, pending and
  moved. A finished round is not automatically a completed task. Current timer
  state does not leak into historical dates.

## Verification

- `scripts/check.sh`: formatting, strict workspace Clippy, 51 application tests,
  85 core tests and two process-recovery tests passed.
- UI feature strict Clippy and all 182 application/UI tests passed. Bounds cover
  1040 × 780, 780 × 620 and the settings breakpoint, including title/status
  children, compact controls and nested scroll handoff.
- Signed native macOS development app, dedicated synthetic data: added and
  edited tags, saved/cancelled drafts, ordinary key editing, confirmation cancel
  followed by typing, and window reopening with the draft preserved.
- Native calendar at 1040 × 780: colored titles/marks, running/paused/recorded/
  completed/pending/moved states, long-title truncation, day popup and time grid.
  Changing the test tag color updated past and current calendar entries; clearing
  it returned those entries to neutral. Archived snapshot color stayed visible.
  Popup opening no longer leaves the underlying month tooltip over its content.
- A second independent source review found no remaining supported defect.

## Boundaries

The native checks used macOS and synthetic data. Minimum-window geometry is
covered by TestPlatform, not a native minimum-window screenshot. Windows/Linux
native rendering and real IME composition were not exercised. No user records,
schema, public release or update publication changed.
