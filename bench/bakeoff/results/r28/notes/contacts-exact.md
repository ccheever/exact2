# Contacts build notes

Implemented the supplied 320-person directory, last-name sorting and alphabetical sections, live search across name/company/phone/email, contact details, favorites, add/edit forms, deletion, and persistent local storage. The supplied JSON is imported into the bundled data module and used only when there is no saved contacts file. Edits preserve birthdays and favorite status.

Hard parts and lookups:
- Read Exact’s required start guide and looked up list-detail, grouped forms, editing, storage, iOS driving, native views, navigation layout, and swipe actions.
- Exact’s automatic native section index requires every section heading to be a single letter. A Favorites heading disables it. Added a small UIKit index control with drag navigation, selection haptics, and VoiceOver adjustable actions. The web preview uses an A–Z menu.
- Exact’s verification driver reveals hidden controls as it probes large text. Generated swipe-delete test identifiers caused it to leave rows shifted, and the web jump menu appeared over the native list during probes. Explicit accessible swipe actions and keeping the web menu out of the iPhone view tree resolved these.
- Large-text navigation headers needed wrapping in the web preview. The profile header uses scalable type and an initials avatar. Grouped directory sections keep headings from floating across large contact labels.
- One native relaunch test needed additional settling time before tapping the newly mounted favorite control on a busy simulator.
- Native build caching needs access to the toolkit checkout outside the app folder. The cold build took several minutes; subsequent builds were much faster.

Decisions:
- People with only one name sort under that name; nonalphabetic initials are grouped under #.
- Favorites are also retained in their normal alphabetical sections.
- Phone search accepts digits without punctuation. Phone/email details use tel: and mailto: links.
- Birthday is displayed when known and retained during edits; the requested editing form contains the five specified fields.
- Duplicate names are allowed. Save requires at least one nonblank name. Optional fields may be empty.

Validation:
- Seven functional tests cover the seed data, all search fields, no results/clearing, details with and without birthdays, validation, first-name-only and last-name-only forms, edits/cancellation, favorites, deletion, and relaunch persistence.
- `bun exact.mjs verify`: clean, all seven tests pass; light/dark, two viewport widths, relaunch, and largest text checks pass.
- Native rail tested with real simulator touches on the assigned device.
- `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios`: clean. All seven native tests pass; light/dark, relaunch, and accessibility-extra-extra-extra-large (53 pt body) checks pass.
- Native scrolling reports nothing for the app to fix, with zero measured hitches. The host machine was heavily loaded, so Exact did not judge timing. It separately reported a toolkit-level status-bar scroll-to-top timing difference.
- Final app built and installed with `bun exact.mjs ios` on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`; no other simulator was used.

What would make this faster:
- A native section index that supports named sections such as Favorites.
- Verification probes that recognize intentionally hidden swipe/menu actions.
- More examples for accessible custom native views and native/web differences.

No requested app feature is intentionally omitted.

Final normal-launch check: the default store opens with an empty search and all 320 starting contacts. The verified app remains installed on the assigned simulator.
