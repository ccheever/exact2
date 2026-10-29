# Contacts build notes

Finished and installed as **Contacts** (`dev.bakeoff.contacts.bdfc49e`) on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The final command was `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios --run`.

## Implementation and decisions

- The supplied `data/contacts.json` is imported into the bundled data module and provides all 320 starting contacts. The original data file is unchanged.
- Contacts sort by last name, then first name, with case-insensitive locale comparison. First-name-only contacts sort and index by first name; accented Latin initials normalize to their letter, and other initials use `#`.
- Favorites appear at the top and remain in the complete alphabetical directory as well.
- Search covers names in either order, company, email, formatted phone numbers, and phone digits.
- Add and edit require a trimmed first or last name. Optional fields may be empty. Existing birthdays and favorite status survive edits.
- Phone and email are native `tel:` and `mailto:` links. Their destination apps depend on the device; a simulator cannot place real phone calls.
- Changes use an atomic JSON file in app storage, separate from the bundled seed. All mutation types share a write queue. Empty saved directories remain empty rather than being seeded again. IDs are monotonically allocated.
- Detail deletion confirms; list swipe deletion acts immediately. Create and edit use native modal sheets.

## Validation

- `bun exact.mjs verify`: seven functional tests passed. Normal-size web checks passed. Its automated web accessibility resize pass still reported four coordinate/clipping findings; isolated checks at the same narrow size and large text were clean. The shipped target is iOS.
- `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios`: **clean**, seven tests passed, no light/dark layout findings, no overflow across nine screens at accessibility-extra-extra-extra-large (53 pt body).
- Relaunch tests verified favorite, edit, add, and deletion persistence. iOS correctly launches at the directory, so tests reopen a contact before checking its saved details.
- Native scrolling physics matched UIKit. The toolkit reported zero hitch time and a 16.7 ms longest frame, but declined to judge timing because the machine was busy; its physics check passed.
- A separate real-touch simulator drive opened the form, focused name/phone/email fields, displayed the email keyboard, and saved with the keyboard visible. Light and dark checks passed afterward.
- Verification screenshots and reports are under `.exact/verify/`; the keyboard screenshot is `.exact/keyboard-ios.png`.

## What was hard / looked up

- Read the Exact start guide and started with `list-detail`. Looked up search, section indexing, swipe actions, edit sheets, grouped forms, file grants, symbols, URL links, and iOS driving.
- Exact accepts a defined symbol-role vocabulary rather than arbitrary SF Symbol-style names. The vocabulary command resolved initial compile errors.
- A form component's navigation attributes must sit on its own root for its header to become the native navigation bar. Moving those attributes fixed the sheet chrome.
- Largest-text web checks required flexible header wrapping and a constrained detail-name width; the native controls handled Dynamic Type well.
- Editing while a background native build or verification was running produced stale-plan diagnostics. The final verification ran against stable sources, and a final native install embedded the tested plan for ordinary launches.
- The initial toolkit compiler/cache build needed sandbox escalation. No other simulator was used.

## Unfinished / faster next time

No requested iPhone feature remains unfinished. No server, account, or device Contacts permission is needed.

A concise directory starter combining alphabetical sections, favorites, and an edit sheet would have saved time. A clearer early distinction between symbol roles and SF Symbol names, and between a component wrapper and a navigation root, would also help. Keeping sources stable during final native verification avoids unnecessary stale-build retries.
