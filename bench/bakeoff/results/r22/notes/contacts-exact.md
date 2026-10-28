# Build notes

Contacts (`dev.bakeoff.contacts.b15c0fb`) is built and installed on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. No other simulator was used. The final default store was reset to the original 320 contacts and an empty search.

The app includes the supplied JSON through its bundled data module. It seeds from that dataset only when no saved file exists; an empty saved directory remains empty. Changes use Exact’s atomic file writes. All requested features are implemented: surname sections, a draggable alphabetical index, live search, details and birthday display, phone/email links, favorites, add/edit, deletion from rows and details, and persistence.

What was hard and what I looked up:

- Read Exact’s start guide and started from its list-detail pattern. Looked up search, swipe deletion, grouped forms, edit sheets, files/grants, navigation, native views, scrolling commands, and simulator driving.
- Exact has no built-in alphabetical section index. Added a small Swift native view, with touch scrubbing and an adjustable accessibility element. Real simulator touches verified letter selection and jumping. The scrolling target must be an actual row, rather than its section container.
- The form’s navigation metadata must be on the component root for its header to become a native sheet navigation bar.
- Plain-list pinned headings showed text through their background and triggered overlap findings at the largest text size. Native inset-grouped sections avoid that issue. Rows contain one native name label, which also reduced scrolling work.
- Native transitions need `clock settle` in tests. A native relaunch returns to the directory, while a web reload can preserve the detail URL. Persistence tests explicitly return to the directory before reloading.
- Agent hot plans do not update the installed build’s receipt. Explicit `bun exact.mjs ios` installation was needed before native verification.

Decisions where the specification left room:

- Favorites appear both in their top section and their normal surname section, keeping every contact in the alphabetical directory.
- A contact without a surname sorts under its first name. Nonalphabetic initials use `#`.
- Duplicate names are allowed; identity is the contact ID. Names and other form values are trimmed. Either name is required; all other form fields are optional.
- Editing preserves birthdays and favorite status. The specified edit form does not include birthday entry.
- Details deletion asks for confirmation; list swipe deletion acts immediately. Phone and email use native `tel:` and `mailto:` links.

Validation:

- Final web behavior tests: **11 passed, 0 failed**.
- Final `verify ios`: **11 passed, 0 failed**; no normal light/dark layout findings; no spilling on 13 screens at the largest accessibility text size.
- Additional real-touch index dragging and final light/dark checks passed. Screenshots and measurements are in `.exact/`.
- Final `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios` completed successfully.

Remaining limitation: Exact’s native scrolling probe still reports five renderer performance findings despite the minimal single-label rows and absence of scroll handlers, images, shadows, or custom row drawing. It measured 9.08 ms/frame main-thread p95, tracking ratio 0.956, three frames of touch-to-content lag, two deceleration offset jumps, and 1.08 s status-bar scroll-to-top. No requested feature is missing, but scrolling does not exactly match the toolkit’s UIKit benchmark. The full report is `.exact/verify/ios-feel/feel.json`; no feedback was sent externally.

Building would have been faster with a built-in section index, clearer documentation of installed plans versus agent plans, progress output during the native audit, and better native list scrolling performance. The shared pattern and cross-platform behavior tests were useful.
