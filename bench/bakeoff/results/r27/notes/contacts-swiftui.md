# Contacts build notes

Finished and installed on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`.

- App name: Contacts
- Bundle ID: `dev.bakeoff.contacts.b1b795b`
- Final build: `SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run`
- All 320 original contacts ship in `Resources/contacts.json`, byte-for-byte identical to `data/contacts.json`.
- Changes are stored as atomic JSON writes in the app's Application Support directory. Relaunching or installing an updated build preserves them.

## Implementation choices

Favorites appear at the top and also remain in the complete alphabetical directory. Contacts without a last name sort and index by first name; names beginning with a nonletter use `#`. Search ignores case and diacritics, supports multiple terms across the searchable fields, and matches phone digits independently of formatting. Names and optional fields are trimmed when saved. Editing retains birthdays and favorite state.

The UI uses SwiftUI navigation, lists, forms, search, swipe actions, sheets, confirmation dialogs, and alerts. Phone and email buttons use standard `tel:` and `mailto:` URLs, with a fallback explanation if no compatible app is available. Values can be copied through their context menus. Dark colors adapt automatically. Accessibility text sizes use wider contact rows, while decorative initials are hidden to give names room.

## Verification

Real UI interactions ran on the assigned simulator with simulator cloning disabled. The test harness is in `Tests/`; it is not part of the shipped app.

Passed checks included:

- Initial count of 320 contacts and 31 favorites; alphabet index navigation.
- Phone-digit, email, company, and name search; clearing search restores the directory.
- Details and known birthday display.
- Favorite marking, unmarking, and persistence across app termination/relaunch.
- Whitespace-only name rejection; valid first-only and last-only contacts.
- Adding, editing, trimming, and persistence of the edited contact across relaunch.
- Keeping or discarding unsaved edits.
- List swipe deletion and detail deletion, including confirmation cancellation.
- Deletion surviving relaunch.
- Dark mode at the largest accessibility text size, scrolling, editor controls, and keyboard dismissal.

The final lifecycle pass is recorded in `build/uitests5.log`, list deletion/discard passed in `build/uitests4.log`, and final accessibility checks passed in `build/accessibility-final.log`. Screenshots in `build/` were visually reviewed. Temporary test contacts were removed, the test runner was uninstalled, and the simulator was restored to light appearance and the standard Large text size. The final installation was produced by the supplied build script.

## What was hard / looked up

The bare Swift compiler skeleton had no UI test target, so I generated an isolated Xcode test project. Compiler plugins and CoreSimulator required tool access outside the execution sandbox. Native search dismisses with a control labeled Close on this runtime, and the keyboard and toolbar can both expose Done; reliable test selectors needed to account for that. A broad test search also found legitimate contacts outside the visible list, so the fixture query was made unique. Xcode sometimes stalled while collecting result bundles after reporting passing assertions; those cleanup processes were stopped after test execution ended.

The largest accessibility size needed a separate visual pass: fixed avatars and growing decorative stars initially narrowed names too much. The final rows reserve more width for text at accessibility sizes.

I consulted the supplied README and local `simctl ui` / `xcodebuild` command help. No web research was needed.

## Limitations and improvements to the toolkit

No required feature remains unfinished. Actual phone calls and email delivery cannot be exercised on this simulator; the app delegates those actions to installed compatible apps.

A ready-made UI test target, stable result collection, and preauthorized Swift compiler/simulator access would have made this faster. Clearer guidance about whether favorites should repeat in the alphabetical directory would also remove one product assumption.
