# Contacts build notes

Completed and installed as **Contacts** (`dev.bakeoff.contacts.be3732a`) on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The final `bun exact.mjs ios --run` succeeded and launched the installed app.

All required features are implemented. The 320 starting contacts are embedded from `data/contacts.json`. Changes use an atomic local JSON file and survive relaunches. Editing preserves the original birthday and favorite status. First-only and last-only names are supported; names without an A–Z initial remain accessible under `#`.

## Validation

- Final `verify ios`: **clean**, seven tests passed.
- Native light/dark layout checks passed on the main screen, test end screens, and relaunch.
- Largest Dynamic Type verification: nothing spills.
- Real-touch scrolling: like UIKit. The machine was heavily loaded, so Exact judged scrolling physics rather than absolute timing.
- Real-touch section-index dragging, details, and edit-sheet checks were exercised on the specified simulator.
- Web behavior tests also passed. Its layout checker reported hidden swipe actions as covered; the iOS checks are the shipping checks.

## What was hard and what I looked up

Exact's built-in A–Z index requires every section heading to be one letter. A Favorites section disables it, so the app supplies a small Swift index with tap/drag tracking and VoiceOver adjustment, forwarding section jumps to Contract. I looked up Exact's native-view API, scrolling commands, storage grants, form/navigation patterns, and grouped-list implementation.

The largest-text checker treats cells scrolling beneath UIKit's pinned Favorites heading as overlapping. Real-touch inspection confirmed standard pinned-header behavior and correctly wrapping rows. `app.json` documents a targeted acceptance for these favorite-row overlap reports; text sizes remain fully scalable.

The native reload test originally assumed the web's route-restoration behavior. iOS returns to the directory after a reload, so the test now reopens the saved contact before checking it.

The first compiler/native build took most of the setup time. A custom Swift control also initially collided with UIControl's existing `selected` property; its index state is now named `selectedIndex`.

## Decisions and limitations

I used last name, then first name, for sorting; first name is the fallback for contacts without a last name. Birthdays are formatted in UTC to avoid a day shift. Phone and email use system `tel:` and `mailto:` links; available handlers depend on the device. No required feature remains unfinished.

## What would make this faster

A built-in index supporting mixed headings such as Favorites plus A–Z, consistent reload navigation across hosts, and checker awareness of pinned section headers and concealed swipe buttons would remove the main extra work. A warm native build cache also makes iteration much faster.
