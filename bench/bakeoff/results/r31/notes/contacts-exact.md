# Build notes

Contacts is implemented in Exact and ships all 320 entries from `data/contacts.json`. The JSON is imported into the bundled data module; changes go to a separate, atomically written app-private JSON file. No network is required.

## Decisions

- Favorites appear at the top and also retain their position in the alphabetical directory.
- Sorting uses last name, then first name. A person with only a first name is filed under that name; names outside A–Z use `#`.
- Either a first or last name is required, after trimming whitespace. Duplicate names are allowed.
- Search matches names in either order, companies, email addresses, and formatted or unformatted phone numbers.
- Editing changes the five requested fields and preserves the existing birthday and favorite status.
- Both list swipe deletion and detail deletion ask for confirmation because deleting a contact is permanent.
- Phone and email use native `tel:` and `mailto:` links. Completing a phone call or sending mail depends on system handlers; no external calls or messages were made.

## What was hard / looked up

- Exact's section index requires every section after the first letter to be a lettered, nonempty section. An extra footer-only count section suppressed the A–Z index; removing it fixed the native list.
- A column of text is projected into a native subtitle cell. The detail avatar needed its own box to preserve a centered custom header.
- Native search stays presented after its keyboard resigns. Opening a contact now blurs the keyboard and removes the off-screen search controller until returning to the directory, so Back and Edit display correctly.
- Native relaunch begins on the root screen, whereas web reload can retain the URL. Persistence tests reopen the contact after relaunch rather than assuming restoration of the current detail route.
- Semantic colors have Exact-specific names; `contract vocab color` was needed to resolve them.

## What would make this faster

A complete contacts starter, warnings for sections that suppress indexing, a clear custom-header example, and automatic search-controller dismissal on route pushes would reduce iteration. Native screenshots were essential: model-level assertions alone did not expose the search presentation or subtitle-cell problems.

Web behavior tests pass. The web layout probe reports shifted swipe containers at the largest text size; an isolated drive at the same size has clean bounds. The deliverable is the native iPhone app, whose own layout and accessibility verification is recorded separately below.

The simulator was heavily loaded during verification. Search-result interaction tests explicitly allow UIKit's search dismissal to finish before tapping the next screen. A separate real-touch drive verified search → details → favorite, and native navigation was visually checked for Back and Edit.

## Final result

- Finished, built, installed, and launched as **Contacts**, bundle ID `dev.bakeoff.contacts.b485324`.
- Used only iPhone simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`.
- `bun exact.mjs verify ios`: **clean**, 10 tests passed, 0 failed; light/dark checks have no findings; largest accessibility text size spills nowhere across 12 screens; scrolling physics reported “like UIKit.” Timing judgment was limited by machine load.
- `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios --run`: completed successfully and left the finished app installed and launched.
- No requested iPhone features remain unfinished.

Verification logs and screenshots are kept under `.exact/`; final native report: `.exact/verify-ios.log`, installation: `.exact/final-install.log`.
