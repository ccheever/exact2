# Contacts — build notes

Finished and installed as **Contacts**, bundle ID `dev.bakeoff.contacts.b29787c`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The app is left open on the directory, with standard text size and light appearance. No other simulator was used.

## Delivered

- All 320 starting contacts are bundled in `Resources/contacts.json`; the bundled copy was compared byte-for-byte with `data/contacts.json`.
- Last-name sorting, alphabetical sections, and a draggable letter index with selection feedback and a VoiceOver adjustable action.
- Live name/company/phone/email search, including case/diacritic matching and telephone searches without formatting punctuation. Native clear/cancel controls and a useful no-results state.
- Contact details, optional company/birthday, phone/email actions, and copy actions for phone/email.
- Favorites shown in a horizontal section at the top and marked in the regular directory.
- Add/edit sheets, appropriate keyboards, keyboard dismissal, whitespace-aware name validation, unsaved-change confirmation, and preservation of birthday/favorite when editing.
- List swipe/context-menu deletion and detail-screen deletion with confirmation.
- Atomic local JSON persistence. Failed writes leave the displayed data unchanged; failed loads preserve the saved file instead of silently replacing it.
- Adaptive light/dark colors, accessible labels, wrapping contact names, and layouts checked at the largest accessibility text size. A custom app icon is included.

## What was hard

SwiftUI does not supply the indexed directory control used here, so it needed a scroll-reader-backed index with drag handling and a separate VoiceOver adjustment action. Large accessibility sizes exposed two layout problems: horizontal Call/Email buttons broke words, and decorative icons crowded the useful text. The final version stacks communication buttons at accessibility sizes, expands favorite cards, and keeps decorative symbols appropriately sized.

Persistence needed to distinguish “no saved file yet” from a deliberately empty saved directory. Tests also cover unreadable/corrupt saved data and disk-write failure so those cases cannot overwrite existing contacts.

## Lookups and decisions

Read the supplied `README.md`, build script, and contact data. Consulted `simctl ui` help for appearance and accessibility text-size controls. No external research or third-party dependency was needed.

Decisions where the specification left room:

- Favorites stay in the full alphabetical directory as well as the favorites section.
- A first-name-only contact is filed under its first initial. Non-letter initials use `#`.
- Equal surnames sort by first name, then ID.
- Birthday is displayed when supplied and preserved during editing; the editor contains the five fields requested by the specification.
- An address-book-style icon and warm neutral/orange visual treatment were created locally.

## Verification

Built and installed using the supplied `build.sh`. The final build completed without compiler warnings.

`QA/model-tests.log` records **37 passing assertions** executed inside the simulator app, covering bundled data, sorting, every search field, clearing search, unknown birthdays, whitespace-only names, first/last-name-only contacts, duplicate names, adding, editing, favorites, deletion, empty directories, corruption protection, and failed writes.

`QA/process-tests.log` records three separate app-process checks: write changes; relaunch and verify additions/edits/favorites/deletions; relaunch again and verify a deliberately empty directory. Test stores were isolated from the real directory and cleaned up.

Screenshots in `QA/` document the directory, offscreen jump to M, phone search, empty search, details, editor, new-contact keyboard, dark mode, and the largest accessibility text size. UI review used launch-driven screen scenarios and screenshots; the provided toolkit had no external tap automation.

## Limitations / unfinished work

No requested app feature remains unimplemented. Real telephone calls and delivery of email cannot be completed on the simulator. The app uses native `tel:`/`mailto:` handlers, shows an explanation if a handler is unavailable, and supports copying the values.

## What would have made this faster

A simulator tap/type/accessibility-inspection tool would make full interaction testing faster and more direct. The supplied build script was useful; preconfigured CoreSimulator/compiler access without the shell sandbox restriction would also have saved an initial failed build attempt.
