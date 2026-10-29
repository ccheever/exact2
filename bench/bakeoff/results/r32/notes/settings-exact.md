# Paperback Settings: notes

Built with Exact (`app.contract`, `app.tamber`, `app.test.contract`). Bundle id
`dev.bakeoff.settings.bfbbc5f`, name "Settings". It is installed on simulator
`CEDFB9C9-…`. 9 tests pass on web and iOS. `verify ios` is clean: light, dark,
largest text size and scrolling. `verify` (web) leaves one finding, described below.

## What's there
- One grouped list (`inset-grouped`) with these sections: Account, Reading, a preview card,
  Appearance, Library, Notifications, Storage, About and Reset.
- **Pickers.** Font (five fonts, each name drawn in its own face) and Sort By push
  a screen of checkmark rows, as iOS Settings does. Theme is four inline checkmark rows.
  Line spacing is a `UISegmentedControl`, and the time is a `UIDatePicker` pill.
- **Theme.** The theme calls `setScheme`, again from a mount task at launch. Sepia is light
  mode with a sepia page and ink on the preview card. "Sepia" is the reading surface's
  theme, and the footer says so.
- **Preview.** The preview follows font, size (12–28 pt slider) and spacing as they change.
  Its footer says "Georgia, 17 pt, normal spacing".
- **Notifications.** The master switch shows or hides the reminder time and three toggles.
- **Confirmations.** Sign Out, Clear Cache and Reset All Settings each ask first in a
  `UIAlertController`. Signing out swaps the account row for a "Sign In" button.
  Clearing the cache shows "Zero KB" and disables the button.
- **Persistence.** Every setting is a root `state … persist` (`UserDefaults`), so it
  survives a relaunch. Reset restores the setting defaults. It leaves sign-in and the
  cleared cache alone, because those aren't settings.
- **About.** Privacy and Terms push simple text screens. Rate the App `openURL`s an App
  Store write-review URL with a placeholder app id.

## What was hard / guessed
- **Card backgrounds in list rows.** A grouped list row is read by its shape. A row
  holding a column of one or two texts becomes a native title/subtitle cell, which drops
  that column's `background-color` and `padding` without any warning. I only found this
  in screenshots. The fix was to nest the paragraph one more `column` deep so the row
  counts as "custom". I also put the card in a `section background-color="transparent"`
  so it sits on the page like a card. The rule is in `contract-for-humans.md`, but
  `start-here` and the `grouped-form` recipe don't mention it.
- **Radio values.** A radio's `change` passes its `value` (a string), not a bool. The
  compiler's message explained this well.
- **testIds with spaces.** "font-Avenir Next" can't be tapped from the agent, so the ids
  are slugged.
- **Largest text, layout.** Account name and email moved into one profile row (icon,
  name, email as subtitle), because a value cell clipped the long email.
- **Largest text, line spacing.** The Line Spacing label now sits above its segmented
  control.
- **Largest text, web.** The web host scales the segment labels with the 53 px root and
  clips them. That is the one remaining web `verify` finding. iOS draws UIKit's segmented
  control and is clean, and iPhone is the target.
- **Guessed.** The five fonts (Georgia, Palatino, Charter, Avenir Next, Helvetica Neue)
  are ones installed on iOS. The default values, the storage figures, the version string
  and the policy text are made up.

## Not done
- The reminder schedules no real notification and asks for no notification permission.
  The spec only asks for the settings.
- Rate the App opens a URL. It does not call `requestReview()`, because the docs say that
  must never be behind a button.

## What would have made it faster
- A warning when a row's shape turns it into a native cell and drops authored paint.
- A `contract vocab` entry for "settings selection list" pointing straight at
  `input type="radio"` in a grouped row (I found it in the iOS mapping table).
- A `prefer` for Dynamic Type content size on iOS drives. `root-font-size` scales only the
  custom rows, not native cells.
