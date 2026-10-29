# Paperback Settings — notes

Built with Exact (`app.contract`, `app.tamber`, `app.test.contract`). Installed on simulator
EEFC0029-… as `Settings` (`dev.bakeoff.settings.b9239c8`). The 11 tests pass on web and iOS,
`verify` on the web reports only the issue described below, and `verify ios` is clean (light,
dark, largest text size, relaunch, scrolling).

## What's there
- One large-title grouped list with these sections: Account, Reading, Appearance, Library,
  Notifications, Storage, About, and Reset All Settings.
- Every setting is a root `state … persist`, saved to UserDefaults on iOS. A `mount` task
  re-applies the saved theme at launch with `setScheme`.
- The Font, Theme and Sort By pickers each push a list of choices with a checkmark on the
  current one, the way iOS Settings does. Each font is shown in its own typeface. Line
  spacing is a segmented control.
- The preview paragraph uses the chosen font family, size and line height. Sepia gives the
  preview a paper-coloured page and forces the app into light mode. System, Light and Dark
  set the scheme for the whole app.
- Sign Out, Clear Cache, Rate the App and Reset All Settings each open the system alert.
  - After you sign out, the Account section shows a Sign In alert with an email field.
  - Clear Cache sets the cache to "Zero KB" and disables its button.
  - Reset puts every reading, appearance, library and notification setting back to its
    default. It does not touch the account or the storage figures, and the footer says so.
- Turning off Allow Notifications hides the reminder time picker (`UIDatePicker`) and the
  three toggles.

## Hard or guessed
- `path()` only accepts a literal route name, so I wrote one action per pushed screen.
- `footer` can't sit inside an `if`. A footer that changes has to be a conditional text.
- At the largest text size the segmented control cut off its labels. On iOS, when
  `exactViewport().accessibilityTextSize` is true, the control becomes checkmark rows,
  as UIKit apps do. The web has no such signal, so web `verify` still reports the cut-off
  labels there. The iOS check is clean.
- Some choices weren't in the spec, so I made them myself:
  - the five fonts: Georgia, Iowan Old Style, Palatino, Charter and Avenir Next, all built
    into iOS;
  - the sort orders and the three notification toggles;
  - the storage sizes, which are made up (there is no real cache);
  - what Rate the App does: there is no App Store on the simulator, so it shows an alert
    and then a thank-you footer;
  - what Reset covers.
- A typed-in text size looked wrong in the agent's `prefer root-font-size` drive (only my
  own styled text grew). `verify ios` uses real Dynamic Type and found nothing.

## Not done
- No real sign-in, App Store review prompt or cache. The spec has no other screens, so
  these are simulated.

## What would have made it faster
- An example of a grouped-list radio picker (a pushed checkmark list) in the
  `grouped-form` recipe.
- An error from `path()` that suggests the one-action-per-route pattern.
