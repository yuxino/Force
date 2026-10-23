# Force workbench redesign

## Direction

Rebuild the visual composition around the task, the time and the next action.
The existing geometric F remains the brand mark. Neutral surfaces, strong type
contrast, aligned numbers, restrained rules and open lists give the small local
application a coherent identity. Do not add dashboards, account features,
illustrations, textures, gradients, marketing slogans or decorative cards.

The owner approved merging this substantial interface change after reviewing the
design. Preserve the running formal app and all production data during delivery.
Use short action labels and avoid repeating timer state in captions or log rows;
keep errors and meaningful distinctions visible.

## Shared language

- Keep the 1040 × 780 default / 780 × 620 minimum management window and separate
  328 × 68 floating strip. Keep the 896-point content grid, 40-point insets,
  32-point column gap and 240-point task column.
- Use the existing system font with tabular time digits. Page titles are 32,
  task titles 28, main timers up to 104 (80 when compact), section labels 16,
  body/actions 14 and secondary information 13 points. Long timers and titles
  must remain within the real available width.
- Use neutral ink, soft white, readable gray and fine gray dividers. Keep the
  existing destructive semantic color. Primary controls remain 44 points high,
  fields 40–44 and most corner radii 4–6. Hover, keyboard focus and disabled
  contrast remain visible.
- Put brand, primary navigation and secondary settings into one 68-point header.
  Keep page headings and settings content on the same reading axes.

## Surfaces

Focus keeps its daily summary and task column stable across preparation,
running, paused, rest and completion. A thin daily progress line ties the summary
together. The operation column shares a left-aligned task heading, time area
and action row. Countdown progress may use a restrained functional scale;
stopwatch time must not imply an invented target. Timing choices become compact
text choices rather than a large nested segmented container. Validation remains
beside its field; Save/Discard/Cancel behavior is preserved.

The task list uses continuous 64-point rows, subtle rules and a left selection
mark. Running status remains independent of selection. Preserve actual title and
status widths, tooltips, safe truncation, one New action and independent scrolling.

The strip gives its title a full-width first line, then time, state and fixed
positions for pause/resume, finish and open on the second line. Preserve original
model routing, native drag behavior, long feedback and the two-point countdown
progress line. Reserve the finish slot while unavailable to avoid action movement.

Records read from yearly activity to the selected day to individual sessions.
Keep the real 53-week calendar and existing date/interval arithmetic. Move export
near the page heading; put date navigation and daily total together, followed by
search/list count and an open log. Duration columns align and expanded details
continue under their parent row without enclosing cards.

Settings remains one page, with the existing 160-point title column and 32-point
gap. Give sections a consistent rule and spacing; align fields, switches and tag
actions. Preserve tag edit protection, transient feedback and essential errors.

## Implementation and acceptance

Keep portable persistence and timer logic unchanged. Separate focus and record
rendering from the shared view helpers to let each surface use the same tokens
without growing the monolithic renderer. Reuse existing controls and model
methods; introduce no dependency, font download, storage migration or native
window-style change.

Create a self-contained interactive HTML design prototype using synthetic data,
rendered only in an isolated headless browser. Save default/minimum-size views,
all three destinations and strip previews. These are design previews, not native
screenshots. Refine the composition after inspecting the rendered images.

Update affected GPUI TestPlatform layout/event regressions against the accepted
new geometry while retaining behavior coverage, text bounds, independent scroll,
draft protection, title/validation edge cases and strip routing. Run canonical
checks, feature-enabled UI lint/tests and signed development packaging with the
existing deployment target and canonical caches. Do not launch or activate a
native window while the user's no-interruption instruction remains in force.
Native glyphs, hover, automatic scroll targeting, IME and OS window behavior
remain a separate acceptance boundary.
