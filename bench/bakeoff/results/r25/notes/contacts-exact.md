# Contacts — build notes

Implemented the complete iPhone specification with Exact: 320 bundled starting contacts; last-name sections and native section index; live name/company/phone/email search; detail screens and phone/email links; favorites; validated add/edit sheets; list swipe deletion and confirmed detail deletion; atomic local persistence.

## Decisions

- Favorites appear in a separate top section and also remain in the alphabetical directory. The index’s first `#` jumps to favorites. Exact requires one-character section headers to provide its native index, so that section has a separate “Favorites” title row.
- Missing company and birthday values are normalized from null and omitted from details. Existing birthdays and favorite status survive edits. Birthday formatting uses UTC to avoid shifting the calendar date.
- A first or last name is sufficient, whitespace-only names cannot save, and names without an A–Z initial go under `#`. Phone search also accepts digits without formatting.
- The bundled JSON is imported into the data module. A missing saved file starts from that dataset; an intentionally empty saved directory stays empty. Writes are serialized and atomic.

## What was hard / looked up

- Read Exact’s required start guide, list-detail starter, and references for native lists, indexed sections, sheets, forms, color roles, storage, and simulator driving.
- Cold compiler/native-host preparation took several minutes and needed access to the configured toolkit’s build caches.
- Exact’s plain-list pinned headers produced overlap findings while scrolling at the largest text size. Native grouped sections preserved the index and eliminated those overlaps.
- Native relaunch returns to the list, while web test navigation is restored differently. Persistence tests explicitly reopen the saved contact after relaunch.
- Some color-role spellings in examples differed from the active schema; compiler messages supplied the accepted values.

## Validation

- `bun exact.mjs verify`: all 8 functional tests pass. The exploratory web renderer still reports clipping at scrolled viewport edges; this is an iPhone deliverable.
- `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios`: **clean**, 8 tests passed, no light/dark layout findings, and nothing spills at accessibility extra-extra-extra-large (53-point body text).
- Native scrolling verification reported nothing for the app to fix. It noted a toolkit-owned status-bar scroll-to-top timing difference and high machine load during timing measurement.
- Simulator screenshots and detailed reports are under `.exact/`.

## Completion and improvements to the toolkit

All required iPhone functionality is complete. Real phone calls and mail composition depend on the system handlers; the app provides native `tel:` and `mailto:` links.

Building would have been faster with a named favorites header compatible with the native section index, clearer reload-navigation guidance across hosts, and fewer false overlap/clipping findings around native scrolling headers and web scroll edges. The tested starter and native-control mappings were useful.

Final touch checks confirmed keyboard entry, saving with the keyboard visible, detail navigation, native swipe-action reveal, and deletion of the temporary test contact. The default store was reset and checked to contain 320 contacts with an empty search.

The final `bun exact.mjs ios --run` built, signed, installed, and launched `dev.bakeoff.contacts.bdd3e9c` as **Contacts** on simulator **486FD498-0E3E-4B64-8E4B-152BD8265EE7**. No other simulator was used.
