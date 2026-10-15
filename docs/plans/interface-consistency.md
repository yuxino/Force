# Floating utility visual rules

Preserve the 328-point timer strip and same-width panel. Use the existing system
font, black/white/neutral gray surfaces, and red only for destructive actions and
errors. Do not introduce a dashboard, decorative cards or a brand color.

- Use 13-point control text, 12-point section labels and 11-point metadata.
  Timer figures use tabular digits; task headings use 16 points.
- Single-line fields are 32 points high with a 6-point radius. Set their styled
  height explicitly: `Input::h` affects multiline editors only.
- Standard icon hit areas are 28 points with 14–15-point glyphs. The strip's
  primary action remains 30 points. Primary panel actions are 34 points high.
- Align content to a 16-point inset and use 12–16-point section gaps. Reuse the
  same navigation selection, field, button and disabled-state helpers.
- Keep setup, running and completion panels proportional to their contents.
  Scroll long task/tag lists inside the panel; keep the strip and footer visible.
- Records separate the total, record count, search and rows. Align export to the
  trailing edge rather than presenting another centered primary action.
- `gpui-component` 0.5.1 hardcodes red button hover text. Give button labels and
  icons explicit semantic foregrounds, and dim unavailable controls as a whole.
  Do not register another button hover handler: GPUI rejects duplicate handlers.
  This avoids a dependency change or modifying the shared package cache.

Verify the signed native app with disposable data: empty/populated/long-content
screens, input and hover/pressed/disabled feedback, dialogs, scrolling, timer
states, panel resizing and keyboard actions. Preserve the text-input responder
and deferred keyboard callback fixes. macOS evidence does not accept Windows or
Linux interaction.
