# Contacts build notes

Completed and installed as **Contacts**, bundle ID `dev.bakeoff.contacts.b4f414c`, on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`.

## What works

All specified features are implemented: 320 bundled starting contacts, surname sorting, alphabetical sections and native section index, live search, contact details, favorites, add/edit forms, swipe deletion, confirmed detail deletion, and persistence through relaunch. The original JSON is imported into the shipped data module. Changes use atomic writes to the app's private data directory. The default simulator store was reset to the original 320 contacts and 31 favorites before the final build.

## What was hard and what I looked up

- Started from Exact's `list-detail` pattern and read its start guide, search, swipe, edit-sheet, grouped-form, navigation-stack, file-storage, and iOS-driving documentation.
- A form component's navigation attributes must be on the component root that directly holds its header. An extra wrapper prevented native navigation-bar placement; the compiler's diagnostic explained the fix.
- The native alphabet index disappeared when a footer-only section followed the letters. Looking at Exact's grouped-list implementation showed that every section after the first letter must also be a letter section containing rows. The contact count now lives in the final letter section's footer.
- A query-dependent resource caused a retained-answer diagnostic after relaunch. Search now filters the stable, sorted directory locally, giving immediate results and avoiding that cache mismatch.

## Decisions

- Favorites also remain in their alphabetical sections. The displayed count counts each person once.
- A contact with only a first name sorts and groups by that name. Names are trimmed, and whitespace alone cannot be saved.
- Optional values are omitted from details when absent. Editing the five requested fields preserves the original birthday and favorite status.
- Search ignores case and accepts unformatted or commonly formatted phone numbers. Phone and email are native blue links with telephone/email URLs.

## Validation and limitations

- Final source: `bun exact.mjs contract build app.contract --json --errors-only` passed with no errors.
- Final simulator suite: `bun exact.mjs test ios` passed **9 tests, 0 failures**, covering the seed data, search, favorites, creation, editing, both deletion paths, cancellation, missing records, optional birthdays, and relaunch persistence.
- `verify ios` found no layout problems in light or dark appearance; all 11 checked screens fit at the largest accessibility text size. Native sheets, keyboard behavior, details, dark mode, and the alphabet index were inspected in simulator screenshots.
- Real-touch scrolling found nothing for the app to fix. Timing was not judged because the machine was heavily loaded; Exact separately reported a toolkit-owned status-bar scroll timing difference.
- Exact's additional randomized two-engine semantics checker reported internal disagreements using synthetic resource answers. Its synchronous oracle also cannot execute reload steps. These extra checks did not produce a clean overall verification result; the nine actual simulator workflows passed. The earlier web verification also reported web-specific layout findings; this deliverable targets iPhone.
- No requested app feature remains unfinished. Actual phone calls and email delivery cannot be exercised as real communications in this simulator.

## What would make this faster

Document the alphabet-index/footer constraint beside the pattern, include a complete screen-component form example, clarify retained-resource diagnostics across reload, and make documentation lookups independent of writable compiler caches. Exact's fast plan updates and native controls saved substantial implementation work.
