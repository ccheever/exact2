# The desktop modules by hand (LLP 1133.025)

`desktop.mjs` drives what the agent can on this Mac (`bun
packages/dock/test/desktop.mjs macos`), and the absent cells (`web`,
`linux`). This list is what needs a person: the system's own UI, a real
login, and Windows. Each step names what to see; anything else is a
finding. Run `desktop.mjs macos --only 1 --keep` to keep the fixture app it
built, then launch that app's bundle by hand (its path is printed).

## Mac

1. **Status item.** The fixture's star appears in the menu bar, tinted for
   the menu bar's appearance (light and dark). Click it: AppKit's menu with
   Open Desktop Fixture, New Note, a separator and a dimmed Unavailable.
   Open brings the window forward. The window has no star button in it.
2. **Drag it.** ⌘-drag the star to another place in the menu bar, quit and
   relaunch: it comes back where it was left (`autosaveName`).
3. **Close keeps running.** Close the window with the red button: the app
   keeps running (the star stays, no window). Open from the star shows it
   again. Hide Menu Bar Item, then close the window: the app quits.
4. **Dock menu.** Right-click the fixture's Dock icon: New Note above
   AppKit's own items; choosing it raises "composed" by one.
5. **Attention.** Request Attention, then switch to another app within a
   second (or use `requestAttention(critical=true)`): the Dock icon bounces
   once (informational), or until the app is activated (critical).
6. **Login item.** Open at Login, Hidden: System Settings › General › Login
   Items lists the fixture, and macOS shows its own notice. Log out and in:
   the fixture starts with no window and its star in the menu bar; Open
   shows it, and the login line reads "launched at login". Record whether
   `keyAELaunchedAsLogInItem` was seen (D6, Q9): "launched at login"
   present means it was. Then Don't Open at Login, and confirm Login Items
   no longer lists it. Never leave the fixture registered.
7. **Global shortcut.** With another app in front, press ⌃⌥⌘K: "Presses"
   goes up by one, the other app does not see the key. Use ⌃⌥⌘J moves it:
   ⌃⌥⌘K no longer reaches the app.
8. **Modifier pair.** In a build whose fixture reads `Shift+Shift`, with
   Accessibility not granted to the app: `not registered not-allowed`.
   Grant it in System Settings › Privacy & Security › Accessibility and
   relaunch: holding both Shift keys presses once.
9. **Menu-bar app.** `desktop.mjs macos --only 8 --keep`, then launch the
   kept `menubarfixture` bundle: no Dock icon, no menu bar, no window; the
   star's menu ends with "Quit menubarfixture", which quits it.

## Windows (waits for the NucBox)

Built for Windows so far: the host's services (`host/windows/src/desktop.rs`:
`RegisterHotKey`, `FlashWindowEx`, the `Run` value) and the three packages'
Rust (`packages/*/windows/lib.rs`), cross-checked on this Mac only. Not
built: the notification-area icon, `keepRunning`, the launch with no window,
one instance, the startup task and the modifier pair's hook (LLP 1133.025
§14, stage 2's as-built note). Run these in the NucBox's signed-in console
session (`docs/testing.md`, "Windows: the NucBox"), not over SSH, with a
development build of an app that depends on the three packages.

1. **Global shortcut.** With Notepad in front, press the app's chord
   (`Control+Alt+Meta+K` is Ctrl+Alt+Win+K): the app hears it once, and
   Notepad does not see the key. Hold it down: still once (`MOD_NOREPEAT`).
   A chord Windows keeps (Win+L) reads `not registered reserved`; one
   another app holds reads `in-use`.
2. **Attention.** Request Attention, then click Notepad within a second:
   the app's taskbar button flashes three times; with `critical=true` it
   flashes until the app comes forward. With the app in front the journal
   says "the window is in front" and nothing flashes.
3. **Login item.** Open at Login, Hidden: `reg query
   HKCU\Software\Microsoft\Windows\CurrentVersion\Run` lists the app with
   `"<exe>" --login hidden`, and Settings › Apps › Startup lists it, on.
   Turn it off there: the status reads `requires-approval`. Sign out and
   in with it on: the app starts and reads "launched at login" (a window
   shows: the launch with no window is not built). Then Don't Open at
   Login: the value is gone. Never leave it registered.
4. **Login Items Settings…** opens Settings › Apps › Startup.
