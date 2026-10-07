---
name: 20261005-x36-data-runtime-intl-locale
plan: 20261005-t3code-macos-parity
status: adopted
kind: framework-gap (unconfirmed)
blocks: [20261005-desktop-shell-details, 20261005-reference-logic-tests-done-areas]
upstream_url: https://github.com/ccheever/exact2/issues/118
reproduced_on: null
---

# X36: Locale-aware `Intl` and the system locale in the data runtime

## Summary

T3 Code shows times and the first day of the week in the Mac's own locale: the desktop app reads the system locale and gives it to `Intl.DateTimeFormat` and `Intl.Locale`. The clone's data runtime is Hermes, and nothing in this plan's sources says whether Hermes in exact2 supports `Intl.DateTimeFormat` with an explicit locale and `hour12`, `Intl.Locale` week info, and `supportedLocalesOf`, or how a page reads the system locale. The clone formats with the runtime default today. What is needed is ECMA-402 `Intl` in the data runtime with locale data, and a way to read the system locale tag.

## Why this issue arose

### The T3 Code behavior

- **Why the reference reads the locale itself.** The packaged Electron app ships only the `en-US` Chromium locale pak, so `navigator.language` and the default `Intl` locale are pinned to `en-US` whatever the OS says. The desktop therefore exposes the OS locale as a BCP-47 tag: `getSystemLocale` on the bridge (`packages/contracts/src/ipc.ts:1133–1139`, comment), implemented as `Electron.app.getSystemLocale().replace(/_/g, "-")` (`apps/desktop/src/electron/ElectronApp.ts:49–52,132`; IPC `apps/desktop/src/ipc/methods/window.ts:74–79`).
- **What uses it.** `apps/web/src/timestampFormat.ts`: `resolveTimestampLocale(systemLocale)` (lines 31–47) returns the tag if `Intl.DateTimeFormat.supportedLocalesOf([tag])` accepts it, otherwise `undefined` (the runtime default); `readHostSystemLocale` (:49–54); every timestamp formatter is a cached `Intl.DateTimeFormat(timestampLocale, {hour: "numeric", minute: "2-digit", [second], hour12 for "12-hour"/"24-hour"})` (lines 1–20, 86–125); `resolveWeekStartsOn(locale)` (:69–83) reads `new Intl.Locale(...).getWeekInfo().firstDay` or `.weekInfo.firstDay` and maps Monday=1…Sunday=7 to a `Date#getDay` index; `weekStartsOn` is used by the custom snooze calendar (`components/CustomSnoozeDialog.tsx`). About 15 call sites format timestamps (sidebar times, message times, tooltips, usage and connection rows). Long tooltips deliberately stay English with an ordinal day ("12:04, 4th June", lines 121–150).
- **Numbers.** The reference formats numbers with the default locale, which is `en-US` in that app; the clone's explicit `toLocaleString('en-US')` calls (for example `diagnostics-view.ts:17`, `r6-pr-logic.ts:222`) therefore match the reference's effective output.
- **Tests** (`apps/web/src/timestampFormat.test.ts`): "defers to the runtime default when the host reports no locale", "uses a BCP-47 tag reported by the host", "defers to the runtime default rather than throwing on an unusable tag", "leaves the default to the caller for a malformed locale", "follows the locale the desktop host reports", plus "uses the host locale for both the numeric date and wall-clock time" (line 204) and the `formatShortTimestamp` cases.
- **User-visible states.** A Mac set to Korea shows Korean-formatted times ("오후 2:20"); to Germany, 24-hour times; the week starts on Monday in the snooze calendar where the locale says so. A bad tag falls back to the runtime default without an error.

### What exact2 does today

Bundled library (`20261005-platforms-v3`): the data runtime's `Intl`, locale data and system-locale access are **not covered: unknown**. `EXACT2-GAPS.md` has no entry (found while writing the plan's tickets, 2026-10-05). The framework facts available to the page that this plan has seen are `online`, `root-font-size`, `visibility-state` and `can-share` plus the viewport `media` list (drive receipts in `target/t3-ui-parity/lanes/r12-threads/drive-receipts.ndjson`); no locale appears there. The data runtime is Hermes at the pinned revision `6badada` (`AGENT-HANDOFF.md`, "Pinned tools"). Whether this Hermes build enables `Intl`, which locales it knows, and what its default locale is: all to confirm on the pinned `main` at `issue-open`.

### Where the clone hits it

- Time formatting uses the runtime default: `sidebar-presentation.ts` (`clockLabel`, lines 82–84; the "next week" preset label, line 111), `timeline-presentation.ts` (`shortTime`, lines 26–30), `composer-controls-usage.ts:105` (`toLocaleString()` for the usage-limit reset time). Tests compare against `Intl.DateTimeFormat(undefined, …)` run by Bun, not by Hermes (`chat.test.ts:46`).
- `shell-slow.ts:66` (`startedLabel`) writes the 12-hour "Started h:mm:ss AM/PM" text by hand "as en-US prints it"; the file does not say why `toLocaleTimeString()` was not used.
- `pages-usage.ts:78` reads the time zone with `Intl.DateTimeFormat().resolvedOptions().timeZone`, with a `'UTC'` fallback in a `catch`, which shows that an earlier session expected `Intl` to be missing or to throw in some runs.
- `settings-b-icons.ts:50–56` has a fallback "without `Intl.Segmenter`": again an expectation that `Intl` may be incomplete.
- The plan's workaround (`20261005-desktop-shell-details`, scope item 3): a native op returns `Locale.current.identifier` with `_` replaced by `-`; TypeScript ports `resolveTimestampLocale` and `resolveWeekStartsOn`; if the runtime ignores an explicit tag, the times are formatted in Swift with `DateFormatter` for that locale, accepted only if its output equals the reference's for a table of locales (en-US, ko-KR, de-DE, ja-JP, en-GB, ar-SA) and instants.

What a person would see differently if the runtime ignores the tag and the Swift fallback is not exact: times and week start follow the runtime default instead of the Mac's region, or differ in small ways (separators, AM/PM text, digits) between ICU and Foundation. Not measured.

## Why it must be resolved

Parity goal: the reference follows the Mac's region for times and the week start; the clone should too. The ticket `20261005-desktop-shell-details` carries this and cannot finish its locale rows without knowing whether the runtime honors a tag. Cost of the Swift fallback: a second formatting implementation, an equality table that must be kept current for each locale ICU and Foundation disagree on, and a TypeScript/Swift split for what is one function in the reference. A missing `Intl.Locale` week info would also make the snooze calendar's first day wrong for Monday-first regions.

## Requested support

The web standard is ECMA-402 (`Intl`). On the macOS host first:

- **A. `Intl` in the data runtime (preferred).** `Intl.DateTimeFormat(locale, options)` honoring `hour12`/`hourCycle`, `second`, `weekday`, `month`; `Intl.DateTimeFormat.supportedLocalesOf`; `Intl.Locale` with `getWeekInfo()`/`weekInfo`; `Intl.NumberFormat`/`toLocaleString(locale)`; locale data for the system's languages (on Apple platforms, backed by the OS's ICU).
- **B. A page fact for the system locale** (the web equivalent is `navigator.language`/`navigator.languages`), so apps do not need a module op. Needed even with A, because the default `Intl` locale may not follow the OS (as in Electron).
- **C. Keep the Swift formatting** and close by decision. Not preferred: it duplicates ICU behavior.

## How to reproduce

To confirm on the pinned `main` at `issue-open`:

1. Minimal app whose data source returns, for the table of tags (en-US, ko-KR, de-DE, ja-JP, en-GB, ar-SA, and an invalid tag `not_a_tag`) and instants (now, yesterday 23:59, one year ago): `new Intl.DateTimeFormat(tag, {hour: "numeric", minute: "2-digit"}).format(date)` with and without `hour12: true`; `Intl.DateTimeFormat.supportedLocalesOf([tag])`; `new Intl.Locale(tag).getWeekInfo?.().firstDay ?? new Intl.Locale(tag).weekInfo?.firstDay`; `Intl.DateTimeFormat().resolvedOptions().locale`.
2. Run it under Chrome (the conformance oracle) and under the agent on macOS. Expected: equal strings. Actual: unknown (an exception, a pinned `en-US`, or equal).
3. Clone: with the Mac's region set to Korea, open a thread list and a timeline. Expected (reference): Korean-formatted times. Actual: the runtime default (to confirm).

## Acceptance for the fix

- The table in step 1 gives equal output in Chrome and in the data runtime for every tag and instant, including `hour12` true and false; an invalid tag throws `RangeError` the same way, or `supportedLocalesOf` returns an empty list.
- `Intl.Locale(tag).getWeekInfo()` (or `weekInfo`) returns the same `firstDay` as Chrome.
- A page fact or an `Intl` default reports the Mac's locale tag; changing the region in System Settings and relaunching changes it (attended session).
- The ported `timestampFormat.test.ts` cases pass under the data runtime (a Hermes-run test, to be defined with the maintainers) and under Bun.

## App adoption after resolution

`20261005-desktop-shell-details`: drop the Swift `DateFormatter` fallback and the equality table; keep the TypeScript port of `resolveTimestampLocale` and `resolveWeekStartsOn`; read the locale from the fact or the native tag op. Replace the default-locale calls in `sidebar-presentation.ts`, `timeline-presentation.ts` and `composer-controls-usage.ts` with the ported formatter; reconsider `shell-slow.ts` `startedLabel` and the `Intl` fallbacks in `pages-usage.ts` and `settings-b-icons.ts` (remove only if the runtime test shows they are not needed). Rows that must pass: the locale table equality row and the attended region row. `issue-close` verifies them.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (check the capability on the pin first: run the step-1 table; if it already matches Chrome, record the result and close this issue; otherwise reproduce, search for duplicates and prepare the report for the user's approval; publication only after approval).

## Resolved upstream and adopted (2026-10-07, adopt-main-fixes-r4)

[#118](https://github.com/ccheever/exact2/issues/118) was closed by main #204 (`4132f02c5`), in the feature
branch since main `463acda68` ([20261007-adopt-main-fixes-r4](../../tasks/closed/20261007-adopt-main-fixes-r4.md)):
the Hermes prelude (`js/src/standard.js`) installs `Intl.Locale` on every Hermes host, with Chrome's
canonicalization, getters and `getWeekInfo()` from CLDR week data. Main's Hermes-against-Chrome test
(`cargo test -p exact-js --test it pure_utilities_match`, 75 `Intl.Locale` rows) passes in this branch.

Adopted: `timestamp-format.ts`'s `resolveWeekStartsOn` is the reference's and needed no change; on macOS it
now answers the week start instead of `undefined`. Its comment no longer says the runtime lacks
`Intl.Locale`, and `desktop-shell-details.test.ts` expects the weekday for every tag (en-US 0, en-GB 1,
pl-PL 1, ar-EG 6, de-DE 1, ko-KR 0, fa-IR 6) instead of accepting `undefined`.

Kept, with reasons: `RUNTIME_LOCALE` (en-US) for the calls the reference leaves at the runtime default,
because Hermes's default locale follows the Mac's region (en-KR) where the packaged Electron app's is en-US
(main's docs: pass the locale explicitly; not part of #118). `T3Locale.swift`, because it is Electron's
`app.getSystemLocale()` (`[NSLocale currentLocale]`), which `exactTime().locale` (the preferred language)
is not. Known upstream differences that the clone does not hit: a formatter given a `Locale` object rather
than its string (the clone passes strings) and the `ja-JP` long-date space. No clone view shows a week
calendar (the snooze picker is a date input), so nothing visible changes.
