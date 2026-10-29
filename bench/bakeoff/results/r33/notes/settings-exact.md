# Paperback Settings: notes

Built with Exact (pin 05b2942866). The app is `app.contract`. Every setting is a root `state … persist`,
so `app.ts` has no sources. It is installed on simulator 486FD498… as `dev.bakeoff.settings.b449cb4` ("Settings").

## What's there
- **Layout:** one inset-grouped list under a large "Settings" title, with sections for Account, Reading,
  Appearance, Library, Notifications, Storage, About and Reset.
- **Account:** editable Name and Email, and Sign Out behind an alert. Once signed out the section shows
  "Not signed in" and a Sign In button.
- **Reading:** a font size slider (12–32 pt) with the value shown, and a Font picker that pushes a list of
  five fonts (San Francisco, Georgia, Charter, Palatino, Avenir Next). Each font name is drawn in its own
  face, with a checkmark on the current one. There is a Compact/Normal/Relaxed segmented control, and a live
  preview paragraph that follows the font, size and line spacing, with a caption underneath that names them.
- **Appearance:** System, Light, Dark and Sepia as rows with checkmarks.
  - System, Light and Dark switch the whole app's scheme with `setScheme`. A `mount` task re-applies the
    saved choice at launch.
  - Sepia keeps the app light and turns the preview into warm paper with brown text.
- **Library:** a Sort By picker that pushes its four choices. Show Finished Books and Download over Cellular
  are switches.
- **Notifications:** a master switch. When it is on, it reveals a Daily Reminder time picker and three more
  switches.
- **Storage:** used, books and cache sizes. Clear Cache asks first, sets the cache to zero and is then
  disabled.
- **About:** the version, plus Privacy Policy and Terms of Service, which push simple pages with a "Settings"
  back button. Rate the App opens the App Store review URL with `openURL`.
- **Reset All Settings:** asks first, then restores every reading, appearance, library and notification
  default. It deliberately leaves the account and the cache alone, and the footer says so.
- **Tests:** 9 tests in `app.test.contract`, covering:
  - the defaults
  - the live preview
  - both pickers
  - revealing the notification rows
  - relaunch persistence of every kind of setting
  - the three confirmations
  - the pushed pages
  - reset, including after a relaunch

  `verify` (web) and `verify ios` both pass. iOS reports no findings at the largest text size, and scrolling
  is "like UIKit".

## What was hard or guessed
- **Preview background:** a `background-color` on a column placed directly in a native list row is silently
  dropped (padding too). Wrapping it in a `view` fixed it. I found this by trial on the simulator; no
  diagnostic pointed to it.
- **Sepia colours:** Sepia needs literal colours. The compiler warns (`lower-colour-one-scheme`) because they
  don't change in dark mode, but Sepia forces the light scheme, so the warnings are expected and stay. That
  is the 4 compiler warnings. The first sepia caption colour failed web contrast; I darkened it.
- **Compile fixes:**
  - `push(nav, path(name))` with a computed route name is refused, so each page has its own action.
  - `after(0, …)` is refused; it has to be `after(1, …)`.
  - `aria-hidden` takes a bool.
  - `font-weight` takes a number.
- **Duplicate testId:** I first gave the section and its master switch the same testId ("notifications"),
  so taps went to the section. Renamed.
- **Line Spacing at the largest text size:** web verify flags the segmented labels as clipped at 53 px. On
  iOS, UISegmentedControl shrinks its labels and `verify ios` reports nothing spilling, so I left it.
- **Guesses:** the font list, the defaults (Georgia 18 pt, Normal spacing, notifications on at 9:00 PM), the
  storage numbers and the App Store URL are mine. The spec gives no data.

## Not finished / caveats
- **Rate the App** opens an App Store URL; the simulator has no App Store app, so it opens Safari or fails.
  The docs say `requestReview()` must never be a button.
- **contract verify:**
  - It can't drive `back`, `reload` or slider payloads, so several tests are only partly model-checked.
  - It notes that its oracle doesn't toggle the notifications switch in the test that taps it. The real web
    and iOS runs pass that test.

## What would have made it faster
- A note in `grouped-form` or the `list` vocab that paint on a row's direct child is dropped (wrap it in a
  `view`).
- A recipe for a settings "picker that pushes a checkmark list". It is the most common iOS settings idiom,
  and I composed it from `screen` and `choice`.
- A way to mark intentional fixed colours (like a sepia page) so the warning stays quiet.
