# Contacts build notes

Completed and installed `Contacts` (`dev.bakeoff.contacts.b1ea203`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The app is left running with the original 320 contacts, including 31 favorites. The supplied JSON is bundled unchanged in `Resources/contacts.json`.

## Implementation decisions

- Favorites have a horizontally scrolling section above the complete alphabetical directory. Favorite contacts also remain in their alphabetical sections.
- Contacts without a last name sort under their first name. Accented Latin initials fold into their corresponding section; other initials use `#`.
- Search matches names, company, phone, and email, ignoring case and accents. Phone searches also work without formatting punctuation.
- Names and other fields are trimmed on save. A first or last name is required; existing birthdays and favorite status survive edits.
- Changes are written atomically to the app's Application Support directory before updating the UI. Empty saved directories stay empty after relaunch. Unreadable saved data is preserved rather than replaced with the seed.
- Phone and email use system `tel:` and `mailto:` handlers. Details can be copied with a long press.

## What was hard

The largest accessibility text size exposed overlapping decorative icons and a focused field partly hidden by the keyboard. Icons now retain a suitable decorative size, text wraps, and the editor centers focused fields at accessibility sizes. Normal sizes retain native form scrolling; an explicit regression check caught and fixed the interaction between the two behaviors.

The skeleton had no Xcode project or UI automation harness. I added a small standalone XCTest UI runner that targets the installed app. The simulator's current search dismissal button is labeled “Close”; confirmation and discard flows use explicit native alerts.

## Verification

- Built, installed, and launched with the supplied `build.sh` and the assigned simulator only.
- Four functional UI tests passed: alphabetical section jumps; search across all four fields and clearing; details and birthday display; blank and whitespace name validation; first-name-only and last-name-only creation; editing; favorite toggling; relaunch persistence; deletion from details and the list; canceling deletion; discarding and retaining edits.
- Rechecked search after reducing repeated sorting, and creation/editing/cancellation after the keyboard fix.
- Passed dark-mode UI navigation and editing at `accessibility-extra-extra-extra-large`, including an assertion that the focused input remains above the keyboard. Reviewed screenshots for layout.
- Model checks passed for all 320 seed contacts, search normalization, sorting, optional fields, validation, favorites, persistence, deleting every contact, empty-store relaunch, corrupt-file protection, and failed-write handling.
- Final results and screenshots are in `Artifacts/`; repeatable commands are in `Tests/README.md`. The temporary UI test runner was removed from the simulator, and light appearance and standard large text were restored.

## Lookups, limits, and improvements to the toolkit

I used local compiler feedback and `simctl help ui`; no web research or external dependencies were needed. The favorite layout and first-name-only sorting behavior were design choices where the spec allowed judgment.

No specified feature remains unfinished. The simulator cannot complete a real telephone call; email and telephone actions depend on the system having an appropriate handler. The app explains unavailable handlers and offers copying.

A supplied UI test runner and screenshot-export command would have made iteration faster, particularly for keyboard visibility and large-text checks. A starter app-icon asset pipeline would also help.
