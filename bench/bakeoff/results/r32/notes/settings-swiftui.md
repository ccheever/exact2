# Paperback Settings — notes

## What's built
A single SwiftUI `Form` in a `NavigationStack` with these sections: Account, Reading (font size slider, font picker, line-spacing segmented control, live preview), Appearance, Library, Notifications, Storage, About and Reset All Settings.
- **Pickers:** Font and Sort By use `.navigationLink` style. They push a checkmark list, as iOS Settings does, and each font name is drawn in its own face. Theme is an inline checkmark list with colour swatches.
- **Theme:** System, Light, Dark or Sepia. It drives `preferredColorScheme` for the whole app. Sepia forces light mode, gives the app a warm brown tint and puts the preview on sepia paper.
- **Preview:** shows the chosen font at the exact point size (fixed size, so it matches the slider at any Dynamic Type setting), with line spacing equal to the font size times a factor.
- **Confirmations:** Sign Out, Clear Cache and Reset All Settings all ask first, using destructive confirmation dialogs.
  - Signing out shows a "Not Signed In" state with a Sign In button.
  - Clear Cache sets the cache to zero and updates the used-space total. The button is disabled once the cache is empty.
- **Reset All Settings** restores Reading, Appearance, Library and Notifications to their defaults. On purpose, it does not touch the account name and email, the signed-in state, or the cleared cache: those are data, not settings, and the footer says so.
- **Persistence:** everything is stored in `UserDefaults`, written on every change in `didSet` and read back at launch. Bad stored values fall back to the defaults, and the font size is clamped to its range.
- **Text entry and accessibility:**
  - The name and email fields have the right content types and keyboard, Next/Done submit buttons, a keyboard Done button, interactive keyboard dismissal, and an inline warning for an invalid email.
  - At accessibility text sizes, the account header stacks vertically and the text fields align left.
- **About:** Rate the App calls the system `requestReview`.

## Testing
I checked light, dark and sepia, plus the largest accessibility text size, with simulator screenshots. There is no tap tool, so to reach lower sections and dialogs I added temporary launch-environment hooks: one to scroll to a section, one to open a dialog or change a value. I removed them afterwards. I confirmed persistence by reading the app container's preferences plist after a change made inside the app.

## What was hard / guessed
- **Persistence check:** the first attempt looked broken. `simctl spawn … defaults write` writes to the simulator's global domain. The app reads that only when its own container plist lacks the key, and writes go to the container. I lost some time before reading the container's plist directly.
- **Regex:** `swiftc` without `-enable-bare-slash-regex` rejects `/regex/` literals, so the email check uses `range(of:options:.regularExpression)`.
- **Guesses where the spec is silent:**
  - Content: the five fonts (San Francisco, New York, Georgia, Charter, Avenir Next), the font size range (12–32 pt, default 18), the sort orders, the names of the three extra notification toggles, and the storage figures.
  - Behaviour: what Reset covers, and what signing out does.
- **Rate the App:** whether `requestReview` shows a prompt in a sideloaded simulator build is up to the system. I didn't confirm it visually.

## Not done
- The segmented control's labels don't grow with Dynamic Type; that is a system limitation.
- There are no real notifications or permission prompts; the toggles only store preferences.

## What would have made this faster
A way to tap and scroll in the simulator (an idb- or XCUITest-style driver), so interactions could be tested directly instead of through launch hooks.
