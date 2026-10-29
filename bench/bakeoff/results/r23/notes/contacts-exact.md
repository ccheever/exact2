# Contacts build notes

Finished and installed as **Contacts** (`dev.bakeoff.contacts.bdf9d78`) on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7` using `bun exact.mjs ios`.

## Implementation and decisions

- All 320 starting contacts from `data/contacts.json` are embedded through the data module. The original data file is unchanged.
- Changes use a separate local JSON file, atomic writes, and a shared write queue so overlapping edits cannot overwrite each other. An empty saved directory stays empty on relaunch.
- Favorites appear at the top and also retain their place in the full alphabetical directory. Contacts without a last name sort by first name; nonalphabetic initials use `#`.
- Search includes names, companies, email addresses, and phone numbers with or without formatting. Optional blank fields are omitted from details.
- Birthdays remain intact when editing. The editor includes the five fields requested in the specification.
- Phone and email rows use native `tel:` and `mailto:` links. Actual calls and email sending were not exercised in the simulator.

## What was hard and what I looked up

I used Exact's local start guide, list-detail pattern, and references for grouped forms, search, swipe actions, native views, scrolling, file grants, and iOS driving.

Exact has no built-in alphabetical index, so a small Swift module uses UIKit's actual section-index control. Real-touch testing found that scrolling to a section header did not move the native list; targeting its first contact worked.

UIKit hides navigation buttons during active search. Search results now include an Add Contact action, and leaving the directory dismisses its active search controller. Native grouped sections avoid pinned-header overlap at the largest text size. Phone and email values needed explicit semantic body fonts to scale correctly.

The first native build compiled a substantial dependency graph. Even vocabulary commands needed write access to the toolkit's shared compiler directory.

## Validation and remaining limitations

- Final `bun exact.mjs verify ios`: **clean**, 10 tests passed, zero failed.
- Light and dark layouts: no findings.
- Largest accessibility text size, body 53 pt: nothing spills.
- Native scrolling physics: “like UIKit.” High machine load prevented an authoritative timing comparison.
- Verified adding first-only and last-only names, whitespace validation, editing different contacts, retaining birthdays/favorites, searching, favorite toggles, both deletion paths, cancellation, and relaunch persistence.
- Web functional tests also pass all 10 cases. Its layout checker still reports findings around the index overlay, hidden swipe controls, and large-text navigation. The requested iPhone app passes its native verification.

No requested iPhone feature remains unfinished.

## What would have made this faster

A built-in indexed-list property, a documented recipe for adding while native search is active, clearer section-header scrolling behavior, a compiler usable without shared-directory writes, and incremental progress from native verification would have saved time. The native navigation/list/form starter and fast Contract plan updates were useful.
