# Contacts — build notes

Finished and installed as **Contacts**, bundle ID `dev.bakeoff.contacts.938ac7`, on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The final `bun exact.mjs ios --run` completed successfully and launched the installed app.

## Implemented

- All 320 contacts from `data/contacts.json` are embedded in the app as its starting directory.
- Last-name sorting, alphabetical sections, a native ★/A–Z section index, and a Favorites section.
- Live search by name, company, formatted or unformatted phone number, and email.
- Details with optional company and birthday, plus native telephone and email links.
- Add, edit, favorite/unfavorite, swipe/context-menu deletion, and confirmed deletion from details.
- Atomic local persistence, including preserving an empty directory rather than reseeding it.
- Native navigation, sheets, search, keyboard controls, dark mode, Dynamic Type, and an app icon.

## Verification

`bun contacts.mjs verify ios` finished **clean**: seven tests passed; initial screen, seven test ends, and relaunch checked in light and dark; nine screens checked at the largest Dynamic Type size (53 pt body). Add/edit/favorite/delete persistence was exercised across relaunches.

Additional simulator drives checked the add form with the software keyboard visible, a real touch on a contact name opening its details, last-only and first-only names, and the largest-text form. Telephone and email links reached `openURL tel:+442079460958` and `openURL mailto:taylor@example.com`; the agent journals these instead of opening external apps.

The native inspector counts some invisible row portions behind the pinned Favorites header as overlap/covered. `app.json` documents the two narrow `favorite-*` exceptions. The header was made opaque and visually checked while scrolling at the largest text size (`.exact/ios-opaque-header.png`); names do not show through it.

The web host also passed all seven feature tests and normal-size light/dark checks. Its largest-text verification still reports 33 clipping/overflow findings. The requested shipping target is iPhone; its layout was verified separately as described above.

## What was hard / looked up

- Exact's manifest regex rejected the required numeric-leading final bundle-ID component. A local schema overlay accepts the requested ID.
- Exact's native section index only recognized lists whose every header was one character. A local Swift host patch maps `Favorites` to ★ while preserving the alphabetical index.
- Native sticky section headers were transparent at large text sizes. The local host now gives plain-list headers an opaque platform background.
- A navigation-bar Edit button inside a keyed loop failed native activation after adding a contact. A stable header button that resolves the selected contact in its action fixed it.
- A toolkit overlay initially caused Cargo to see symlinked sources as duplicate packages. The build helpers now retain the canonical pinned Cargo workspace while using the local scripts and Swift host.
- Looked up the Exact list-detail pattern, grouped forms, edit sheets, search, swipe deletion, resource/mutation storage, native section-index behavior, and simulator driving/verification.
- The real-touch driver once rejected a row-container tap because its expected child differed from the actual child hit. Tapping the named contact label worked and opened the expected details.

## Decisions

- Display first name then last name; sort by last name, then first name.
- A contact without a last name sorts and groups by first name. Nonletter initials go under `#`.
- Duplicate names are allowed. An all-whitespace name is rejected.
- Birthday is displayed when known and preserved during edits; the specified add/edit fields are first name, last name, company, phone, and email.
- Data is local to this app. No access to the device's system Contacts database is required.

## Rebuilding

The pinned toolkit was left unchanged. `prepare-toolkit.mjs` reproducibly creates the app-local compatibility overlay in `.exact/toolkit`. `contacts.mjs` selects that overlay and always selects the assigned simulator.

```sh
bun contacts.mjs ios --run
bun contacts.mjs verify ios
```

The direct Exact invocation is:

```sh
EXACT2=.exact/toolkit EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios --run
```

## What would have made this faster

A bundle-ID validator matching Apple's allowed identifiers; native section indices that support a Favorites section; opaque sticky-header defaults; stable native toolbar handling for keyed children; and inspector geometry that accounts for pinned headers and native child hit-testing. The tested starter, generated data types, and fast replacement plans were useful.

No required iPhone feature was left unfinished.
