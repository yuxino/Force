# Daily record accounting

Daily statistics and the activity calendar continue to merge focus intervals at
local midnight, exclude pauses/rest, and include ongoing focus. Individual task
investment remains separate, so parallel records can sum above merged focus.

The records list now queries actual interval overlap for the selected date,
including running/paused rounds. Preserve zero-time start/completion events on
their event date, exclude paused-only gaps, and retain the uncapped date query.
A cross-day round appears on each date with its clipped daily investment, plus
its full accumulated time. Keep full dated boundaries, pause duration and timing
mode in expanded details; dates appear in cross-day row time ranges too.
Deletion still deletes the entire completed round across all dates, with an
explicit confirmation. Unfinished rounds cannot be deleted from records.

Follow the date at midnight only when the user was viewing today. Refresh from
the same persisted intervals; no schema migration or rewriting historical data.

Verify a synthetic 20:52:54–01:19 round with 3:52:25 investment, 33:41 paused and
54:00 after midnight. Compare both day rows with statistics/calendar, and test
ongoing rounds, paused-only dates, zero-time boundaries, parallel focus/rest,
uncapped history, date rollover and minimum-width layout.
