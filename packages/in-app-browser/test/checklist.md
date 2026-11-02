# The in-app browser by a person

What no tool can do here: look at Safari's view and a Custom Tab over the
app, close them, and see the default browser open. The agent drives the
rest: the `browser` ticket on iOS and Android, and the link followed
where the page does not show over the app (macOS, the web). Write down
the commit, the device and its OS version beside each result.

A chat app that carries the module (`<app>` below) opens a page from an
embed's title and an attachment's row. Start its fixture server first, which serves a channel whose
messages have link embeds:
`bun apps/<app>/fixture/server.ts --world transcript --reset-on-identify`.

## iPhone or the iOS simulator, iOS 17 or later

`bun host/apple/build.mjs --ios <app>-apple --run` (a simulator;
`--device --run` on a phone).

1. **Over the conversation.** Open #general and scroll to a message with a
   link preview ("exact: keep the visible row anchored"). Tap its title:
   Safari's view slides up over the conversation with Safari's own bar and
   a Done button, at github.com.
2. **Done returns.** Tap Done: the view goes and the conversation is where
   it was, at the same message.
3. **One at a time.** With the page shown, nothing else in the app can be
   tapped; the next tap after Done opens again.
4. **The app reads nothing.** Sign in to a site in the page, close it, open
   Safari: The app never sees the page's cookies (nothing to see; this is
   the platform's).

## Mac

`bun host/apple/build.mjs <app>-apple --run`.

5. **The default browser.** Click the same title: the page opens in the
   Mac's default browser, beside the app, and the app stays as it was. No
   sheet or web view appears in the app.

## Android phone with Chrome, Android 12 or later

`bun scripts/agent-android.mjs build <app> --apk`, then install the APK.

6. **A Custom Tab.** Tap the title: a Custom Tab opens over the app with
   Chrome's toolbar, the page's title and a close (×) button.
7. **Close returns.** Tap ×: back in the conversation at the same message.
8. **No Custom Tabs browser.** With only a browser that provides no Custom
   Tabs (disable Chrome, install one without them), tap the title: the
   page opens in that browser; the app's `inAppBrowserStatus` reads
   `overApp` false after returning to the app.

## Web (Chromium, Firefox, Safari)

`bun host/web/dev.mjs --app <app>`.

9. **A new tab.** Click the title: the page opens in a new tab or window,
   as the browser chooses, and the app's tab stays.
