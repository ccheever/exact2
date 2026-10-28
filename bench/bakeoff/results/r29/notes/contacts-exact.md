# Contacts build notes

Finished and installed as **Contacts** (`dev.bakeoff.contacts.b946f13`) on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The final installation was launched with `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios --run`.

All requested features are complete. The supplied 320 contacts are bundled through the JSON import in `app.ts`. Saved changes use a separate, atomically replaced application data file; an empty saved directory stays empty rather than reseeding.

## Decisions

- Favorites appear at the top and also remain in the alphabetical directory.
- A first-only contact is indexed by its first name. Accented initials are folded to their base letter; other initials go under `#`.
- Optional values are omitted from details when empty. Birthdays are formatted without a timezone day shift and preserved when editing the five specified fields.
- Names are required after trimming whitespace. Duplicate names are allowed.
- The compact alphabetical rail has individually labeled accessibility actions. A native Jump to Letter menu provides full-size alternatives, and the rail scrolls at large text sizes.

## What was hard and what I looked up

I used Exact’s start guide, list-detail pattern, grouped-form and persistence recipes, native list/index implementation, and simulator-driving documentation. Native automatic alphabetical indexing requires every section header to be one letter, so it cannot directly include a named Favorites section. The app therefore supplies its own index and native menu.

The first prebuild caught a temporary mismatch between the manifest ID and the data-module ID. Changing assets during a build also invalidated its bake receipt. Keeping both IDs synchronized before prebuild and freezing inputs during verification avoided these rebuilds.

UIKit trims trailing whitespace when measuring adjacent labels. Explicit layout spacing was necessary to display first and last names correctly while allowing them to wrap independently.

## Validation and limitations

The final `bun exact.mjs verify ios` reported **11 passed, 0 failed; verify: clean**. It checked light and dark mode, all test-end screens, relaunch persistence, the largest accessibility text size, and real-touch scrolling. Nothing spilled at the largest size. Tests cover all search fields, both ways of changing favorites, add/edit/cancel, required-name validation, optional birthdays, deletion from both locations, and persistence.

Verification explicitly accepts the compact index targets and native rows passing under clipped section headers. The full-size letter menu is available for users who need larger targets. Earlier web-host checks also reported the geometry of hidden swipe actions; native deletion was tested successfully.

No requested feature remains unfinished. Exact’s scrolling report identifies a framework-level status-bar scroll-to-top difference (about 1.07 seconds versus UIKit’s roughly 0.94 seconds). The final scrolling probe found nothing for the app to fix. Machine load was very high, so the tool judged scrolling physics rather than timing.

Building would have been faster with automatic indexing that supports a named Favorites section, documentation that emphasizes updating both app IDs before prebuild, and checker handling for native clipped section-header transitions and compact index controls.
