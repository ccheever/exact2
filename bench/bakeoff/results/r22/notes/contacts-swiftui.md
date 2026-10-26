# Contacts build notes

Completed the SwiftUI app, built and installed it as `Contacts` (`dev.bakeoff.contacts.b5a1e29`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The finished app is running at the directory screen. The simulator is back to light appearance and standard text size. No other simulator was used.

The app bundles all 320 starting contacts in `Resources/contacts.json`. Edits, additions, deletions and favorites are saved atomically in the app's Documents directory. A saved empty directory stays empty on relaunch.

The main challenges were keeping the alphabet index accurate during filtering, fitting 31 favorites above a useful alphabetical directory, and handling the largest accessibility text size with the keyboard open. Favorites use a horizontal carousel plus a complete favorites list. Large text uses wider favorite cards, stacked detail actions, smaller decorative icons and a separate search-cancel row. Splitting the large SwiftUI view into smaller pieces also improved compiler performance.

I read the skeleton's README and local `simctl ui` help for appearance and content-size controls. No web research or third-party packages were needed. I added a small standalone UI-test project because the skeleton had no test target.

Decisions where the specification left room: favorites also remain in the alphabetical list; contacts without a last name sort and index by first name; nonletter initials use `#`; whitespace-only names are rejected; phone searches ignore formatting; search is case/diacritic insensitive and supports multiple words. Editing preserves existing birthdays and favorite state. The editor offers the five requested fields.

Validation passed: the five final simulator UI tests cover search, clearing, alphabetical jumps, details, add/edit, favorite toggles, relaunch persistence, both deletion paths, optional contact information, discarded edits, favorites navigation and keyboard behavior. A separate simulator pass exercised dark mode at the largest accessibility text size, including typing and cancellation. Screenshots were visually reviewed. Model checks cover the bundled counts, search normalization, validation, accented names, name-only contacts, sorting, links, birthday formatting, persistence and an empty-directory relaunch. Results and screenshots are under `build/`.

All requested features are complete. Actual phone calls and email delivery cannot be tested end to end on this simulator; the app uses native `tel:` and `mailto:` handoff, with a graceful unavailable-action alert and copy controls.

A ready-made simulator UI-test target or an accessibility tap/type CLI would have made iteration faster. The supplied build script and resource convention otherwise kept the setup small.

Rebuild with:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```
