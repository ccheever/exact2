# Paperback Settings — build notes

Built with Exact (`app.contract`, `app.tamber`, `app.test.contract`). It's installed on simulator
`CEDFB9C9-…` as `dev.bakeoff.settings.bf96bb6` ("Settings"). `verify` (web) and `verify ios`
both come back clean: 8 tests pass, light and dark, the largest text size, a relaunch and
UIKit-like scrolling.

## What's there
- **One inset-grouped list** with these sections: Account, Reading, Preview, Appearance, Library,
  Notifications, Storage, About, and Reset All Settings at the bottom.
- **Account:** editable Name and Email rows, and a Sign Out button that asks first. When you're
  signed out, the section shows a short message and a Sign In button.
- **Reading:**
  - A font size slider from 12 to 32 pt, with the current value shown.
  - Font is a pushed list with a checkmark. Each font name is drawn in its own face: New York,
    San Francisco, Georgia, Palatino and Iowan Old Style.
  - Line Spacing is a segmented control: Compact, Normal or Relaxed.
  - The preview paragraph and its caption change live as you adjust font, size and spacing.
- **Appearance:** Theme is a pushed checkmark list: System, Light, Dark or Sepia.
  - System, Light and Dark set the app's own scheme with `setScheme`, which is re-applied by a
    mount task at launch.
  - Sepia uses the light scheme and draws the preview in paper and ink colours from `app.tamber`.
- **Library:** Sort Order is a pushed list, plus two switches.
- **Notifications:** a master switch. When it's on, it reveals the Daily Reminder time (the iOS
  compact time picker) and three more switches.
- **Storage:** Used, Downloaded Books and Cache sizes. Clear Cache asks first, then shows "Zero KB"
  and disables itself.
- **About:**
  - Version.
  - Privacy Policy and Terms of Service push simple text screens.
  - Rate the App opens the App Store review URL with `openURL`.
- **Reset All Settings** asks first, then restores the defaults. It leaves the account and the
  cache alone, and its footer says so.
- **Every setting is a root `state … persist`** (UserDefaults on iOS), and the tests reload to check
  this.

## Hard parts, lookups and guesses
- **Fonts on iOS:** `system-ui` didn't resolve to SF on iOS, so "San Francisco" was drawn as a
  serif. Writing `-apple-system, system-ui, …` fixed it. `ui-serif` does give New York.
- **Slider row:** a row that has a `text` and a control is treated as title plus accessory, so the
  slider didn't stretch. Wrapping the "A / slider / A" in an inner `row flex=1` fixed it.
- **Segmented control at the largest text size:** on the web the labels were clipped. I added
  `flex-wrap="wrap"` on the radiogroup, which verify accepts on both hosts.
- **`after(0, …)` is refused:** you have to write `after(1, …)`. `aria-hidden` needs a bool, not
  a string.
- **Agent drives:** a plain `drag` on the root doesn't scroll. `tap <row-id> wheel 0 N` was the
  easy way to scroll the list for screenshots.
- **Guesses (the spec doesn't specify these):**
  - All the example values: name and email, storage sizes, version 2.4.1 (318), and the App Store
    id in the rating URL.
  - Which five fonts to offer.
  - That pickers push a checkmark list (iOS Settings style) and stay on that screen after a
    choice.
  - That Reset doesn't sign you back in or restore the cache.

## Not finished / limits
- **Rate the App** opens a placeholder App Store URL. There's no in-app review prompt, because
  Exact doesn't expose `SKStoreReviewController`.
- **Notifications aren't actually scheduled.** The settings are only stored.
- **Sepia only tints the preview.** The rest of the app uses the light scheme, since there are no
  book pages to re-theme.

## What would have made it faster
- A `settings` pattern: a grouped list with pushed checkmark pickers, a slider row and confirm
  alerts. That's the most common shape of app, and I had to piece it together from
  `grouped-form`, `screen`, `choice` and `confirm`.
- A note in the `font-family` vocab that `system-ui` doesn't resolve on iOS.
