# Settings scroll acceptance — 2026-10-28

The tag list previously moved together with the page, and its scrollbar moved
with the rows. The scrollbar also overlapped the right edge of delete actions.

The list now retains its 240-point height limit, reserves a 20-point gutter,
and scrolls independently until an edge. The following wheel event can then
scroll the surrounding page. Both tag and page scrollbars live outside their
translated content.

Validation:

- `scripts/check.sh`: formatting, strict workspace Clippy, 134 tests and diff checks passed.
- UI feature: 175 app tests passed (including 47 also covered above); strict UI-feature Clippy passed. Three new regressions cover 13-tag lists at 1040- and 780-point widths, both edge handoffs, fixed track click targets, action clearance, and short-list wheel forwarding.
- Signed native macOS app, dedicated synthetic data with 16 tags, 1040 × 780 content window: reproduced the original simultaneous scrolling; confirmed independent middle scrolling, top/bottom handoff, track click to bottom, thumb drag back to top, and edit/cancel after scrolling.
- Native minimum-size resizing was not established through the automation tool in this pass; its geometry and dispatched events are covered by TestPlatform. No new IME or Windows/Linux native acceptance is claimed.

No production records were used for acceptance. The change does not alter
storage or saved tags.
