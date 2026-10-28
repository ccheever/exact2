# Expenses — build notes

Completed the specification with a programmatic UIKit app, app and scene delegates, and the requested name and bundle identifier. The production build is installed and launched on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was used.

## Features

- All 240 supplied transactions ship in the bundle as the starting history.
- Activity is grouped by date, newest first, with day totals and a current-month summary.
- Add and edit amount, merchant, one of the seven categories, date, and optional note. Swipe to delete; editing also offers a confirmed delete action.
- Categories show monthly totals, descending spending order, and percentage shares. Drilldowns default to this month and also offer all-time history.
- Changes save atomically to local storage; deleting the entire history does not reimport the seed. A failed load is reported and protected from overwrite.
- Dynamic Type, dark mode, VoiceOver labels, native navigation, date selection, decimal keyboard, keyboard dismissal, and unsaved-change protection are included.

## Verification

The simulator-based, XCTest-free harness passed **47 checks, 0 failures**. Checks cover exact seed and day totals, positive amount validation, USD formatting, leap/year-boundary dates, add/edit/delete persistence, empty-history persistence, corrupt-data protection, grouping, category totals and filters, and the real editor save path. Results are in `build/verification.txt`.

Inspected simulator screenshots of the activity list, categories, category drilldown, editor, decimal keyboard, and the largest accessibility text size, including the note field above the keyboard. Captured runtime logs for the tested screens contained no layout warnings. Screenshots are in `build/`.

To run the test-enabled build again:

```sh
BUILD_TESTING=1 SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run
xcrun simctl launch --terminate-running-process CEDFB9C9-FF04-4617-B156-94C56B83CA86 dev.bakeoff.expenses.bbdd852 --verify
```

The harness writes `Documents/verification.txt` in the app's simulator container. A normal build excludes the harness:

```sh
SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run
```

## Hard parts and decisions

- iOS 26 moves large navigation titles into the scrolling content. An opaque scroll-edge navigation background covered the title; native transparent scroll-edge appearance fixed it.
- Large accessibility text required vertical cell layouts, wrapping headings, and fitted currency labels. Explicit active-input scrolling ensures the optional note remains visible above the keyboard.
- Monetary values are stored as integer cents. Input accepts a positive USD amount with up to two decimal places, up to $999,999,999.99, and requires a nonblank merchant. Category choices come directly from the supplied seven-category vocabulary.
- “This month” uses the device's current local month. Category drilldowns offer both current-month and complete category history to resolve the specification's unspecified drilldown period.
- I used the skeleton README, compiler diagnostics, and `simctl` help; no external packages or web lookups were needed. CoreSimulator required sandbox escalation for build/install and screenshots.

Nothing in the specification remains unfinished. A supplied simulator touch/accessibility automation driver and an initial iOS 26 navigation-appearance example would have made the UI verification faster.
