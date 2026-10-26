# Health by a person (LLP 1133.026 §15, stages 2 and 3)

What no tool here can do: answer HealthKit's sheet or Health Connect's
permission screen, and read a store that holds a real week. The suites
drive the rest under the agent, over `packages/health/fixture`:
`examples/calendar/tests/activity.mjs` on an iOS simulator, macOS and the
web, and `contract/cli/tests/it/health.rs` for the permission cases.

Use LLP 1133.020's test phones and a simulator or emulator you made,
never a personal phone and never a store that holds a person's data.
Write sample data only into the test phone's store, and write down the
commit, the device and its OS version beside each result.

The Calendar example carries the module on iOS:
`EXACT_APP_DIR=examples/calendar bun host/apple/build.mjs --device calendar-ios --run`
on the test iPhone (`--ios --sim <udid>` on a simulator you made). Its
inspector's Show my activity switch asks; the month grid shows steps per
day; the selected day's agenda shows its workouts.

## Before: sample data

On the test iPhone, open Health › Browse › Activity › Steps › Add Data and
add steps on three days of the current month, one of them 0; add one
workout (Fitness or Health › Browse › Activity › Workouts › Add Data).
Note each day's total as Health shows it.

## iPhone or iPad, iOS 17 or later

1. **The switch, and nothing asked.** Install Calendar fresh. The
   inspector shows Show my activity, off. No sheet shows at launch.
2. **The sheet.** Turn it on: HealthKit's sheet shows, naming Steps and
   Workouts, with Calendar's purpose ("Shows your steps and workouts
   beside your plans …"). Turn both on and tap Allow. Each noted day
   shows its total under its date in tabular digits; the 0 day shows 0;
   a day with no samples shows nothing. The noted workout shows in its
   day's agenda.
3. **Part of it.** Health › Sharing › Apps › Calendar: turn Workouts off.
   Return: the steps stay, the agenda's workouts are gone, and the
   section shows "Showing the activity Health shares with Calendar." It
   never says access is off (D5).
4. **None of it.** Turn Steps off too and return: the month shows "No
   activity to show. If you turned off Calendar's access, you can allow
   it in Settings › Health › Data Access & Devices."
5. **A change in the store.** With both on again, add a workout in Health
   and return to Calendar: it shows without a relaunch.
6. **Nothing kept.** Close Calendar and open it again: the first frame
   shows no steps, then the steps come (D9).
7. **A limited window (iOS 27).** If the sheet offers a start date, allow
   from the 1st of the month: earlier days show nothing, and the line
   reads "Calendar can see your activity from 1 <month>." Record whether
   iOS offered the choice.
8. **A workout's statistics.** With Workouts on and Steps off, record
   whether a workout's distance still shows (§15 stage 2: whether a
   workout's statistics need their type's own read).

## Android phone, API 31 or later

The conformance fixture (`test/fixture`) carries the module on Android.
`bun packages/health/test/android.mjs` builds it and leaves its APK at
`host/android/app/build/healthfixture/outputs/apk/release/app-release.apk`;
install that with `adb install`. On an Android 13 phone, install Health Connect
from Google Play first. Add steps and a workout in a fitness app that
writes to Health Connect, or in Health Connect's own data screens, and
note them.

1. **The permission screen.** Install the fixture fresh and tap Ask:
   Health Connect's screen shows Steps, Distance and Exercise. Its
   privacy policy link opens the fixture's `privacy_policy`
   (`https://example.com/privacy`) in the browser.
2. **All of it.** Allow all: `health-status` reads `true granted`, `days`
   the noted steps, `workouts` the noted workout.
3. **Part of it.** In Health Connect › App permissions › the fixture, turn
   Exercise off and return: `health-types` reads `workouts prompt`, and
   `workouts-error` `NotAllowedError`. Tap Ask and decline Exercise twice:
   it reads `workouts denied`, and a third Ask shows nothing.
4. **The window.** Steps older than 30 days before today do not show in
   `days`; record what Health Connect returns for them.
5. **Android 14 and later:** Settings › Security & privacy › Privacy ›
   Health Connect › App permissions › the fixture: its privacy policy
   entry opens the same page (the permission-usage alias).
