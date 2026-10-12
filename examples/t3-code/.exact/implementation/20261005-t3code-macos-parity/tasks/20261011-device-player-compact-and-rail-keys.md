---
name: 20261011-device-player-compact-and-rail-keys
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-device-player-compact-and-rail-keys
pr_url: https://github.com/ccheever/exact2/pull/464
verified_commit: null
---

# The details card's compact and essential rows, its popover beside the player, and the device rail menus' keys

## Outcome

The user decided on 2026-10-11 that every item below is built now, matching the reference T3 Code `1e2ecbd975`. They
are the remaining differences of [floating-device-player](closed/20261005-floating-device-player.md) ("the card has one
density"; "when no placement is left the inline card hides instead of becoming the popover") and of
[audit-wave-followups-4](closed/20261010-audit-wave-followups-4.md) ("Found, not changed": the device rail's menus draw
their rows beside their `KeyMenu`).

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| DP-1 | The thread details card has one density: its full content, scrolled when a floating player folds it (`thread-details-card-layout.ts` ports `resolveThreadDetailsCardDensity` with its tests, unused) | `ThreadDetailsCard` measures its content per density and folds it: `full` (everything), `compact` (no Run on, no workspace row, no Automations, no Lineage, Version Control unseparated, the Git action group hidden unless an action runs or just finished), `essential` (compact without Open in) |
| DP-2 | With the inline card open and the floating player resized into it so that no 160 pt placement is left, the card hides; the header button keeps toggling the hidden inline card | `ThreadDetailsCard` turns into the header's popover (`mode = placement ? "inline" : "popover"`); the header toggles the popover; an inline placement coming back closes it |
| DR-1 | The device rail's menus (Text size, Rotate, More) draw their rows beside their `KeyMenu`, so a key at a focused row (one the pointer rests on) reaches no menu | Base UI `Menu`: on a focused row ↓ / ↑ move the highlight and wrap, Home / End go to the ends, a letter to the next row starting with it, Enter / Space press it, Escape closes the menu and gives the focus back to the trigger |

## Steps

1. Reference first: the source below; CDP on the reference lane for the card's three densities and its popover beside a
   floated player, and the rail's menus by keys.
2. DP-1: the canvas source (`chat-canvas-view.ts`) measures the card's full and compact heights from `t3-frame` frames
   (the content, and the parts only `full` shows), keeps them per thread and presentation as the reference's
   `measurements`, and answers the density (`resolveThreadDetailsCardDensity`); the card (`shell-details.contract`)
   shows each density's rows; `R4GitRows` hides the Git action group when compact.
3. DP-2: the canvas answers whether the card is inline or a popover and counts the switches; the window's toggle opens
   the popover in popover mode (also when the player took the inline place), and a switch back to inline closes it.
4. DR-1: the rail menus' rows go inside their `KeyMenu` (`r6-device.contract`); the shared rule's test stops naming the
   rail as the exception.
5. Bun tests; one agent drive on a lane with a fixture device hub (no real devices); before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| DP-1 | reference comparison; Bun tests of the density and of the rows each density shows; agent drive: the player resized under the card at 1280×840 and a short window | before / after / reference images |
| DP-2 | reference comparison; Bun tests; agent drive: the player resized into the card until no placement is left, the header button opens the popover, the player shrunk back closes it | before / after / reference images |
| DR-1 | reference comparison; Bun tests of the shared rule; agent drive: a rail menu row focused by the pointer, ↓ / End / Escape | before / after text record and image |
| DP-1, DP-2, DR-1 under real pointer and keys | real-input steps for the coordinator's session | — |

## Reference

- `apps/web/src/components/chat/ThreadDetailsCard.tsx` (placement, mode, per-density measurement keyed by thread and
  preferred width, `cardBottom` from the full height), `threadDetailsCardLayout.ts`, `ThreadDetailsPanel.tsx` (the rows
  per density), `components/GitActionsControl.tsx` (`compact`: the action group only while an action runs or after
  it), `components/BranchToolbar.tsx` (panel sections), `components/ChatView.tsx` (`threadPanelPresentation`,
  `toggleThreadPanel`), `rightPanelStore.ts` (`ThreadPanelVisibility`: `inlineOpen`, `popoverOpen`).
- `apps/web/src/components/device/DeviceControlsRail.tsx` (three Base UI `Menu`s: Rotate on Android, Device text size,
  More device actions), `components/ui/menu.tsx` (Base UI Menu: `useListNavigation` with `loopFocus`, typeahead).

## Cause and fix

**DP-1.** The clone ported `resolveThreadDetailsCardDensity` but never measured the card per density nor folded its rows.
ThreadDetailsCard measures its content tree (`offsetHeight` plus the frame's borders) in the density it is showing,
keeps both heights per `thread:preferred width` ("popup" when no inline placement exists at all), and asks the density
of the height its placement leaves (`placement.height`, else the canvas height less 52 for the popover). The clone's
measurement is the module's `t3-frame` hook, which reports a frame only for mounted nodes and can be read before or after
a density switch lands, so the content frame alone cannot say which density it measured. The card now marks its layout:
the parts only `full` shows report their own frames (`details-full-top`: the Run on and workspace rows; `details-full-tail`:
Automations and Lineage; `details-full-git`: the Git action group at rest, a zero-width box inside it; `details-vcs`:
the Version Control section, whose 1 pt hairline only `full` draws), and a zero-size `details-density-compact` marker
mounts with the compact rows. `measureThreadDetailsCard` (`thread-details-card-layout.ts`) takes a full layout's height
and its compact height (the full one without those parts, the same number the reference measures after laying the
compact rows out), a compact layout's height, and nothing from essential rows or a layout caught between densities.
The canvas source (`chat-canvas-view.ts`) keeps the heights per key, answers `density`, and reports the open docked card
to the canvas as tall as its full height once that is known (the reference's `cardBottom`; before, the current content).
`DetailsBody` (`shell-details.contract`) shows ThreadDetailsPanel's rows per density, `R4GitRows` (`r4-git.contract`)
hides the action group when compact unless an action runs or just finished (GitActionsControl `compact`), and the
Open in and script menus hang under the rows each density shows.

**DP-2.** The clone hid the inline card when no 160 pt placement was left and kept toggling the hidden docked card from
the header. The canvas now answers ThreadDetailsCard's mode (`placement ? inline : popover`; a narrow chat is the
popover too) and counts its switches; the window opens the popover in popover mode (`detailsPopover`), shows it only in
the stint it opened in (`detailsEpoch`, as the reference's inline mode clears `popoverOpen`), presses the header button
for the presentation it toggles, and an open popover reads the details on a wide chat too (`shell-details.ts`). The
reference's popover sits below the floating player; here `z-index` orders siblings only (LLP 1001 "`z-index` stays inside
its holder"), and the popover was the window's, so it covered the player and its backdrop took the player's presses. The
popover now lies in the chat column before the player (`app-main.contract` ChatColumn); its backdrop covers the column, a
gesture on the player closes it (`canvasGesture`), and a primary press beside the column closes it too (`app-window.contract`
`outsidePressDown`, as Base UI's outside press), except on the right panel's own details button, which toggles it. Over a
right panel sheet (a narrow window) it stays the window's, above the sheet, as before. The popover's Git and Run on menus
take the popover's inner width (`inline` is now the presentation's, not the docked card's state).

**DR-1.** `KeyMenu` hears the keys of what it holds; the rail's three menus drew their rows beside it (fix-keyboard-focus
put `KeyMenu` and the popup column at one depth), so a key at a row the pointer had focused reached no menu. The popup
column now holds `KeyMenu`, which holds the rows (`r6-device.contract`). `menu-keys.test.ts` no longer names the rail as
the one menu with its rows beside its `KeyMenu`.

**Review round 2 (2026-10-12).** An independent review found five problems; each is fixed:

1. *A press on the floating player left the popover open.* Only a drag on the player (`canvasGesture`) closed it; Base UI
   closes on any press outside the popup. The players (`R6DeviceMiniPlayer`, `BrowserMiniPlayer`) now hand their
   `pointerdown` and `pointerup` to the window's `detailsPlayerPointer` (`app-window.contract`, through ChatColumn's
   `detailsOutside`), which arms on the primary button's down and dismisses the popover as the press ends, so a pill
   button's own press still lands (a dismissal at the down removed the popover between the button's down and up, and the
   agent drive lost the pill's press). On macOS a press is held by the innermost node that hears pointer events, so the
   window's `main` never hears a press on the player; the players hear it themselves, and `detailsPlayerPointer` also
   closes a skill chip's details, as `outsidePressDown` does for a press anywhere else (`usage-pooled.test.ts` lists the
   device player among the nodes that take the pointer, beside the browser player).
2. *The pull request rows placed their hover cards for the docked card while the popover showed on a wide chat.* The
   details source now reads the popover shown (`shellDetails(detailsShown, …)`), and `prRowsView` takes
   `inline && !open` (`shell-details.ts`): 246 pt rows 13 pt in, as on a narrow chat. That closed a cycle (the canvas
   read `details.inline`), and the runner refused every header toggle (`press view 115 (detailsAct#1) refused: Cycle`, a
   diagnostic drive of `73805d7d3`): the canvas now takes the docked card's possibility from the window (`conversationWidth
   >= 984 and data.projectId != ""`) and reads the card's own open state as before (`chat-canvas-view.ts` `inlineOpen`).
3. *Acceptance checks not run.* DP-1's short window and DR-1's Rotate and Device text size menus are driven now (see the
   results). The lane's fixture hub lists a fixture Pixel beside the fixture iPhone, and `ANDROID_HOME` points both apps'
   servers at a lane-local fake Android SDK whose `adb` answers the Pixel's settings reads (`font_scale` 1.0, `uimode
   night` no) and refuses everything else, so Rotate (Android only) and Device text size (enabled by a settings read)
   are live on both sides without a simulator, an emulator or a device. The window is shortened below both apps' 620 pt
   minimum by the agent's window resize and the reference's CDP viewport.
4. *The `panel-toggle-details` exception was unreachable.* That button is drawn only on a sheet, where the popover is the
   window's and its backdrop takes every press. The exception and the two ids are gone; the window's rule stands aside
   over a sheet (`detailsSheet`, now a derive of T3Window).
5. *A popover taller than the chat column ran under an open terminal drawer.* ChatColumn now has `z-index` 1, so it paints
   over the drawer (z-index orders siblings, LLP 1001) and the popover runs over the drawer as the reference's portaled
   popover does, at the reference's `100dvh − 6.5rem`; its backdrop reaches below the column to the window's foot
   (`ThreadDetailsPanel` `below`), so a press on the drawer closes it. The reviewer's alternative, bounding the height by
   the column, would have left the popover short of the reference's.

**Found in round 2 (fixed).** The Device panel listed its devices only after Refresh devices; DevicePanel calls
`device.list` each time it shows, the state loaded and the hub enabled. `deviceView` takes the panel's shown state and
sends one list per stint, not awaited (the answer comes on the state stream); `deviceHidden` ends the stint
(`r4-surfaces-device.ts`, `r4-surfaces-panel.ts`).

**Found while driving (fixed).** "Float device over chat" left the right panel open on its launcher: the window holds the
panel it opened (`rightPanelAt`) until an op lets go, and a floated Browser tab let go but a floated device did not. The
device's float now lets go too (DevicePanel `floatActive`: `rightPanelStore.close`).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| DP-1 | pass (agent drive, reference): the player resized from its top edge to 557 pt leaves the card 196 pt: before the full rows scroll in a 195 pt card; after and the reference show the compact rows (Open in, Add project script, branch, Changes) in a 166 pt card. To 589 pt (163 pt left): before still the full rows; after and the reference the essential rows (Add project script, branch, Changes), 134 pt. The player frames and card heights are the reference's to the point | [compact](https://raw.githubusercontent.com/ccheever/exact2/dbfce89f5bf1d8f4270521a2c6e81372d88f9495/device-player-compact-and-rail-keys/dp1-compact.png), [essential](https://raw.githubusercontent.com/ccheever/exact2/2ca6626548a23e0d7bf4f5b5fd4e69dab6d044e9/device-player-compact-and-rail-keys/dp1-essential.png), [record](https://raw.githubusercontent.com/ccheever/exact2/62103a55a9fb989806e31ac3382572844ad65d2d/device-player-compact-and-rail-keys/dp-record.txt) |
| DP-2 | pass (agent drive, reference): to 626 pt (126 pt left) the docked card leaves and the header button reads closed; pressed, before it closed the hidden docked card (it stayed gone after the player shrank); after and the reference open the popover with the full rows, under the player. The player dragged smaller from its top edge closes the popover and the docked card returns full, the button pressed, the player at the reference's frame (979, 308, 240 × 520). With the right panel open (narrow chat) the popover opens and closes from the header, and a press in the right panel closes it | [popover](https://raw.githubusercontent.com/ccheever/exact2/94f96de50b35237bee2370d5bcdaa4ee5b66c4b2/device-player-compact-and-rail-keys/dp2-popover.png), [back](https://raw.githubusercontent.com/ccheever/exact2/2ebd480de0d710acc53eb2973d70091caea5cedf/device-player-compact-and-rail-keys/dp2-back-inline.png), [record](https://raw.githubusercontent.com/ccheever/exact2/62103a55a9fb989806e31ac3382572844ad65d2d/device-player-compact-and-rail-keys/dp-record.txt) |
| DR-1 | pass (agent pointer and keys, reference by CDP): More device actions, the pointer on Close device panel, then ↓, ↓, End, Home, P, Escape: before the focus stays on Close device panel for every key; after it goes Power off device, Float device over chat (wraps), Power off device, Float device over chat, Power off device, as the reference's highlight, and Escape closes the menu with the focus on its trigger | [image](https://raw.githubusercontent.com/ccheever/exact2/6454ad840ad339a268267f051d428dea4ebadf52/device-player-compact-and-rail-keys/dr1-keys.png), [record](https://raw.githubusercontent.com/ccheever/exact2/761a80761afaace1254bbe8f299c1c0bc548a284/device-player-compact-and-rail-keys/dr1-record.txt) |
| Float closes the panel (found) | pass (agent drive, reference): before the right panel stays open on "Open a surface" beside the floated device; after and the reference close it | [image](https://raw.githubusercontent.com/ccheever/exact2/72644b4a39d26d9e1d702f6fba981690bf439e8d/device-player-compact-and-rail-keys/float-closes-panel.png) |
| DP-1, a short window (round 2) | pass (agent drive, reference by CDP viewport): no player, the window 1280×300: before the full rows scroll in a 224 pt card; after and the reference fold to the compact rows (Open in, Add project script, branch, Changes) in 166 pt. 1280×238: before the full rows in 162 pt; after and the reference the essential rows in 134 pt | [compact](https://raw.githubusercontent.com/ccheever/exact2/6b468fe91a13248a0605f676762400be88dc3230/device-player-compact-and-rail-keys/r2-dp1-short-compact.png), [essential](https://raw.githubusercontent.com/ccheever/exact2/23ddf7795e7973301318255a2561d35e2a1e4b25/device-player-compact-and-rail-keys/r2-dp1-short-essential.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f8436561622087405102e388feacbc3910dc1211/device-player-compact-and-rail-keys/r2-record.txt) |
| DR-1, Rotate and Device text size (round 2) | pass (agent pointer and keys, reference by CDP; the fixture Pixel): Rotate, the pointer on Portrait, then ↓ ↓ End Home L Escape: before the focus stays on Portrait; after Landscape, Portrait (wraps), Landscape, Portrait, Landscape, as the reference's highlight, and Escape gives the focus to Rotate device. Device text size, the pointer on Default, then ↓ End Home ↑ D Escape: before it stays on Default; after Large, Extra large, Small, Extra large (wraps), Default, as the reference | [rotate](https://raw.githubusercontent.com/ccheever/exact2/c16721122b4a9f39b2964c2864f6823f8a277bd1/device-player-compact-and-rail-keys/r2-dr1-rotate.png), [text size](https://raw.githubusercontent.com/ccheever/exact2/1485d0d44038b7e30e09faff99f4031efde89778/device-player-compact-and-rail-keys/r2-dr1-text.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f8436561622087405102e388feacbc3910dc1211/device-player-compact-and-rail-keys/r2-record.txt) |
| Review 1: a press on the player closes the popover | pass (agent drive, reference): 1200×840, the Pixel floated, the popover open under the player; Open preview in right panel on the pill: after and the reference the popover closes and the device returns to the right panel (runner log: `detailsPlayerPointer` down and up, the popover's 112 nodes leave, then the pill's press). Before, the window's popover covered the player and the press closed it through its backdrop; the player stayed floated | [open](https://raw.githubusercontent.com/ccheever/exact2/434467d44b7af2616474fbf3deec0fcd7ffa4515/device-player-compact-and-rail-keys/r2-f1-open.png), [pressed](https://raw.githubusercontent.com/ccheever/exact2/9cd04d1d35fed23806d8522e2899427bb36b5fbf/device-player-compact-and-rail-keys/r2-f1-pressed.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f8436561622087405102e388feacbc3910dc1211/device-player-compact-and-rail-keys/r2-record.txt) |
| Review 2: the pull request rows in the popover on a wide chat | pass (Bun test; no pull request in the lane): with the popover shown on a wide chat the row's checks tail is 84.3 and its card's max width 254 (the popover's 246 pt rows), as on a narrow chat; before 83.3 and 270 (the docked card's) | [record](https://raw.githubusercontent.com/ccheever/exact2/f8436561622087405102e388feacbc3910dc1211/device-player-compact-and-rail-keys/r2-record.txt) |
| Review 4: the unreachable exception | pass (Bun test): `outsidePressDown` names no `panel-toggle-details`, the two ids are gone, and the rule stands aside over a sheet | — |
| Review 5: the popover over the terminal drawer | pass (agent drive, reference): 1200×620, the right panel open, the drawer dragged to its 465 pt maximum (top 155; the reference's 156): after and the reference the popover's essential rows (134 pt from 60) run over the drawer. Before, the window's popover with the full rows | [image](https://raw.githubusercontent.com/ccheever/exact2/2ef9f883696026105834e0fdd61daaecd1fcdc53/device-player-compact-and-rail-keys/r2-f5-drawer.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f8436561622087405102e388feacbc3910dc1211/device-player-compact-and-rail-keys/r2-record.txt) |
| The popover over a right panel sheet (round 2) | pass (agent drive, after only: the sheet's path is the base's but for the window's rule standing aside): 960×840, the right panel a sheet: the sheet's details button opens the window's popover above the sheet (683, 60, 264 × 231); a press where the sidebar's toggle is lands on its window-wide backdrop and closes it; reopened, a press on the sheet's button closes it | [image](https://raw.githubusercontent.com/ccheever/exact2/55718f881e483b0d82746bff8e637dd2ece97e84/device-player-compact-and-rail-keys/r2-sheet-after.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f8436561622087405102e388feacbc3910dc1211/device-player-compact-and-rail-keys/r2-record.txt) |
| Found: the Device panel lists as it opens | pass (agent drive, Bun test, reference): before "No simulators or emulators were found on this environment." until Refresh devices; after and the reference the hub's devices | [image](https://raw.githubusercontent.com/ccheever/exact2/fea0a404409231783aedb4899f8078d74d1c4253/device-player-compact-and-rail-keys/r2-device-list.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f8436561622087405102e388feacbc3910dc1211/device-player-compact-and-rail-keys/r2-record.txt) |
| DP-1, DP-2, DR-1 under real pointer and keys | open: see "Real-input batch steps" | — |

Round 2 drives (agent mode; the clone's window resized by the agent, the reference's viewport by CDP): before the
evidence worktree at `6db5cadea`, after this branch's bundle of `365b38cd5` (merged with `origin/feat(example)/t3-code`
`e91bdc8af`), the same lane (base port 16460) with the fixture hub's iPhone and Pixel and the fake Android SDK (`adb`
calls logged; none reached a device). The after drive ran four times: on `b7d74067d` the header toggle was refused (the
cycle above), on `f10850194` likewise, on `94f84c289` the popover opened but the pill's press was lost when the popover
left at the press's down, on `365b38cd5` every step passed (the images). The terminal drawer's web view draws black in
the agent's window capture, and its prompt is pixelated in the image.

Round 1 drives (agent mode, 1280×840): lane `device-player-compact-and-rail-keys` (base port 16460: reference backend 16460, CDP
16461, clone server 16462), the Verification fixture's "Timeline verification" thread. Devices come from a fixture device
hub: a lane-local stand-in for `expo-device-hub@0.12.0` installed in each lane T3 home
(`tools/expo-device-hub/0.12.0`, its `.install-complete` sentinel; source `target/lane/fixture-hub/cli.mjs` in this worktree,
not committed) that answers `/readyz`, `/api/devices` (one "Fixture iPhone (lane fixture)", booted) and the boot and
serve-sim grid start with `ok`, logs every request beside the install, and touches no simulator, emulator or device; the
stream requests get 404, so both apps show the device's loading or Reconnect tile. Settings `enableDeviceSupport` and
`deviceOnboardingCompleted` are on in the lane homes. On the clone lanes `providers.codex.binaryPath` points at a missing
file, so no Codex update toast races the drive's taps (both sides of the clone comparison; the reference keeps its CLI).
Before: `t3-code-evidence-base` at `6db5cadea`; after: this branch's bundle of `74b583799` (the last commit, `d6e29bcde`,
only keeps the popover the window's over a right panel sheet, unchanged from the base there; not re-driven). The after
drive ran three times: the first stopped at its tenth op (a toast had not yet appeared), the second completed on the bundle
before the popover moved under the player and showed the popover over the player, the third (the images) on `74b583799`.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout) with
`T3_LOCAL_HOME=<lane>/clone-t3-home` and `T3_LOCAL_PORT=<lane port>`; before launching, copy the fixture hub into the
lane home (`<lane>/clone-t3-home/tools/expo-device-hub/0.12.0/node_modules/expo-device-hub/dist/server/cli.mjs` from
`t3-code-device-player-compact-and-rail-keys/target/lane/fixture-hub/cli.mjs`, a `package.json` there, and `0.12.0` in
`<lane>/clone-t3-home/tools/expo-device-hub/0.12.0/.install-complete`) and set `enableDeviceSupport` and
`deviceOnboardingCompleted` to `true` in `<lane>/clone-t3-home/userdata/settings.json`. For the Pixel's Rotate and Device
text size menus, launch with `ANDROID_HOME` and `ANDROID_SDK_ROOT` set to
`t3-code-device-player-compact-and-rail-keys/target/lane/fake-android-sdk` (its `adb` answers the Pixel's settings reads
only and logs every call to `target/lane/r2/adb-calls.log`). Window 1280×840, a thread open
(Verification fixture › Timeline verification). Never send a message; never open a real device.

1. **DR-1.** Right panel › Device: the fixture iPhone and the fixture Pixel are listed without Refresh devices. Open
   Fixture iPhone (lane fixture). Click More device actions (⋯), rest
   the pointer on Close device panel, press ↓: Power off device is highlighted; ↓: Float device over chat; End: Power off
   device; Home: Float device over chat; P: Power off device; Esc: the menu closes, the ⋯ button has the focus. Open
   Fixture Pixel (lane fixture): in Rotate device rest the pointer on Portrait, ↓: Landscape; ↓: Portrait; End:
   Landscape; Home: Portrait; L: Landscape; Esc. In Device text size rest the pointer on Default, ↓: Large; End: Extra
   large; Home: Small; ↑: Extra large; D: Default; Esc.
2. **DP-1.** Close the right panel with its toggle (the device floats over the chat). Drag the player's top edge up about
   37 pt: the card shows Open in Cursor, Add project script, main, Changes (no project row, no Commit row); about 32 pt
   more: Add project script, main, Changes.
3. **DP-2.** About 37 pt more: the card leaves. Click the header's thread details button: the popover opens under the
   player, full rows. Drag the player's top edge down about 150 pt: the popover closes, the docked card returns, the
   button pressed. Open the popover again (drag up), click the sidebar: it closes.
4. **Sheet.** Make the window narrower than 980 pt with the right panel open (a sheet), click the sheet's thread details
   button: the popover opens above the sheet; click outside it: it closes.
5. **Float.** Open the right panel on the device, More device actions › Float device over chat: the panel closes.
6. **Review round 2.** Window 1200×840, the device floated: open the popover from the header, move onto the player's
   handle so its pill shows, click Open preview in right panel: the popover closes and the device returns to the panel.
   Make the window 1200×620, open the terminal drawer from the right panel's header and drag its top edge up as far as it
   goes, open the popover: its rows run over the drawer; click the drawer: the popover closes.

## Tests

- `device-player-compact-and-rail-keys.test.ts` (10): DP-1, the measurement of a full and a compact layout and nothing
  from essential or a switch in flight; the density sequence; the canvas folding the card to compact and essential as the
  player grows and restoring it as it shrinks, with the full height kept; the popover's density from the canvas less
  52 pt under its own key. DP-2, the canvas's mode and its switches (a narrow chat, the player's 160 pt, the docked card
  closed); the window's toggle, epoch, shown state and header state; the popover in the chat column before both players
  (z 35 under z 40), the outside presses and the right panel's details button; the sheet's window-level popover; the
  menus' widths. The rows of each density in `DetailsBody` and `R4GitRows`. DR-1, each rail menu's rows inside its
  `KeyMenu`. The Float fix's rule.
- Updated: `chat-canvas-view.test.ts` (the args, the full layout's marker frame, the narrow window's two mode switches),
  `menu-keys.test.ts` (no menu draws its rows beside its `KeyMenu`), `r3-shell-details.test.ts` (an open popover reads
  the details on a wide chat).
- Round 2: `device-player-compact-and-rail-keys.test.ts` (12): the outside press (the window's x range, the players'
  `detailsPlayerPointer` armed at the down and dismissing at the up, no `panel-toggle-details`, the rule aside over a
  sheet), ChatColumn's `z-index` 1 and the backdrop's `below`, the pull request rows placed for the popover shown on a
  wide chat (a `shellDetails` run with a pull request: tail 84.3 and card width 254, as narrow; before 83.3 and 270), and
  the canvas reading nothing of `details` (the cycle). `r4-surfaces.test.ts`: the Device panel lists once per stint when
  it shows with the hub enabled, again after hide and show, never with the hub off.

## Checks

On `d6e29bcde` (merged with `origin/feat(example)/t3-code`, already up to date), once:

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4663 pass, 1 skip, 0 fail (312 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 29,284,849 bytes; `app.contract` 1,406 lines |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,679 passed, 0 failed, 34 ignored |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust and no Swift changed |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | three builds: `566e10e76`, `c7ca0a8ec`, `74b583799` (the driven bundle) |

## Seen while building, not a finding row

- With the hub just started, the clone's Device panel read "No simulators or emulators were found" until Refresh devices; the reference lists on mount (DevicePanel's `device.list` effect). Fixed in round 2.
- (Round 2: the pill press and the pull request rows' placement are fixed; the Device panel's listing is fixed.)
- With the right panel open at 1200×620 the popover's left edge is 383 in the clone and 388 in the reference: each ends
  at the chat header's thread details button, which sits 5 pt further left in the clone's header when the right panel
  is open. That is the header's control row (not this task's rows); left for the coordinator, with no pixel-perfect loop.
- The plan grew by 0.46 MB (28,820,298 → 29,284,849 bytes): the popover is drawn in two places (the chat column and,
  over a sheet, the window), each in two scheme branches.

## Delivery

Draft pull request [#464](https://github.com/ccheever/exact2/pull/464) into `feat(example)/t3-code`.

## Next action

The coordinator reviews draft PR [#464](https://github.com/ccheever/exact2/pull/464), runs the real-input steps in the next batch, and merges it.
