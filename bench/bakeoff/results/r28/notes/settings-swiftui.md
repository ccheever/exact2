# Notes: Paperback Settings (SwiftUI)

## What's built
One `NavigationStack` with a grouped `List`, in this order: Account, Reading (with a live preview), Theme, Library,
Notifications, Storage, About and Reset All Settings. Every setting uses `@AppStorage`, so it persists across launches.
- **Pickers:** Font and Sort By push a checkmark list, the way the Settings app does. Each font name is drawn in
  its own face, and choosing an option goes back to the previous screen. Line spacing is a segmented control. At
  accessibility text sizes it becomes a pushed list, because the segments would truncate. Theme is an inline
  checkmark list with swatches.
- **Theme:** I set it with `overrideUserInterfaceStyle` on the window rather than with `preferredColorScheme`.
  That way, going back to System reliably follows the device again, and alerts and dialogs follow the theme too.
  Sepia uses the light appearance with warm backgrounds for the list, its rows and the pushed screens.
- **Confirmations:** Sign Out, Clear Cache and Reset all ask with a `confirmationDialog`. Signing out shows a
  "Not Signed In" state with a Sign In button. Clearing the cache sets it to 0 KB, and the button is disabled while
  the cache is empty. Reset removes only the setting keys. It leaves the account details and cache size alone, and
  the dialog says so.
- **Reminder time:** stored as minutes after midnight, because `@AppStorage` cannot store a `Date`.
- **Other details:** the email field warns about invalid addresses, has a keyboard Done button, uses
  `.next`/`.done` submit labels, and the keyboard dismisses interactively. Rate the App uses StoreKit's
  `requestReview`.

## What was hard or guessed
- I chose the exact options myself: the five fonts (New York, San Francisco, Georgia, Palatino, Avenir Next), the
  sort orders, the three notification toggles (Reading Streak, New Releases, Recommendations), and the default
  name, email and storage figures.
- A slider with `step:` draws tick marks on iOS 26, which looked noisy. I round the value in a binding instead.
- The preview's font size is a fixed point size, because the slider sets the size directly; it does not scale with
  Dynamic Type. The rest of the UI does scale, and I checked it at AX5.
- Storage figures are simulated, since the app has no real books or cache.

## Testing
No UI automation tools were available, so I couldn't tap. To test without tapping, I passed settings as launch
arguments (for example `-appearance.theme sepia`). I also added a small hook: a `-scrollTo <section>` launch
argument scrolls to that section. `shot.sh` relaunches the app with arguments and takes a screenshot. I checked:
light, dark and sepia; notifications on and off; an empty cache; another font, size and spacing in the preview; and
the largest accessibility text size.

I could not check the tap-driven flows on the simulator: the confirmation dialogs, the pushed pickers, the Privacy
and Terms pages, and Rate the App. They use standard SwiftUI controls.

## What would have made it faster
A way to send taps and swipes to the simulator (something like idb or `simctl io tap`) to test the interactive
flows end to end.
