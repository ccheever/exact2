---
name: 20261005-browser-surface-profiles
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-surface-profiles
pr_url: https://github.com/ccheever/exact2/pull/354
verified_commit: null
---

# Browser surface part 4: profiles, cookie import and clearing

## Outcome

Browser tabs open under the reference's profiles: the built-in Default and Incognito and up to 24 named ones, each with
its own storage; the "+" menu's profile submenu (part 1 lists Default only) and the launcher's chevron list them; a
tab in another profile than the default shows its badge in the chrome row; the More menu's "Profile: <name>" group
clears cookies and cache for that profile only (part 1 draws Clear cookies and Clear cache disabled, marked "Part 4");
Settings › Integrations › Browser manages the profiles and the default one; the cookie import wizard brings cookies
from Chrome, Edge, Brave, Vivaldi, Opera, Arc, Helium, Firefox and Safari; and the rest of Settings › Integrations ›
Browser's defaults group (coordinator scope addition, 2026-10-09, confirmed by the repo owner: the settings defaults belong
to part 4): the default viewport, zoom and appearance a new tab opens with, the recording frame rate and key/mouse
presses (part 3's client settings), Auto-show floating preview, beside part 5's "Open links in".

Split from [20261005-browser-surface](closed/20261005-browser-surface.md) at its `prepare` (planned split, part 4). It starts
after part 1 merges into `feat(example)/t3-code`.

## Scope and exclusions

From the parent's scope (its numbering):

1. **The profile submenu.** "+" menu › Browser › the profile list and the launcher's "Open browser in a profile"
   chevron (shown only with more than one profile, `RightPanelTabs.tsx:495-560, 1246-1285`); the tab's profile badge
   (`PreviewView.tsx` leadingActions, shown only when it differs from the default).
3. **More menu, part 4's group.** "Profile: <name>" with Clear cookies and Clear cache (`PreviewMoreMenu.tsx:196-228`,
   `BrowserSession.ts` `clearCookies`/`clearCache`: cookies, local storage, IndexedDB, service workers; the cache),
   scoped to the tab's environment and profile.
10. **Profiles and cookie import.** Built-in Default (persistent) and Incognito (in memory, cleared when the app quits),
    up to 24 user profiles (`BROWSER_PROFILE_MAX_COUNT`, names up to 48 characters; `browserProfile.ts:19-60`), the
    default profile for new tabs; Settings › Integrations › Browser's profiles row ("Browser profiles", "Profiles
    separate cookies and logins. Incognito data is cleared when the app closes.", Add profile; clone:
    `settings-source-control.contract` BrowserDefaults); the cookie import wizard with its six unavailable reasons and
    copy (`browserImport.ts:31-60,142-154`, `BrowserImportWizard.tsx`): "Quit <browser> to import", "Let T3 Code read
    <browser>'s cookies" (Full Disk Access), "Import from <browser>", "Importing cookies", "Couldn't import from <browser>".
    WebKit: part 1 already gives each environment's profile its own persistent `WKWebsiteDataStore` (identifier derived
    from the environment and the profile, `T3BrowserSessions.storeIdentifier`); Incognito is a non-persistent store; an
    import writes through the store's `httpCookieStore` (confirmed at `prepare`).

Added by the coordinator (2026-10-09): **the Browser defaults group** in Settings › Integrations › Browser
(`IntegrationsSettings.tsx` BrowserViewportSetting, the zoom, appearance, recording and Auto-show rows;
`browserDefaults.ts`): the default viewport (Fill, Responsive with typed width/height and rotate, the presets), zoom and
appearance, applied to every new page through part 2's `browserSet` and to Show device toolbar; the recording frame
rate, key presses and mouse presses (part 3's `CLIENT_DEFAULTS` keys, byte-identical); Auto-show floating preview, read
by part 5's automation host; part 5's "Open links in" row reused. Each row has the reference's reset.

Excluded: parts 2, 3 and 5 (part 4 wires its defaults into their open paths: part 5's link opens and agent tabs, part
2's device toolbar).

## Context and guidance

Parent specification: [spec](../spec.md); engine decisions and declared differences: the parent record and
`EXACT2-GAPS.md` "Browser surface: declared differences (X1 path B)". Reference: T3 Code `1e2ecbd975`
(`packages/contracts/src/{browserProfile,browserImport}.ts`, `apps/desktop/src/preview/BrowserSession.ts`,
`apps/desktop/src/preview/BrowserImport/*`, `apps/web/src/browser/browserDefaults.ts`). Part 1's seams:
`browser-surface.ts` `BROWSER_PROFILES` (the list the menu and the badge read), `T3BrowserSessions.swift` `store(…)`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-browser-surface](closed/20261005-browser-surface.md) (part 1) | [#337](https://github.com/ccheever/exact2/pull/337) | Merged into `feat(example)/t3-code` | merged as `dce6d78df` (2026-10-09) |
| framework issue | [X66](../issues/20261008-x66-popover-from-action-and-toggle.md) | #319 | nonblocking: the profile submenu opens from its chevron, not on hover, until a popover can open from an action | — |
| task PR, reverted | `20261005-browser-surface-navigation` (part 2) | [#348](https://github.com/ccheever/exact2/pull/348), reverted by #351 | Part 2 re-lands on `feat(example)/t3-code`; this branch then merges the base again | merged here at `46436acc3` before the revert (`827470bbc`); the PR's diff shows part 2's files until part 2 is back |

## Acceptance and reproduction

Rows from the parent's table: "Open and tabs" (each profile), "More menu" (the Profile group), "Profiles", "Cookie
import" (fixture browser stores; no real browser data is read in the lane run). Tests to port (`bun:test`, original
names; counts from the parent): `packages/contracts` `browserProfile.test.ts` (9), `BrowserImport/*` (108; the reader
tests need the engine and fixtures: port the pure parts, run the rest as lane rows), `addBrowserSurface`'s profile cases
and `openPreviewSession`'s "does not open … with unread settings and uses the saved profile on retry". Standard gates
as the parent's.

### Results

Live rows ran in agent mode on fixture browser stores only (a fixture home under the worktree's `target/`, named by
`T3_BROWSER_IMPORT_HOME`; no real browser profile, Keychain item or Full Disk Access was read, asked for or granted).
The import and profile rows ran at `827470bbc` (drive branch-5); the defaults and tab rows at `5a7522e56` (drive
branch-6), which differs from `827470bbc` only by the Browser defaults rows' test ids.

| Row | Result | Evidence |
| --- | --- | --- |
| Settings › Integrations › Browser: Browser profiles and the defaults group, beside part 5's "Open links in" | passed (agent mode) | [before/after](https://raw.githubusercontent.com/ccheever/exact2/3db690395d870aa78fd83b65b6d02a759585c593/browser-surface-profiles/p4-integrations.png) |
| Add profile: menu (Blank profile, Import from the installed browsers), a blank profile, rename | passed | [image](https://raw.githubusercontent.com/ccheever/exact2/77353ef58e943050fa903a6acd9441015b1c1ed1/browser-surface-profiles/p4-add-profile.png) |
| A profile's menu (Set as default, Clear cookies and cache, Remove profile and data), the remove dialog, removed (its stores cleared in the environment) | passed | [image](https://raw.githubusercontent.com/ccheever/exact2/65a0ce31378e58a8fdffa1fd4f5739c7e5856352/browser-surface-profiles/p4-row-menu-remove.png) |
| Import from Chrome into a new profile: 3 imported, 2 skipped (a CHIPS cookie and a schema-24 cookie bound to another host) | passed | [image](https://raw.githubusercontent.com/ccheever/exact2/3275b4cd5925fd33e8fb273d863fbd74e0a07f6b/browser-surface-profiles/p4-import-chrome.png) |
| Brave running: "Quit Brave to import", then "I've quit it" with the browser gone opens the configure step | passed | [image](https://raw.githubusercontent.com/ccheever/exact2/8e4dba0259f56fe0f3baf0410bcaa83e970507da/browser-surface-profiles/p4-import-brave-quit.png) |
| Safari: "Let T3 Code read Safari's cookies" (the fixture's EPERM); Allow is only recorded outside the packaged build | passed | [image](https://raw.githubusercontent.com/ccheever/exact2/d634b5ab717b005a2a6b76175496071838a02a51/browser-surface-profiles/p4-import-safari-fda.png) |
| Arc: the Keychain refused, "Couldn't import from Arc" with Try again | passed | [image](https://raw.githubusercontent.com/ccheever/exact2/c5f3e2e5d72c292514515c275f40a98bb4ac41be/browser-surface-profiles/p4-import-arc-keychain.png) |
| Import from Firefox into the existing Default profile (the container cookie is not imported) | passed | [image](https://raw.githubusercontent.com/ccheever/exact2/3ebb2fae7b7714e3dfb7a34bb16a5b6b0b928c75/browser-surface-profiles/p4-import-firefox-default.png) |
| Default viewport, zoom, appearance, frame rate, key presses, Auto-show set and two reset | passed (agent mode; the viewport and zoom took effect on the tabs below) | [image](https://raw.githubusercontent.com/ccheever/exact2/884f66bd4d9a861c29f8f8e34d448cc8bc62a3c5/browser-surface-profiles/p4-defaults.png) |
| Launcher chevron with more than one profile; a tab in the Chrome profile has its imported cookies, the badge, and the configured defaults | passed: the probe received `lane_chrome, lane_session`; the page saw dark, devicePixelRatio 2.5 (125% at 2×), and the iPhone 12 Pro toolbar | [before/after](https://raw.githubusercontent.com/ccheever/exact2/4103b9b5688bfe58ed078b72820b7dafb5c6bfff/browser-surface-profiles/p4-launcher.png), [image](https://raw.githubusercontent.com/ccheever/exact2/fd506fb662f1f2bc32f1c195e5d15a4272758cfb/browser-surface-profiles/p4-chrome-tab.png) |
| More menu: "Profile: Chrome" with Clear cookies and Clear cache; Clear cookies empties that profile only | passed: after Clear cookies and a reload the probe received none | [before/after](https://raw.githubusercontent.com/ccheever/exact2/b40432d28d94e2a538f51f536d829529706a726c/browser-surface-profiles/p4-more-menu.png) |
| "+" › Browser › the profile list; Incognito's cookie stays in Incognito; a Default tab has Firefox's cookie only | passed: Incognito received `incognito_probe` after `/set`; the Default tab received `lane_firefox` only | [before/after](https://raw.githubusercontent.com/ccheever/exact2/71f809fb2bc6d235a7aa0ceb050deb2d816b4d13/browser-surface-profiles/p4-plus-profiles.png), [image](https://raw.githubusercontent.com/ccheever/exact2/238c4d6be66144d5319281cb306ae48a1f894650/browser-surface-profiles/p4-incognito-default.png) |
| A packaged build reading real browsers (Keychain prompt, Full Disk Access in System Settings) | not run: the brief forbids reading real browser data or granting Full Disk Access in a lane | — |
| Real-input rows ("Real-input batch steps" below) | not passed yet: deferred to the next real-input batch | — |

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Planned; starts after part 1 merges.

2026-10-09 (`prepare` and implementation, draft PR [#354](https://github.com/ccheever/exact2/pull/354)):

- **Profiles** (`browser-profiles.ts`): the reference's `browserProfile.ts` rules (Default and Incognito built in, up to 24
  named profiles of up to 48 characters, Incognito never the default), kept at the preference file's root
  (`browserProfiles`, `browserDefaultProfileId`). A tab opens only once the settings were read
  (`BrowserSettingsReadError`), at the configured profile and viewport.
- **Stores** (`T3BrowserSessions.swift`, `T3BrowserSessions+Profiles.swift`): Default and named profiles persist per
  environment (part 1's `storeIdentifier`); Incognito is `WKWebsiteDataStore.nonPersistent()`. Clear cookies and Clear
  cache (`browserClearData`) act on the tab's environment and profile; removing a profile clears its stores in every
  known environment.
- **Menus and badge**: the "+" menu's Browser submenu and the launcher's chevron list the profiles; the chrome row's
  badge shows a profile other than the configured default; the More menu's Profile group works. The menus take
  ↑/↓/Escape (KeyMenu).
- **Cookie import**: the reference's `BrowserImport/*` ported to the data module over the module's primitives
  (`browserImportIO`: SQLite, CommonCrypto/CryptoKit, Keychain, fcntl; `T3BrowserImportIO.swift`); writes through the
  profile's `WKHTTPCookieStore` with a read-back. Development and agent builds read only a fixture home
  (`T3_BROWSER_IMPORT_HOME`, confined); only the packaged build reads the user's browsers.
- **Settings**: Settings › Integrations › Browser's profiles row with its menus, the removal dialog and the import
  wizard (quit, Full Disk Access, configure, importing, done, blocked steps), and the defaults group.
- **Wired into parts 2 and 5** (after merging them): part 5's link opens (`openUrlInPreview`: chat, terminal and script
  links) and an agent's new tab open under the configured viewport and profile, and its host reads Auto-show; every page
  the module creates gets the default zoom and appearance through part 2's `browserSet` (on `browserSync`, whichever path
  opened it); Show device toolbar opens at a fixed configured viewport (`browserResponsiveViewportForToggle`).
- **Two fixes the live drives found**: the app runtime freezes `Error.prototype`, so `this.name = …` in an Error subclass
  throws there while Bun passes (the import's error classes now define `name`; a test runs the readers under a frozen
  prototype); a listing whose reads a newer command let go read every browser as running (the let-go answers are now
  discarded). Two test-id collisions (the wizard's dialog against its buttons, the defaults rows against their selects)
  made agent taps land on the container; both renamed.
- **Base merges**: `5f0ae7dca` (part 5, #346) and `46436acc3` (part 2, #348). Part 2 was reverted on the base by #351
  (`c603c22d6`); per the coordinator this branch keeps `46436acc3` and does not merge past it. Part 4 depends on part 2's
  re-land (its `browserSet` and `browser-viewport.ts`), and merges the base again once part 2 is back.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Unit, AppKit and the five checks | `827470bbc` | Bun 3,934 pass / 0 fail / 1 skip (part-4 files: profiles 29, import 30, readers 30, Safari 25, sources 43, defaults 6); strict tsc clean; contract build OK (5,966 slots); `cargo test -p t3-code-macos --lib` 13; AppKit `browser` 27, `browser-profiles` 16, `browser-automation` 20, `menus` 45; build, test (3,521 passed, 0 failed, 34 ignored), clippy, fmt, caps, boot: all exit 0 | [checks](https://raw.githubusercontent.com/ccheever/exact2/f0013459ca171ace31503b60a90fb8968d3af3a5/browser-surface-profiles/checks-827470bbc.txt) | — |
| Live drives 1–3 (agent mode) | work in progress (before `4e4f76f23`) | 1: the agent's `type` replaces a field's text, so the rename op was rewritten. 2 and 3: the Chrome wizard opened on "Quit Chrome": the readers threw `TypeError` in the app runtime (a frozen `Error.prototype`; Bun passes), and a let-go listing read every browser as running. Both fixed in `4e4f76f23` with tests | — | — |
| Live drive 4 (agent mode) | `c7a0aa02f` | Ops 1–102: add, rename, remove; Chrome import (3 imported, 2 skipped); Brave's quit step; Safari's Full Disk Access step; Arc blocked; Firefox into Default. Done and "I've quit it" missed their buttons: the dialog's test id was the button's (`browser-import-<step>`); fixed in `0af00d295` | [record](https://raw.githubusercontent.com/ccheever/exact2/2190ae19fcd99108474fc144dac23f7d69919621/browser-surface-profiles/drive-branch-4.txt) | — |
| Live drive 5 (agent mode) | `827470bbc` (parts 2 and 5 merged) | Ops 1–102 passed, including Done and "I've quit it" → configure; stopped at op 103: each defaults row's test id was its select's, so the select did not open; fixed in `5a7522e56` | [record](https://raw.githubusercontent.com/ccheever/exact2/596efb792211934bb2ce71b68b19cd8536a2a8cd/browser-surface-profiles/drive-branch-5.txt) | — |
| Live drive 6 (agent mode, final mode: the Chrome and Firefox imports, the defaults rows, the tabs) | `5a7522e56` | All 117 ops passed: Chrome import into a new profile and Firefox into Default; the defaults rows set and reset; the launcher chevron (Default, Incognito, Chrome); a Chrome tab at the configured iPhone 12 Pro, 125% and dark with its imported cookies (the probe names `lane_chrome, lane_session`); the More menu's Profile: Chrome group, Clear cookies then none; "+" › Browser › Incognito keeps `incognito_probe` to itself; a Default tab sees `lane_firefox` only | [record](https://raw.githubusercontent.com/ccheever/exact2/ab29fa36a2bbad9660d6040ce286e58e1fbe2c20/browser-surface-profiles/drive-branch-6.txt) | — |

## Real-input batch steps

The lane: `target/lane` in the profiles worktree (`drive.sh <run> final` is the agent-mode version; fixture home from
`make-browser-home.mjs`, probe page `fixture.mjs` on 16781, server on 16780; isolated HOME, CODEX_HOME,
CLAUDE_CONFIG_DIR, XDG_*, T3_LOCAL_HOME). A lane app copy needs its own name and bundle id; never the user's browsers. Copies: [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/ee83a8e71fc5ffbd2157318707b7818c8a8a9849/browser-surface-profiles/drive.sh.txt), [make-browser-home.mjs](https://raw.githubusercontent.com/ccheever/exact2/0f4b4d0ca3b5cc40792b8663566aac4a74b4fe3a/browser-surface-profiles/make-browser-home.mjs.txt), [fixture.mjs](https://raw.githubusercontent.com/ccheever/exact2/a634c7f48ebe1a77f70bac9604144a433c0a8137/browser-surface-profiles/fixture.mjs.txt).

1. **Stores across a relaunch** (needs a normal launch: agent runs keep every store in memory). Import Chrome into a new
   profile, open a Chrome tab at `127.0.0.1:16781/cookies` (names `lane_chrome`, `lane_session`), open an Incognito tab at
   `/set`. Quit with ⌘Q and launch again normally. Read back: the Chrome tab's probe still names `lane_chrome`; a new
   Incognito tab's probe names none.
2. **The profile menus by real keys.** Settings › Integrations › Browser: focus Add profile, press ↓: the menu opens at
   Blank profile; ↓ reaches the first browser under "Import from"; Escape closes it and the focus is back on Add
   profile. The same on a row's ⋮ menu and on the launcher's chevron.
3. **Real pointer.** Hover a tab's profile badge: the tooltip names the profile. "+" › Browser: the chevron opens the
   profile list (X66: by press, not on hover).
4. **Rename by real key events** (ASCII; the input source is left as it is): type in a row's name field and press
   Return; the "+" menu shows the new name.

## Next action

1. The real-input batch (steps 1–4 above; STATUS "Next real-input batch").
2. When part 2 re-lands (#352), merge the base and set the default zoom in liveSessions' zoom field; re-drive a
   default-viewport tab at 125% and check its first layout and zoom with the first-layout probe from #352's fixture.
   (#352's `T3BrowserSessions.ensure` makes a fixed-size page at its target size before `navigate()`, from the
   width × height × zoom that `liveSessions`/`browserSync` carry; a page the module has not reported yet gets zoom 1,
   so without part 4's default in that field a new tab is made at 100% and then re-zoomed by `browserSet`.) Then
   re-run the checks.
3. After the Settings › Integrations request-loop fix (`feat(example)/t3-code-fix-settings-integrations-loop`) lands:
   merge it, keeping both sides on the Integrations page.
