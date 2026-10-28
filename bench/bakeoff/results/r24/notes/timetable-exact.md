# Timetable build notes

Finished, built, installed, and launched on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`.

- App name: Timetable
- Bundle ID: `dev.bakeoff.timetable.b4e680d`
- Final install: `EXACT_SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 bun exact.mjs ios --run`

## Implemented

The complete daily departure board, opaque pinned hour headings, searchable station picker, Now scrolling, single-line/all-lines filtering, past-departure dimming, delay/cancellation labels, and persistent starred departures and station selection are working. Saved shows departures across stations with their station and platform. The app includes a custom icon.

The supplied JSON contains 2,698 departures, twenty stations, and five lines. Every record is included. The original remains in `data/departures.json`; an identical copy ships in `assets/departures.json`, and the data module also embeds the data for offline use.

## Verification

- `bun exact.mjs verify`: clean; nine tests passed.
- `EXACT_SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 bun exact.mjs verify ios`: clean; nine tests passed.
- Light/dark mode and maximum Dynamic Type: no remaining layout findings.
- Tested real simulator touches for starring, tabs, station selection, Now, and scrolling, plus native search/keyboard and line selection.
- Verified station and Saved persistence across relaunches, sorting of departures around midnight, filtering, status labels, empty search, cancellation, end of service, and the transition to a new day.
- The iOS scroll probe found nothing for the app to fix. Its physics checks passed; timing judgment was limited by a busy host. It separately reported Exact's own status-bar scroll-to-top timing.
- Confirmed the raw bundled data matches the source byte for byte. Saving all 2,698 departure IDs uses 12,381 bytes, within Exact's 16 KB persistent-state limit.

Screenshots and detailed reports are in `.exact/verify/`; additional simulator screenshots are in `.exact/`.

## What was hard

Exact's native grouped plain-list headings pinned with transparent backgrounds over custom rows. The main board uses Exact's styled scrolling list with opaque sticky headings. A redundant Saved section heading was removed after the maximum-text iOS check exposed the same issue there. Scrolling remains backed by a native UIScrollView.

Native iOS search hides the navigation bar while active. The cancellation test checks the sheet's Cancel before search and native dismissal after search. Keyboard and transition settling must be explicit in the test driver.

The toolkit initially needed sandbox escalation to acquire its Cargo lock in the pinned SDK checkout. The first iOS runtime build took about three minutes; subsequent view-plan updates were quick.

## What I looked up or decided

I used Exact's start-here guide and vocabulary for tabs, native search, sections, persistence, timers, local time, assets, scroll alignment, and iOS driving. Combined task modifiers require `when` before `key`. Scalable `rem` lengths work individually, but Exact does not support them inside `calc()`.

The source lists trips after midnight at the end of each station block. I sort by the actual minute after midnight. Now reads the live clock when pressed and chooses the first strictly later scheduled departure in the active filter. After the final departure, it shows the end-of-service footer; after midnight it returns to the new day's earliest upcoming trip. Delay and cancellation labels retain the supplied scheduled times and wording.

Colors use the supplied line palette with contrasting badge text. Past time/destination text uses the system secondary label color, preserving readability in both schemes.

`app.json` documents narrow checker allowances for intentional pinned-heading overlap and partially visible station rows at scrolling viewport edges. The final normal-size iOS and web checks reported zero accepted findings; maximum-text checks reported nothing spilling.

## Unfinished work

No requested features remain unfinished.

## What would make this faster

An opaque native section-header background option, an example showing combined task modifiers, and clearer maximum-text diagnostics distinguishing scroll-viewport edges from actual row clipping would reduce iteration. The early iOS prebuild and rapid view-plan updates were useful.
