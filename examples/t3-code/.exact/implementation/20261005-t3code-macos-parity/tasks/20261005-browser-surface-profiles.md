---
name: 20261005-browser-surface-profiles
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Browser surface part 4: profiles, cookie import and clearing

## Outcome

Browser tabs open under the reference's profiles: the built-in Default and Incognito and up to 24 named ones, each with
its own storage; the "+" menu's profile submenu (part 1 lists Default only) and the launcher's chevron list them; a
tab in another profile than the default shows its badge in the chrome row; the More menu's "Profile: <name>" group
clears cookies and cache for that profile only (part 1 draws Clear cookies and Clear cache disabled, marked "Part 4");
Settings › Integrations › Browser manages the profiles and the default one; the cookie import wizard brings cookies
from Chrome, Edge, Brave, Vivaldi, Opera, Arc, Helium, Firefox and Safari.

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

Excluded: parts 2, 3 and 5.

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

## Acceptance and reproduction

Rows from the parent's table: "Open and tabs" (each profile), "More menu" (the Profile group), "Profiles", "Cookie
import" (fixture browser stores; no real browser data is read in the lane run). Tests to port (`bun:test`, original
names; counts from the parent): `packages/contracts` `browserProfile.test.ts` (9), `BrowserImport/*` (108; the reader
tests need the engine and fixtures: port the pure parts, run the rest as lane rows), `addBrowserSurface`'s profile cases
and `openPreviewSession`'s "does not open … with unread settings and uses the saved profile on retry". Standard gates
as the parent's.

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Planned; starts after part 1 merges.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |

## Next action

After part 1 merges: `prepare` (the cookie write path per store, Full Disk Access copy on macOS), then implement.
