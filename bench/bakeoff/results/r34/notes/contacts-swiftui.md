# Contacts — build notes

Finished and installed as **Contacts**, bundle ID `dev.bakeoff.contacts.b6910e0`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final production app was relaunched successfully. No other simulator was used.

## Implementation

- Bundles an unchanged copy of all 320 people from `data/contacts.json` in `Resources/contacts.json`.
- Alphabetical directory with letter sections, a draggable section index with haptics, and an adjustable VoiceOver index.
- Favorites appear in a horizontal section above the complete directory. They also retain their alphabetical entries.
- Live name, company, phone, and email search, including case/diacritic folding and phone-number matching without punctuation.
- Contact details, favorite toggle, phone/email actions, and copy menus. Birthdays use a fixed calendar and time zone to avoid date shifts.
- Add/edit forms with first-or-last-name validation, trimmed input, suitable keyboards, focus navigation, keyboard dismissal, and discard confirmation. Saving a new contact opens its details.
- Swipe deletion from the directory and confirmed deletion from details.
- Atomic JSON persistence in Application Support. Empty saved directories stay empty; failed writes report an error and retain the previous visible state.
- Native navigation, dark mode, Dynamic Type, accessible labels, and a locally drawn address-book app icon.

## Validation

Built with the supplied `build.sh`. Ran **37 passing checks inside the assigned iOS simulator**, covering bundled data, all search fields, formatted/unformatted phone queries, name validation, first-only/last-only contacts, accented names, favorites, add/edit/delete persistence, birthday preservation, sorting after edits, deleting every contact without reseeding, failed-write behavior, and action URLs.

The test harness uses an isolated temporary store for mutations. The original 320 contacts and 31 favorites remain in the installed app. The test runner restores the production executable after completion.

Also inspected simulator screenshots of the directory, contact details, editor, new-contact keyboard, matching search, and empty search results. Reviewed dark mode and the largest accessibility text size, then restored light appearance and the normal Large text setting. Final screenshot: `build/final-directory.png`. Data-check output: `build/checks.log`.

To rebuild:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```

To repeat the simulator data checks after building:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A Tests/check.sh
```

## Hard parts and decisions

The main layout challenge was fitting the section index beside horizontally scrolling favorites without overlapping text, while letting contact rows and cards expand at the largest text sizes. Decorative detail icons needed fixed sizes so they would not crowd the scaled text. Persistence needed to distinguish a first launch from an intentionally empty directory and publish changes only after the write succeeded.

For contacts without a last name, sorting and sectioning fall back to the first name. Accented Latin initials fold into their base letter; other initials use `#`. Birthdays are preserved when editing but are not editable because the specified editor fields omit birthday. Favorites use a compact horizontal section so the 31 initial favorites do not push the alphabetical directory many screens down.

Read the skeleton README and inspected the supplied JSON; no external documentation or web lookup was needed. The Swift compiler's macro service and CoreSimulator required running the provided build/simulator commands outside the shell sandbox.

## Limitations and what would help

All requested app features are implemented. Real calls and email composition cannot be verified on this simulator; the app opens system URLs and offers copy actions, with an explanatory fallback if the device cannot handle them. The supplied toolkit has no touch automation, so validation combines simulator-run data checks and rendered-screen inspection rather than automated end-to-end gesture tests.

A ready-made simulator touch/accessibility test driver and a preconfigured test target would make validation faster. A build command already configured for the compiler macro service and CoreSimulator sandbox requirements would also remove setup friction.
