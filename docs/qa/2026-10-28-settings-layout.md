# Settings layout and tag colors — 2026-10-28

Based on `2f69ba4`. Settings now use timing/application and tag columns,
stacking below 920 points. Application actions and update results have distinct
aligned rows. Saved tag names and classification labels follow their color;
the selected classification previews a draft color before Save.

## Verification

- `scripts/check.sh`: formatting, strict workspace Clippy, 134 default tests
  and whitespace checks passed. Feature-enabled strict Clippy and 172 app/UI
  tests passed (259 distinct tests including core/recovery coverage).
- Layout regressions cover 1040 × 780, 780 × 620, 1040 × 620 and 1512 × 865,
  update success/failure/long-version states, and full action bounds after
  outer scrolling. Color tests cover white, selected, hovered and pressed
  surfaces at 4.5:1 or greater text contrast.
- A disposable browser prototype and signed native development app used
  synthetic tags including a long Chinese name. Native checks covered default
  and minimum sizes, editor auto-reveal, class/color preview, Save updating
  the list, Cancel retaining saved values, ordinary keyboard input and typing
  after cancelling a draft-switch confirmation. A live update check displayed
  its result beside the action.
- The signed release replaced the local formal app with identical designated
  signing requirements. Installed signature, matching binary hash, normal
  launch and the new settings composition were verified. User records were
  not edited for acceptance. No public release was published.

Installed executable SHA-256:
`e32f0a812513e42cd41b84bf194012556e9fe99b350be4f91301e328883e9c3c`.

Windows/Linux native appearance and new actual IME composition were not tested.
Headless geometry and dispatched events do not establish those behaviors.
