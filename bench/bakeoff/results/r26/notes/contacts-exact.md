# Contacts — build notes

Finished and installed as `dev.bakeoff.contacts.b491b7d` on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7` using `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios`.

All requested features are complete. The supplied 320 records are bundled through the imported `data/contacts.json`. Changes use an atomically written local JSON file, including an empty directory after deletion. No network or account is required.

## Verification

- Final `bun exact.mjs verify ios`: clean; 9 tests passed, 0 failed.
- Light and dark appearance: no findings.
- Largest Dynamic Type size (53-point body), 11 native screens: nothing spills.
- Native scrolling: nothing for the app to fix. Exact reported its own status-bar scroll-to-top timing difference; the busy machine prevented judging absolute timing.
- Real simulator touches confirmed the custom letter index jumps to the correct section.
- Web verification: all 9 functional tests passed. Its largest-text scrolling tour still reports two offscreen swipe-row label findings. Manually reproducing the search sequence showed no visible clipping; the target iPhone verification is clean.

The tests cover seed loading, search by all four supported fields, clearing search, favorites, add/edit validation, first-name-only and last-name-only contacts, preserving birthdays and favorites during edits, cancel, both deletion paths, missing contacts, and relaunch persistence.

## What was hard and what I looked up

- Read Exact's start guide and used the list-detail pattern, then looked up grouped forms, editing, persistence/grants, assets, native views, and simulator driving.
- Exact's automatic section index requires every section heading to be one letter. A named Favorites section disables it. Added a small UIKit index with tap, continuous sliding, haptics, and VoiceOver adjustment, plus a web counterpart.
- Sticky plain-list section headings produced overlap findings at the largest text size. Native grouped sections keep the headings clear of names.
- Large text exposed intrinsic-width overflow in centered names. Names stretch to their available width and only break an overlong word when necessary. Decorative avatar geometry stays compact.
- Native relaunches return to the directory. Persistence tests explicitly reopen the saved contact. Native search also hides navigation controls while editing, so tests follow separate browse and search workflows.
- Native test activation needed a short display-frame pause after navigation on this heavily loaded host. The first native build was slow; later baked-plan iterations were much faster.
- The shared toolkit compiler initially needed sandbox escalation to create its build lock.

## Decisions

Favorites appear at the top and remain in the complete alphabetical directory. Contacts without a last name sort and section by their first name. A non-letter initial goes in `#`. Unknown birthdays are omitted; editing the five requested fields preserves any existing birthday and favorite value. Phone and email use `tel:` and `mailto:` links; completing calls depends on device capabilities.

## What would have made this faster

An explicit native list index-title API that permits a Favorites section, prebuilt native toolkit artifacts, clearer guidance on search-controller and relaunch behavior in native tests, and fewer web-only findings for hidden swipe-row content.

No required iPhone feature remains unfinished.
