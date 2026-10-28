# Contacts build notes

- Built with Exact for bundle ID `dev.bakeoff.contacts.bfbf5af` and simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7` only.
- All 320 supplied contacts are bundled through the data module as the offline starting directory. Edits, favorites, additions, and deletions are written atomically to app storage. Saving an existing contact preserves its birthday and favorite flag.
- Native controls provide navigation, search, alphabetical section index, sheets, keyboard types, swipe deletion, context menus, and confirmation alerts. Favorites also remain available in the alphabetical directory.

## What was hard

- Exact’s Contract language accepts fewer JavaScript constructs than TypeScript; regular-expression phone normalization had to use supported string/array operations in the view.
- Native list headers and footers have strict structural rules. The compiler messages helped correct these quickly.
- The largest text size exposed long-word overflow in the prominent name heading and wrapping requirements in form toolbars. Names are presented as individually wrapping words with automatic hyphenation; toolbars wrap without reducing text size.
- The web verifier revealed hidden swipe actions while touring test targets, leaving the corresponding content swiped away. Targeting each contact’s visible name instead of the entire swipe container avoids treating intentionally revealed swipe actions as the resting layout.

## Lookups and decisions

- Read Exact’s start guide and looked up list/detail, search, swipe actions, grouped forms, persistence, assets, navigation, iOS driving, and text layout.
- Null company and birthday values are treated as absent. Unknown birthdays are omitted. Dates are formatted in UTC to prevent a birthday shifting by a day.
- Contacts without a last name sort and section by first name; nonalphabetic initials go under `#`. Duplicate names are allowed and retain separate identities.
- Phone searches accept both formatted numbers and digits alone. Search also matches full name, reverse name order, company, and email without case sensitivity.
- Kept the optional authoring diary locally; it was not sent anywhere.

## Validation

- Seven functional tests cover the seed count/order, search fields, favorites, add/edit/cancel, name validation, birthday preservation, deletion, missing contacts, and persistence.
- All seven passed on web and iOS.
- `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios`: **clean**. Light/dark checks found no issues; all nine screens checked at accessibility-extra-extra-extra-large (53-point body) fit without spilling. Relaunch checks passed.
- The native scrolling check found nothing for the app to fix. The host machine was heavily loaded, so the toolkit judged scrolling physics rather than overall timing; it also reported a toolkit-owned status-bar scroll-to-top delay.
- Web functional tests and standard-size layout checks pass. Chrome’s largest-text pass still reports three long-word name overflows despite automatic hyphenation. The corresponding native iPhone checks are clean.
- Ran `bun exact.mjs ios --run` with the assigned simulator after verification. The finished app is installed and running.

## Unfinished work

- No requested iPhone features were left unfinished. The browser-only largest-text differences above remain outside the requested iPhone deliverable.

## What would have made this faster

- A directory starter with typed bundled seed data, an edit sheet, and name/phone/email fields.
- A clearer list of supported expression methods and canonical symbol names.
- Layout checks that distinguish deliberately revealed swipe actions from resting content.
