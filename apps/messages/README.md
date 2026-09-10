# Messages

An in-progress, local-only iPhone chat example in Exact2. The view is
`app.contract`; `app.ts` provides the baked conversation fixtures and in-memory
message edits. It requests no network or storage capabilities. Relaunch resets
all messages.

Run from the Exact2 root:

```sh
node host/apple/build.mjs --ios messages-apple --run
node host/web/dev.mjs --app messages
```

Drive the installed simulator app:

```sh
node scripts/agent.mjs --app messages ios 'tap conversation-maya' \
  'type composer See you soon!' 'tap send' state
```

`--sim <UDID>` on the build and `EXACT_SIM=<UDID>` on the driver select a
simulator. Native TypeScript execution needs the matching lean Hermes archives
described in LLP 1027's iOS execution section. On this Mac those archives are
provisioned under `target/hermes-ios`; simulator archives must be thin archives
for the build architecture, not universal Mach-O containers. The build now
invalidates a previously cached iOS engine-less stub when archives are provisioned.

Current behavior: searchable fixture conversations, native iOS controller navigation,
native multiline text editing with a growing composer, per-conversation drafts,
local sending, a dedicated recipient/compose sheet, focused inline reply threads, anchored Tapbacks with a scrollable reaction palette,
message deletion, a contact page, and an emoji attachment tray. Safe areas and
the keyboard-resizing viewport use the normal Exact2 host. The module test
exercises real baked Hermes bytecode and verifies conversation isolation.

The inbox starts with a large, left-aligned title and a Compose button beside bottom search,
following [Apple’s iOS 26 guide](https://support.apple.com/guide/iphone/send-and-reply-to-messages-iph82fb73ba3/ios).
On the iPhone 17 / iOS 26.5 capture, the large title's measured ink bounds match
the native reference: x=18–175.67 and y=128–157.67 points. Scrolling fades it into
a centered compact title over the list. UIKit now permits vertical elastic
scrolling even when the inbox fits; held drags, reversal and release work with
both six fixture threads and a single search result. Browser wheel input at a
short viewport exercises the same title state and restores it at the top.
Navigation preserves the title/search geometry and conversation draft. This is
not yet the complete native title transition: the recorded native list travels
farther for the same drag, and its soft header edge differs from the current
uniform material. The initial large-title placement is verified independently
of those remaining motion/compositing differences.
A standalone UIKit comparison isolated that travel difference: an 80-point
drag moved a standard large-title table about 62 points while its top inset
collapsed from 168 to 116; a bare scroll view moved 36.67 points, the same as
Exact, regardless of whether its content size filled the viewport. Both returned
to their initial position on release. This points to navigation-bar ownership,
not a different generic scroll resistance. A separate public scroll-edge fixture
produced the soft edge with a native label; a custom-painted text view did not.
Those mechanisms are not yet integrated into the app's header.
An isolated prototype then mounted the real Messages list under a visible UIKit
navigation bar, removed the authored title from a paired development generation,
and associated the list with `setContentScrollView(_:for:)`. Explicitly restoring
the list to its adjusted top inset kept its rows reachable and preserved an
opened conversation's draft through Back. A true cold launch still produced a
compact bar: adjusted top inset 116 and first row y=116, rather than the reference
large-title position y=168. The prototype stopped after three integration rounds
and is not in the app. Its wheel calls did not move the list: the existing iOS
driver clamps against content size minus bounds and ignores adjusted insets.
Those calls establish neither title collapse nor finger-gesture behavior. A
future bounded attempt needs to resolve the controller/scroll initialization and
native inset ownership, then verify actual drag, reversal, release and return.
A left swipe on an inbox row reveals purple Hide Alerts and red Delete circles,
measured against the installed iOS 26.5 Messages. Mute/Unmute closes that row
and keeps its history; the muted marker appears beside its title. Deleting a
fixture thread removes it from the inbox, clears its saved draft and cancels its
pending reply. Its contact remains available, and a new send starts fresh history.
Browser touch input and actual Simulator drags/taps exercise these actions,
vertical scrolling through the nested horizontal rows, and fresh composition.
The inbox uses 45-point avatars and an 86⅔-point row pitch. Its first row starts
at y=168, correcting the earlier eight-point excess below the large title. A
fixed 20⅓-point heading row prevents the taller text chevron from pushing the
preview down or squeezing a two-line preview. The first sender's ink starts at
y=184⅔, matching the captured reference; the first preview starts within one
captured pixel in that capture. The kernel now preserves fractional frames,
including the row pitch, nested offsets and subpixel updates. A shared geometry
fixture previously lost a quarter-point inset and turned a half-point edit into
a whole-point move; its iPhone result now matches the browser within the driver's
two-decimal reporting precision. Apple text measurement now also preserves explicitly authored fractional
line-box heights, including empty, multiline and clamped text. Intrinsic widths,
normal line heights and painted baselines still round independently. A native-label/CoreText/WebKit comparison found no universal
baseline adjustment, so exact glyph placement remains a separate target.
The fractional line-height repair passes the Swift host regressions and an isolated
iPhone action-dispatch flow covering two-line previews, stable rows, local send,
compose cancellation, retained drafts, reactions and timestamp centers. Caltrain’s
full iOS smoke also passes with its reference image unchanged. Physical
gesture revalidation for this repair remains pending: the shared Simulator was
in concurrent use during the attempted sweep.
The controls use authored icon paths. A revealed row has the reference's gray
rounded background. Swiping another row closes the previous one; resetting a
row clears its highlight. Tapping a row while actions are revealed dismisses
them first; search and new composition also close them. Browser touch and physical Simulator gestures verify
switching rows, acting on them and vertical scrolling. The app observes the
standard `scroll` event; the hosts retain ownership of the gesture and offsets.
A right swipe now reveals a 50-point blue Read/Unread control in a 70-point
leading area, measured against native Messages (including its #0088ff color).
It toggles only that conversation's unread state, keeping drafts, history,
Tapbacks and pending replies. Opening the conversation marks it read again.
The browser touch drive covers both action sides, switching rows, cancellation,
read/unread, draft retention, search, mute and delete. Scroll tracking now lives
in the view; the data module no longer stores per-row scroll-reset revisions.
The earlier physical leading-action investigation stopped after three rounds:
agent activation worked while desktop-pointer taps appeared to miss. A later
reaction investigation reproduced a browser covering the Simulator, establishing
that a Simulator screenshot alone does not prove input reached it.

**Read/Unread physical revalidation (2026-09-10).** With the named Simulator
raised and verified foreground before every input, actual right drags and taps
now pass both Mark as Read and Mark as Unread. Each closes the row to its
70-point resting offset without opening the conversation. A held drag reversed
before release leaves unread state unchanged; switching to another row closes
the first; the group row's Read action works. Mark as Unread preserves the draft,
all other inbox entries and the full message history; opening the conversation
marks it read and restores that draft. A fresh ordinary launch without agent
flags also passes both directions: the unread dot disappears and returns while
the row closes. No runtime change was needed for these results. This closes the
currently reproducible leading-action failure; it does not establish where each
older failed input landed. Captures, state snapshots and source hashes are under
`/tmp/messages-leading-foreground/verification.json`.
At that stage, full-swipe commit remained unfinished; the explicit native binding
below now handles leading full swipes. Recently Deleted/recovery, symbol details
and exact reveal/close animation remain unfinished.
`scrollLeft` closes an acted-on row through the normal post-layout property path;
a browser/iPhone fixture verifies both axes, clamping after content growth,
independent resets and unrelated updates preserving manual scrolling.

**Full-swipe surface comparison (2026-09-10).** A foreground-guarded native
Messages drag from x=110 through displacements 60, 130, 210 and 260 points
keeps Read/Unread pending while held and commits on release. Reversing through
130 back to zero before release cancels. Both full-swipe directions of the
read/unread toggle were observed, restoring the reference conversation's read
state afterward. The first attempted capture entered drag-to-pin instead:
checking the window between touch-down and the first move delayed movement.
The corrected helper posts touch-down and the initial drag in one process,
starting movement after 16 ms. The drag-to-pin capture is excluded from the
swipe evidence.

A standalone public `UITableViewController` with `UIContextualAction` and
[`performsFirstActionWithFullSwipe`](https://developer.apple.com/documentation/uikit/uiswipeactionsconfiguration/performsfirstactionwithfullswipe)
reproduces the action's capsule widths at those held points: 87, 165 and 200
points for the 130, 210 and 260-point drags, matching the native reference.
Its roughly 50-point height differs by at most one captured pixel in those
samples. Callback records show zero actions after reversal and exactly one
per full release; a state snapshot at the last held point still has the
previous action count. This verifies the standard surface and its action
boundary, not complete pixel parity: the fixture uses default cell content,
and its initial unread symbol name did not resolve, leaving an empty capsule.

At the time of that study, Exact, driven through the same path in an ordinary
launch, kept a 50-point circle and left the row open without toggling on release.
The subsequent integration target was a native row presentation using UIKit's swipe
actions while preserving the kernel's row dimensions and logical node identity.
A callback must invoke the existing authored action once; reversal must not
invoke it; vertical list scrolling, row switching, draft retention, accessible
actions and the browser's controls must remain usable. No native row projection
landed from that study alone; the subsequent binding is recorded below. Captures, the temporary fixture and callback
snapshots are indexed in `/tmp/messages-full-swipe/verification.json`.

**Native row integration experiment (2026-09-10).** A private iOS build now
hosts the real Exact inbox buttons inside single-row UIKit tables. The existing
parent scroll view retains vertical scrolling; the kernel still supplies the
row width and height. Before each batch, row views return to their logical
parents for the ordinary update, then remount in their cells. The native action
captures the authored control's icon and invokes that same control's live id;
it does not call the data module directly or infer a read/unread mutation.
Physical full swipes pass Read and Unread, reversal cancels, and the original
row press still opens its conversation. Draft and complete history survive
Back and another full swipe. Row switching closes the previous cell's actions;
the first tap on an exposed row dismisses its actions without opening it.
An overflowing inbox with the search keyboard open scrolls vertically by 70
points. A short inbox moves while held and returns to zero; the first test's
assertion that it should retain a positive offset was incorrect, and the saved
held frame plus the overflowing case establish that the parent receives input.
An ordinary launch also completes full-swipe Read. Its resting inbox crop
(y=168–795 points) differs from the preceding ordinary production capture by
0.000029 mean RGB levels.

This is an experiment, not a supported host binding: it is gated by
`EXACT_NATIVE_SWIPE_PROTOTYPE=1` in the private package, temporarily locates the
fixture by test ids, and implements only the leading action. None of that
lookup code is in the repository. A production boundary must explicitly bind
content and action identities, handle trailing actions and removal during a
native transition, expose accessible action names and native action inspection,
and retain the authored fallback on other hosts. The private source, build,
physical checks and captures are in `/tmp/messages-native-row-integration/`.

The explicit-binding candidate was stopped on 2026-09-10 at its bounded repair
limit and removed from the working tree. Its compiler roundtrip and browser
fallback passed; its final iOS build installed. On the pinned Simulator, a real
right drag exposed the native Read button, and inspection found its 50×50-point
frame at (10, 186), its accessible name, and its registered primary activation
event. Both agent activation and a foreground-guarded physical tap then left
Maya unread. The cause is not established; this does not invalidate the earlier
body-only prototype's recorded full-swipe result. The candidate also moved the
whole authored wrapper subtree to preserve ancestry, so its callback/containment
path needs an isolated explanation before another production attempt. Trailing
actions and deletion were not reached. An initial run used a different booted
Simulator than the physical input and screenshot; its assertion is excluded.
The final pinned runs, source recovery patch and browser evidence are preserved
in `/tmp/messages-native-swipe-binding/`. At that stop, production retained its
authored swipe strip. After restoration, a rebuilt
iOS app passed physical Read/Unread with state assertions, then an ordinary
launch repeated the cycle: the unread dot disappeared and returned, and each
action closed the row. That recovery preceded the following isolated diagnosis.

**Explicit native swipe binding (2026-09-10).** A private callback trace found
why the candidate rejected the action: UIKit sets its `UITableViewCell` to
`isUserInteractionEnabled=false` during the native callback, and Exact's guard
incorrectly treated that projected cell as an authored input restriction. The
host now captures each action's original ancestors before projection and tests
those, preserving hidden/disabled/inert restrictions and the original scroll
visibility. Reset releases the projections. The ordinary source uses explicit
`swipeContent`, `swipeLeading`, `swipeTrailing` ids and `swipeDestructive`;
there is no test-id lookup or diagnostic tracing in the host.

The production build passes physical full Read and Unread, reversal without a
state change, partial action taps, native Mute/Unmute, and Delete while UIKit
completes the action. Driver activation of a revealed native action also passes
and reports `host-activation`; inspection reports its actual frames and accessible
name. Original row presses, drafts and full history survive navigation and
reparenting. Switching rows closes the prior actions; the first body tap closes
an exposed row without opening it. A short inbox bounces back; with the search
keyboard open, the overflowing parent scroll advances 67.33 points. The browser
keeps the authored controls and passes Read/Unread, Mute, draft retention and
Delete. Evidence is in `/tmp/messages-swipe-activation/`.

This closes the missing native leading full-swipe behavior, not the whole inbox
parity gap. Trailing full-swipe confirmation, Recently Deleted/recovery, exact
symbol artwork and all native transition timings still need comparison. The
initial binding captured the first-child icon with untransformed bounds; the
scale correction is recorded below. Input tests still use a temporary
foreground-guarded Simulator pointer helper. A separate fixture verifies that
hidden actions, disabled actions and actions under a disabled authored ancestor
stay unavailable, then become physically activatable when the restriction clears.
An ordinary launch also completes full Read/Unread in light and dark appearance;
the dot is absent after Read and present after Unread, with the row closed.
That fixture uses reported screen coordinates: viewport coordinates alone missed
its safe-area offset. Its first counter also conflated body presses with actions;
the final fixture separates them. The three sampled full-Read capsule widths are
within 0.67 points of the saved native Read captures at the same displacements;
this is sampled shape evidence, not proof of the entire transition curve.
All five repository checks pass for this binding; the workspace test run took
552.97 seconds and ran asynchronously. The combined checks exceeded the shared
blocking time budget on this run; the timing is recorded, not claimed compliant.

**Native swipe glyph sizing (2026-09-10).** The host now includes the first
child’s own scale/rotation when rasterizing its icon for UIKit. A bare layer
render omitted that transform: Mute’s white ink was 23.67×25.33 points and
Delete’s 22×26, although both were authored at scale 0.75. The corrected sampled
ink is 17.67×18.33 and 16×18.67 points respectively. Native’s sampled shapes are
16.67×18 and 16.67×19.33; the size defect is repaired, but the authored paths
still differ from native symbols.

The app’s action backgrounds now match the pinned iOS 26.5 capture: Mute is
`#6155f5` in light appearance and `#6d7cff` in dark; Delete is `#ff383c` and
`#ff4245`. Freshly revealed actions use the correct palette. No native reference
conversation was deleted or sent a message during this comparison.

An already-open action still retains its earlier appearance. Two bounded
adaptive-capture attempts made its image and background switch correctly but
corrupted unrelated avatar lettering or repainting in saved screenshots. Both
were removed at the repair limit; neither live UIKit trait overrides nor a
render-only palette override remains in production. Passing action state tests
was insufficient evidence for those attempts. The verified transform correction
and app colors remain; captures and the rejected sources are preserved in
`/tmp/messages-swipe-glyphs/` for an isolated rendering diagnosis.
After removal, the unaffected inbox rows match the previous verified capture
exactly (zero mean RGB difference). A separate native fixture measures 24×12
points for a plain rectangle, 18×9 with scale 0.75, and 9×18 with that scale and
90-degree rotation; every action activates. The rebuilt browser fallback and
all five repository checks pass; the workspace test run took 195.66 seconds.

New Message opens a dedicated sheet with a To field, close control, fixture
recipient results and a separate draft. Selecting a recipient enables the
composer; adding or removing recipients keeps that draft. The To field remains
editable and wraps selected names. Results exclude already-selected people.
Sending to one person reuses their conversation; multiple people create a local
group on the first nonblank send. Selecting the same members in a different order
reuses that group, including the Weekend fixture. Sending clears its unread state
and starts the offline reply sequence with an attributed group sender.
Cancel leaves inbox search and existing conversation drafts alone; a new compose
session starts empty. MobileSMS on iOS 26.5 immediately dismissed an unsent
one-character draft without a confirmation and reopened empty. Browser and
physical Simulator close taps verify cancellation with an empty sheet, recipient
query, one recipient and multiple recipients: no thread is created, an existing
conversation draft survives, and reopening clears the recipient query, selection
and new draft. The iPhone drive also verifies keyboard dismissal and closed inbox
rows after each cancellation. The Apple presenters now preserve scroll positions
while the inbox is hidden; previously closing compose revealed every row’s leading
action. Browser/iOS/macOS fixtures verify the underlying hide/restore behavior.
New rows created by sending from the sheet also start with their swipe actions
closed. The iOS presenter now defers their scroll assignments with the source
route's geometry; applying the offset against a new row's zero-width content
had discarded it. The browser/iOS inbox drive checks the actual 70-point
offset and saved pixels after sending, receiving, deleting and reopening.
The empty sheet was compared with MobileSMS on the iPhone 17 simulator running
iOS 26.5. Physical taps also verify open/close, recipient choice, a keyboard keystroke, sending, and
retained composer focus after the conversation opens.
Baked-module and browser/iOS drives cover duplicate exclusion, five-recipient
wrapping, removal without draft loss, cancellation without creating a thread,
whitespace-only send rejection, group reuse and preservation of one-to-one history.
Physical Simulator taps also add, remove and re-add recipients, retain a real
keyboard keystroke, and send to a new group with the keyboard still open. Group
titles truncate within the header; initial avatars remain placeholders.
Tapping a recipient now selects it; Backspace removes the selection. At an
empty To cursor, Backspace selects the last recipient before a second press
removes it. Escape or focusing the message clears the selection. Ordinary query
text still deletes normally, and removing every recipient keeps the draft.
Browser, iOS agent and actual Simulator keyboard taps verify these paths.
The native reference also selects recipient text before keyboard deletion.
The UIKit field now forwards software-keyboard Backspace, including on empty
text. A separate browser/physical-iPhone fixture verifies one key event before
normal deletion, emoji deletion, empty-field events and ancestor delivery. That
comparison also fixed the browser driver selecting all text before a key press;
key presses now preserve the existing cursor/selection.
Token editing remains approximate: selection handles, caret movement between
names, replacing a selected recipient by typing, and selection while a query is
present remain unfinished. Native contact picker and sheet presentation/dismissal
motion still need implementation/comparison.
Phone numbers and email addresses can now be entered directly, as in
[Apple’s documented compose flow](https://support.apple.com/en-ie/guide/iphone/iph82fb73ba3/ios).
Return or focusing the composer commits a recognized address or exact contact name.
Direct Send includes a valid typed address with the selected recipients; unresolved
text keeps Send unavailable and remains editable with the draft intact. Common US
number spellings and email case differences reuse the same local recipient, and
selection/cancellation creates no conversation. Baked-module tests and browser/physical
Simulator drives cover these paths, including software-keyboard Return and keeping
an unfinished recipient from being silently omitted. New address avatars use an
authored person silhouette; its exact native geometry remains a comparison item.
These addresses use the example’s
offline iMessage behavior, without an account or carrier lookup. Exact invalid-address
tokens/errors, international formatting and service-dependent colors remain unfinished;
MobileSMS’s malformed recipient labels on this simulator limit that visual comparison.
The recipient field declares `autocapitalize="none" autocorrect="off" spellcheck="false"`,
so names start in lowercase without automatic correction or spelling checks.
Disabling spelling checking removes UIKit’s empty 27-point prediction strip:
the built app’s recipient keyboard is now 308 points high, with its top edge at
screen y=566 matching native Messages on this iPhone 17/iOS 26.5 simulator.
The setting uses HTML’s spelling-check hint and logical-ancestor inheritance;
keyboard height remains platform behavior. Physical software-keyboard touches
verify lowercase entry before and after sheet drag/reversal, and dismissal returns
to the full-height inbox. The forwarding/navigation drive also verifies the
308-point overlap, retained source draft, blocked source Back, and subsequent Back
cancellation/completion. Choosing a recipient restores normal sentence entry in the
composer. Browser and physical iPhone drives cover selection/cancellation/sending
and live input hints; host/kernel tests cover spelling overrides, ancestor changes,
reparenting and clearing to defaults. Existing history appears after sending, not
during recipient selection.

Message bubbles open Tapbacks through UIKit long-press and double-tap recognizers;
the web uses `contextmenu` and `dblclick`. A single bubble tap is inert and retains
composer focus. Physical Simulator long-press and double-tap drives verify both
recognizers in the main conversation and focused replies; the agent's
`tap bubble-m1 contextmenu` / `tap bubble-m1 dblclick` forms inject the recognized
iOS event and do not themselves prove finger recognition. The corresponding
browser forms use actual secondary/double mouse input.

The Tapback preview aligns with its source bubble and the complete panel stays
inside the safe viewport above the keyboard. The rest of the conversation is
dimmed; the selected balloon and its receipt stop painting while their preview
is visible, retaining layout and identity. Tapping outside dismisses the panel.
Scrolling the suggested emoji and
selecting a later choice works on iOS and web. Double-tap shows only reactions
and retains the keyboard, including outside dismissal. Long-press adds Reply
and More below the preview and focuses the action menu, hiding the
keyboard. Dismissal and reaction selection restore composer focus if
it was active before the long-press; a closed keyboard stays closed. Both paths
preserve the draft, and selecting the same reaction again removes it. Browser
and physical Simulator drives verify these distinctions and focused-reply
behavior. The keyboard split follows captures of the local simulated Messages
fixture, including keyboard restoration after a long-press reaction. Normal-mode
Simulator captures also show double-tap keyboard retention and long-press
dismissal restoring an unsent draft and keyboard. One normal-mode sequence
missed outside dismissal at the far-right point (390, 170), although that point
passed in the agent drive; a left-side tap (30, 170) dismissed both presentations.
The cause of that miss is not isolated, so outside hit coverage remains open.

The palette and action card use the existing iOS glass material. The strip is
64 points high with 49-point choice spacing and 11/16-point outer insets,
measured from the native fixture. The preview keeps the transcript’s original
16-point inset and group indentation. The two dimming layers match the sampled
native light-mode background over white: RGB 229/229/229 for double-tap and
206/206/210 for long-press. This is a sampled color match, not full compositing
parity. The heart choice uses a pink emoji while retaining its saved ❤️ value;
it still differs from the native artwork. Browser and physical iPhone checks
cover incoming, outgoing, and group preview geometry, scrolling to later emoji,
and focus restoration. Normal-mode captures cover light and dark materials.

This remains a custom approximation: the remaining native actions (including
Translate and Select), reaction-participant presentation, stickers,
exact reaction artwork and compositing, haptics, final preview pixel rounding/clipping,
and presentation animation remain unfinished. Native double-tap also dims the
system keyboard; this app’s dimming layer ends above it. The dark palette has
been checked for rendering and interaction, but not matched to a native capture.

The native emoji-picker study (`/tmp/messages-emoji-picker/`) establishes a
public platform input path, now integrated through `emojiPicker=true`. A public `UITextField`
overriding [UIKit's `textInputMode`](https://developer.apple.com/documentation/uikit/uiresponder/textinputmode)
to the active emoji mode opens the system grid with Search Emoji. On the same
iPhone 17 / iOS 26.5 Simulator its keyboard frame is (0, 486, 402, 388),
matching the native picker's vertical extent. Physical input searched `coffee`,
selected `☕️`, reopened the picker and cancelled it; the selection callback
received only the emoji, not the search text, and the separate draft remained
unchanged. The public field still shows ABC and dictation controls that the
native reaction grid omits. A second variant using only `UIView + UIKeyInput`
loses search (keyboard top 539, height 335) and retains those extra controls;
it is not the replacement. UIKit documents that an overridden input mode still
allows the user to change keyboards.

Native captures also establish the presentation's state transitions. Opening
the attached smile button replaces the strip and action card with the keyboard,
raises the selected balloon, and exposes a larger smile control with Close.
Close returns to the reaction strip **without** the action card, while keeping
the conversation dimmed; it does not dismiss the entire presentation. Searching
and selecting coffee instead closes the presentation and adds the reaction.
`native-close-outcome.png` records the actual Close action; the earlier
`native-picker-cancelled.png` does not, because backgrounding Messages had
already dismissed the picker.

The integration (`/tmp/messages-emoji-integration/`, 2026-09-09) separates entry
focus/dimming from the visible strip/actions/picker state. The small smile
control sits 6 points beside the enlarged balloon, on its inner edge, with its
center 10 points below the preview top. Zero-height side slots preserve the
balloon's width basis; the host projects their horizontal position alongside
the preview. Opening replaces the strip/actions with the native keyboard and
larger smile control plus Close. The app retains its draft independently.

Browser and iOS driver runs verify outgoing/incoming placement, invalid-text
refusal, skin-tone/ZWJ selection, Close and Escape, prior-focus restoration,
focused replies with a flag, and a group message with a keycap. Physical
Simulator touches separately verify long-press opening, the small smile hit,
software-keyboard entry of `coffee`, selection of ☕️, incoming double-tap
opening/Close, and outgoing long-press opening/Close. Search text never became
a reaction or changed the draft. Hardware keystrokes did not populate the
search in this integrated drive; the successful search used software key taps.
Double-entry Close restores QWERTY while retaining the strip; long-entry Close
retains the dimmed strip with keyboard and action card closed. The native
keyboard frame is (0, 486, 402, 388) on this fixture.

Remaining: joined thought-bubble/glass artwork and its dots, native picker
animation, exact keyboard controls, stickers, native incoming large-control
geometry (currently mirrored from outgoing), and final participant-popover
placement/artwork. Per-person ownership and grouping are implemented below. The custom circles are functional approximations. Web/macOS
filter emoji character entry but do not automatically open an OS picker;
Linux explicitly reports this selection policy unsupported. The single-grapheme
filter is bounded and is not a complete Unicode emoji-sequence validator.

An ordinary launch separately verifies picker selection, draft/QWERTY
restoration, a subsequent physical standard reaction and continued software
keyboard typing (`ordinary-*.png`). A second ordinary launch types `dog` into
Search Emoji through software key taps: the results change to dogs, paws and a
bone; selecting the dog-face result adds 🐶 and closes the keyboard when the
composer was not previously editing (`ordinary-search-dog.png`,
`ordinary-dog-selected.png`). This proves filtering independently of the
frequently used coffee result. The longer reply-to-reaction driver sequence subsequently exposed a separate
input-carrier defect, now repaired (`/tmp/messages-focus-driver/`). A minimal
case is sufficient: type a draft, double-tap a balloon, choose a standard
reaction. The app's focus slot and `layout <composer>` both showed the editor
losing focus. The driver walked beyond the pressed reaction button to the
menu's key handler and made that ancestor first responder; the native pressed
button consumes its touch instead. The driver now stops its responder walk
at the resolved press target. The existing Insets fixture reproduces the old
failure with a retained button beneath a key handler, and verifies the repair
using UIKit's actual first-responder state. Explicit focus-taking controls,
editor taps and blank-ground blur also pass. The original longer sequence now
passes in full, including draft/focus restoration and all receipt/preview cases.
This driver repair passed its first candidate; the earlier picker implementation
used three product fix rounds.
The five workspace checks pass after the driver repair (build 13.0 s, tests
166.7 s with all failures collected, clippy 18.5 s, formatting 1.8 s,
caps/boot 0.1 s each). The targeted Insets smoke passes on iOS and web.
`/tmp/messages-focus-driver/verification.json` records the before/after
first-responder observations, the longer Messages sequence, checks and matching
installed/source build identities. The picker integration's earlier captures
and macOS build remain in `/tmp/messages-emoji-integration/`.

A further offline native comparison establishes that reactions belong to
people, not message direction. The settled captures in
`/tmp/messages-reaction-ownership/` cover an outgoing own reaction, an incoming
other reaction, own-plus-other reactions with different and identical emoji,
a long-press with two participants, and removal of only the local reaction.
Native renders mine blue and the other person's gray; identical emoji produce
two separate badges. The participant popover groups identical values under one
symbol with overlapping avatars. Removing mine preserves the other person's.
Measured badge boxes are 36 points, centered about 3 points inside the balloon's
inner edge and 10 points above its top, with 27-point spacing. The popover uses
98-point group spacing, 32-point avatars overlapping by 14 points, and
124/214-point widths for one/two groups.

An app candidate implemented that per-person model, grouped participants and
zero-width sibling badge slots in both transcripts. Browser and iOS driver
checks passed anchors, unchanged balloon dimensions, addition/replacement/removal,
emoji selection, group messages, focused-thread reopening and draft/focus.
An iOS verifier failure caused by targeting the menu with Escape was isolated:
that input explicitly focuses the menu. Dismissal through an outside physical
touch passed the same sequence. The browser receipt/preview regression also
passed against the candidate.

The candidate was initially reverted after physical badge misses despite
passing activation checks. The follow-up in `/tmp/messages-touch-phases/`
changed the evidence: a reduced overflowing-badge fixture receives complete
UIKit touch sequences; the full candidate does too. A reproduced miss generated
no application- or window-level touch event, and a simultaneous desktop capture
shows a browser covering the Simulator. The coordinate helper was clicking that
browser, while the Simulator-only screenshot still showed Messages. Adding a
post-release delay did not fix those misses. Window placement alone is not proof
that the intended desktop surface receives the input.

**The unchanged candidate is restored.** The physical verifier raises the named
Simulator window, checks that Simulator is frontmost, reads its placement, and
then converts the app coordinates. The traced candidate and a clean build with
all tracing removed pass repeated outgoing/incoming/focused badge taps, another
person's badge, independent local add/remove, and group reactions. Draft and
UIKit first-responder state stay intact during badge use. Prior untraced misses
cannot all be attributed retrospectively, but they no longer establish a badge
layout defect. The clean proof is `clean-physical-results.json` and `clean.log`;
the local Message source is identical to the previously saved candidate. No
native hit-testing or touch-handling change was needed.

The implementation stores reactions per person, derives the two transcript
views from that map, and highlights only the current user's selection. The
fixture seeds Maya's heart on m7. Zero-width slots beside each balloon attach the
badges without changing its percentage width basis; swipe-to-reply remains on
the balloon. The participant popover groups identical emoji under one symbol
with overlapping avatars. The later placement repair below separates it from
the palette with the keyboard open. Flat blue, standard-reaction artwork, dots, glass,
transitions and badge travel during reply drags are not exact native matches.
The physical Close control on the focused thread blurs the composer; its native
reference policy still needs comparison. The passing badge-focus claim does not
cover that Close action.
The final repository build passes the same physical sequence, plus web/iOS
ownership and receipt/preview regressions. An existing module-test file now
checks that changing or removing the local Tapback preserves Maya's reaction in
both transcript views, including same-emoji participant grouping. That targeted
test passes. All five workspace checks pass (build 37.0 s, tests 314.2 s with all
failures collected, clippy 19.2 s, formatting 1.8 s, caps/boot 0.1 s each).
`/tmp/messages-touch-phases/verification.json` records the source and binary
identities and the valid/invalid diagnostic runs separately.
An ordinary launch also passes physical opening of Maya's existing reaction,
adding the local heart beside it, opening the shared-symbol/two-avatar popover,
and outside dismissal (`ordinary-*.png` in the same directory). The app is left
running with both badges visible.

**Participant/menu separation (2026-09-09).** Fresh, foreground-verified native
captures on Exact Date Headings (iPhone 17 / iOS 26.5) show the participant
popover at y=62, height 122, and a crowded palette at y=208: a 24-point gap.
Opening the emoji picker keeps the preview in place and replaces the palette
with the taller attached control. Native also moves the transcript behind the
preview downward in the crowded case; that part remains owed in Exact.
`/tmp/messages-panel-placement/native-{upper-keyboard,crowded-badge,crowded-picker}.png`
holds the reference, using the existing offline KB conversation and a local
heart on its “After ten minutes” message.

Messages now authors a separate menu region below the participant popover.
Apple and web context placement intersect the region with the safe viewport.
The ordinary palette reserves 146 points below the safe-area top (122 + 24);
the picker's 88-point placeholder reserves 122, keeping the preview at the
same lower bound when it replaces the 64-point palette. Messages without
reactions reserve no participant space. The region's blank area dismisses the
menu while retaining the editor; its controls keep their own actions.
The first candidate's no-reaction branch used a dynamic `"0px"`, which the
style bridge refused and poisoned the runner on a newly created conversation.
The second candidate uses the supported zero percentage. This was a style-value
failure, not evidence of a message-loop identity defect.
The corrected candidate passes foreground-verified physical badge, picker,
Close, reaction and outside-dismiss taps on iOS, including two participants
and focused replies. The crowded iOS palette is y=208 and the preview y=278;
the picker keeps the preview y=278. A 402×480 browser viewport exercises the
same lower bound with a zero safe-area inset (palette y=146, preview y=216).
Web/iOS regressions also pass receipt anchoring, action spacing with and without
receipts, unclamped and bottom-clamped previews, new conversations, groups,
focused replies, and draft/focus restoration. The ordinary iOS launch repeats
the crowded badge → picker → Close → outside-dismiss sequence with the keyboard
visible throughout. `native-ordinary-*.png` in the evidence directory are
**Exact** captures; `native-crowded-*.png` are MobileSMS. The Swift package's
21 host tests pass; the initial private-package test attempt lacked its copied
test directory and did not run tests. `verification.json` records the final
source/binary identities, gate results and known remaining gaps.
All five workspace checks pass for this repair: build 26.8 s, tests 401.0 s
with all failures collected, clippy 90.1 s, formatting 1.8 s, caps/boot 0.1 s
each. These full-suite timings exceed the blocking-loop budget and are recorded
as observations, not a claim that the speed budget is met.
The same reference also separates two preview modes that Exact currently
conflates. OCR bounds for “After ten minutes” are about 131.9 points wide in
the transcript, 133.2 after a badge tap, and 152.2 after long press
(`native-*.ocr.json` in the evidence directory). This is evidence that badge entry
keeps the original text scale while long press magnifies it, rather than
evidence for magnifying every entry. A follow-up on the outgoing JA counterpart
confirms it: text width 131.9 in the transcript and after badge entry, 152.2
after long press (`native-outgoing-{transcript,badge,long}.{png,ocr.json}`).
At that point Exact still magnified both entry modes. The native focused-thread
case remains unmeasured; the overlap repair preserved the prior shared scale
policy, which the next repair replaces.

**Preview entry modes (2026-09-09).** `contextMagnify` is a boolean host policy
beside `contextTarget`, declared once in the kernel schema. False keeps scale 1;
absence or true keeps the existing 15% enlargement, capped at 26 added points
of width. Both modes share source-edge alignment, participant-region clamping,
side-control placement and receipt positioning. It does not change authored
CSS transforms or kernel frames. Messages binds it to the original entry's
`selectedActions`, so opening/closing the emoji picker preserves that entry's
mode even while the action card is temporarily hidden.

`/tmp/messages-preview-modes/native-outgoing-double.png` confirms a real double
tap on the native message body uses the same unscaled presentation as badge
entry. Foreground-verified physical iOS badge taps, double taps, long presses,
picker/Close and outside-dismiss taps now pass for outgoing and incoming
messages. The outgoing source/preview widths are 242/242 for badge and double
tap, 242/268 for long press; the incoming widths are 111/111 and 111/127.65.
The focused-reply controls implement those modes too, but that is an Exact
behavior check, not a recovered native focused-thread comparison. The browser
passes the same mode, geometry and draft tests. Its initial run exposed a
missing DOM mapping for the new prop; the mapping and an existing host-policy
regression now cover explicit false and subsequent true. The compiler regression
round-trips the plan and switches the same preview false → true → false through
the distinct input handlers. A first test build needed the concrete runner
data-source type on its closure; the corrected test passes.
The web/iOS receipt and action-spacing regression also passes in both modes,
including new conversations, groups, bottom clamping and focused replies.
The final ordinary launch repeats badge, double-tap and long-press entry
(`native-ordinary-*.png` in this evidence directory are **Exact**, not
MobileSMS). OCR text widths for the same outgoing message are approximately
214.3 in the transcript, 215.6 after badge/double entry and 238.4 after long
press; the geometric checks above are the more precise scale measurements.
That sequence also exposes an anchoring gap: the ordinary transcript text is
at y≈254 before long press, but the enlarged preview text is at y≈575 after
the keyboard closes. Native's outgoing reference keeps its text around y≈298
across that keyboard change (`/tmp/messages-panel-placement/native-outgoing-*.png`).
This exposed source retention and surrounding-transcript displacement as the
next repair (below). A receipt staying aligned with Exact's newly laid-out
source is not proof of native entry anchoring. Transition animation, reaction
artwork and glass remain separate fidelity gaps.
All five workspace checks pass for the entry-mode repair: build 33.8 s, tests
494.3 s with all failures collected, clippy 78.5 s, formatting 1.8 s, caps/boot
0.1 s each. These are observed full-run times, not a claim that the blocking
speed budget is met. `/tmp/messages-preview-modes/verification.json` records
the final sources, installed binary, runtime evidence and remaining gaps.

**Preview source anchoring (2026-09-09).** iOS and web now capture the source
before the entry batch can change keyboard focus. A height change retains its
position; a width change recaptures it. The surrounding transcript receives the
same presentation displacement as the retained, clamped preview. A centered
reply scroller also retains its clip position, so translating its contents does
not cut off neighboring messages when the keyboard closes. Closing the menu
removes both projections; normal scroll layout and the draft return.

The native comparison is the existing outgoing MobileSMS reference in
`/tmp/messages-panel-placement/native-outgoing-*.png`, and the crowded incoming
reference there. The first iOS candidate retained the source but clipped a
neighbor in a multi-message focused thread; the second retains the scroll clip
as well. The browser's first candidate could be displaced by ResizeObserver's
later end-follow adjustment; projection now follows that adjustment too.
Physical iOS entry tests measure an unclamped incoming source at y=290.67 before
and after keyboard dismissal. A crowded outgoing source and its neighbor move
by 37.33 points for badge entry and 39.48 for long press, together with their
preview. Repeated badge, double-tap and long-press entry, clock updates, picker
round trips and dismissal retain the draft and restore the original transcript
offset. A multi-message focused thread retains both source and clip positions.
Browser viewport expansion/contraction passes the same presentation checks;
width changes recapture the anchor, and dismissal clears every projection.
The compact 402×480 browser run also passes crowded-menu and picker checks.
The first resize test incorrectly required an unchanged DOM scroll offset while
the viewport height changed; it now checks retained painted positions during
resize and the original offset after restoration. These focused-thread checks
verify Exact's behavior; native focused-thread parity is still unmeasured.
An ordinary iOS launch (without the agent clock) now keeps the incoming
"Perfect" text at y≈302 versus y≈304 before enlargement. The crowded outgoing
text moves from y≈254 to y≈291, instead of the previous y≈575 after keyboard
dismissal. Both dismissals restore three sampled transcript text rows within
0.2 points by OCR, with the keyboard visible again. Those `ordinary-*.png`
captures are Exact, not MobileSMS. The final iOS build and physical checks,
browser checks, and all five workspace checks pass: build 45.2 s, tests 316.1 s,
clippy 42.5 s, formatting 1.8 s, caps/boot 0.1 s each. These full-run timings do
not establish compliance with the blocking speed budget.
Artifacts and source hashes are in `/tmp/messages-context-anchor/verification.json`.
Preview animation, native
artwork, glass, final clipping/pixel rounding and the other gaps below remain.

**Participant popover shadow (2026-09-10).** The rounded glass surface now owns
an inner scrolling row instead of also serving as the content clip. This is an
app hierarchy change: the same material, radius, panel dimensions and group
geometry remain. It removes the sharp rectangular cutoff around the glass's
shadow. On the one-group panel, mean adjacent-pixel RGB steps along the sampled
left/top corner boundaries fall from 4.9/4.37 to 0.1/0.1; the existing native
crowded-badge reference measures 0.1/0.067 there. This checks the cutoff, not
overall glass or pixel parity. `corner-steps.json` in
`/tmp/messages-popover-shadow/` records the samples. Apple documents the effect's
composition constraints in [UIVisualEffectView](https://developer.apple.com/documentation/uikit/uivisualeffectview/);
no effect subclass or host clipping rule changed for this repair.

Physical iOS and browser checks pass badge, double-tap and long-press entry,
picker return, source anchoring and draft retention. The two-group panel remains
214×122 with 32-point avatars 98 points apart; light/dark changes, both owners'
badge entry and removing only the current user's reaction preserve those
invariants. The ordinary launch also shows the smooth shadow and retains the
keyboard through badge entry and dismissal. All five workspace checks pass:
build 16.3 s, tests 387.7 s, clippy 21.8 s, formatting 1.7 s, caps/boot 0.1 s
each. These are full-run observations, not blocking-budget compliance.
Build, runtime, pixel evidence and source hashes are under
`/tmp/messages-popover-shadow/verification.json`.

**Copy and action rows (2026-09-10).** Copy now writes the complete selected
message through `copyText(selectedBody)`, closes the menu and restores the
composer only when it was editing before entry. It does not change the draft,
send a message, or read the clipboard. UIKit and AppKit use their general
pasteboard; the web starts its Clipboard API write during input dispatch and
logs denied/unavailable writes. Linux explicitly reports unsupported.

The native reference's Copy action writes exactly `After ten minutes` and
restores its retained `Picker draft` and keyboard. Physical Exact Copy taps
pass for outgoing, incoming emoji, focused replies, twelve lines containing
`café 👩🏽‍💻` (beyond the preview's eight-line clamp), and entry without a focused
composer. Each check reads the destination clipboard after the tap and verifies
the entire string, menu dismissal, draft retention and prior focus state.
The browser passes the same cases; clipboard-read permission is granted only
after the first copy for inspection, not to trigger the write.

The menu keeps its 250-point width and now uses leading icon slots, 42-point
action rows and ten-point top/bottom padding, following the native capture.
Reply, Copy and More make its current height 146 points; Translate and Select
remain owed. A first 19-point label candidate was too large by pixel ink
measurement and was corrected to 17. More now has three dots in its circle;
the Copy document outlines and Reply glyph still need native-symbol fidelity.
The taller card passes the existing placement, receipt, picker and Reply/More
regressions. The final Copy label's dark ink begins at x=201 points and measures
38⅓ × 15⅓ points, matching the native capture; its vertical position follows the
different selected balloon and row order. This measures the label, not the icon
or the complete menu's fidelity.

An ordinary iOS launch, without agent flags, also passes a foreground-guarded
long press and Copy tap. The pasteboard contains exactly `I’m in. Meet you there
at 10?`; the keyboard returns, and the transcript crop from y=160 through 470
points is pixel-identical before entry and after Copy. All five workspace checks
pass: build 15.0 s, tests 267.9 s, clippy 21.1 s, formatting 1.8 s, caps/boot
0.1 s each. These full-run timings do not establish the blocking time budget.
All 21 Swift tests pass; AppKit's clipboard branch compiles but has not been
driven through a macOS Copy interaction. Browser success is exercised; the
permission-denial path is implemented but not yet driven. Source hashes, logs
and captures are indexed in `/tmp/messages-copy-action/verification.json`.

More enters message selection with the pressed message already checked. The
composer stays mounted with its draft while the selection toolbar replaces it;
incoming bubbles move 44 points, and outgoing bubbles retain their width and
right edge. Tapping a message or its circle toggles selection. Trash is disabled
at zero selections and opens the native reference’s lower-left glass confirmation
with “Delete Message” or “Delete N Messages”. Tapping outside cancels confirmation;
the top-right X exits selection. Both cancellation and confirmed deletion leave
the keyboard closed, as observed in the native selection capture. Bulk deletion
updates the inbox preview, including empty history. Browser and physical iPhone
drives cover preselection, toggles, disabled trash, confirmation/cancellation,
draft retention, empty deletion, and entry from groups and focused replies.
Selection circles center on the bubble row independently of sender labels,
reactions, and reply counts; both hosts verify the centers and circle taps.
A normal-mode iPhone capture also verifies physical More, a second selection,
and confirmed deletion of exactly those two bubbles.
Forward opens the shared New Message sheet in the current conversation, with
To focused and the selected bodies joined by newlines in transcript order.
The text stays editable before choosing a recipient; Send requires a valid
recipient and nonblank body. Cancel returns to the source conversation with
selection exited, the keyboard closed, and its draft/reply target intact.
Sending creates one local message with the edited text, without copying reaction
or reply metadata, and preserves the source draft when forwarding elsewhere.
Browser and physical iPhone drives cover single/multiple selection, keyboard
entry into To, editing before a recipient, cancellation, sending to another
fixture contact, group history isolation, focused replies, and a clean later
compose session. Baked-module tests cover ordering, duplicate IDs, and exclusion
of unrelated IDs. These behaviors follow captured native single/multiple forwards;
the recipient keyboard now matches the reference height, as documented above.
An ordinary iPhone run also forwards two messages through physical More/Forward
taps, software-keyboard recipient entry, recipient selection, and Send, retaining
the keyboard in the destination conversation. Shared-sheet regression drives
verify zero-selection Forward, recipient removal/re-addition with the draft
retained, and a new local group; the iPhone uses its software Backspace key.
The forwarding sheet now disables the underlying conversation Back control.
UIKit’s back recognizers honor that disabled state before starting, so edge and
header swipes cannot navigate the source route out from under the sheet. Its
header retains focus on release. Browser pointer drags and physical iPhone
swipes preserve the sheet, recipient focus, forwarded text, and source draft;
cancellation restores ordinary Back gestures and contact-details Back. Native
Messages captures show the same stationary sheet and retained keyboard. An
ordinary iPhone run verifies both swipe origins, a subsequent software-keyboard
keystroke, and cancellation returning to its original H draft with the keyboard
closed. Cancelling and completing Back then restores the full-height inbox
with that draft preserved. Removing the focused sheet had synchronously resized
inside an older frame batch, whose remaining frames overwrote the restored
height. The iOS presenter now defers that keyboard update until the batch ends,
retaining its animation duration and curve. An ordinary Simulator capture verifies
the restored inbox; the physical navigation drive also checks the painted search
bar returns to y=798, matching its initial position, with zero keyboard overlap.
The forwarding regression also passes single/multiple sends, editing before
recipient choice, and source draft/reply preservation. Ordinary Simulator drags
retain interactive keyboard tracking, reversal, and completed dismissal.
Compose is now a separate modal route. UIKit supplies its large-sheet
presentation, downward drag, reversal, and dismissal; the browser keeps the
source route visible and inert and handles Escape. An empty new draft permits
dismissal, while populated forwarding springs back with its text and keyboard
retained, matching the two native reference cases. The active viewport uses the
sheet's coordinates and keyboard guide. Its horizontal-Back freeze is excluded
from modal transitions: an earlier attempt incorrectly froze the empty sheet's
layout during dismissal and let the composer slide under the keyboard.
The ordinary iPhone drive now holds the empty sheet at y=252 with its composer
beside the keyboard, reverses it, and completes dismissal to the full-height inbox.
Repeating this physical sequence after the spelling-check fix retains the composer
beside the now 308-point keyboard; a subsequent keyboard touch still edits To. A
populated forward also resists a full downward drag, springs back with its body
intact, and accepts a subsequent software-keyboard touch in To.
An ordinary Simulator reload with the sheet open also returns to the full-height
inbox. Modal teardown defers resizing until the new runtime's first batch is mounted.
Host-owned dimming removes the duplicate corner seam: the 160×124-pixel upper-left
light-mode crop now matches the native capture exactly. The corresponding dark
crop still differs slightly at its edge (mean RGB error 0.17/255); its sheet
surface is the same #1c1c1e. The native host now resolves inherited text color
from the logical tree: inbox headings, names and compose titles follow the
root's light/dark pair without local color overrides. Kernel/host regressions
cover ancestor changes, reparenting and cleared overrides. The sheet now keeps its
source controller and views live in the presenting surface, so changing appearance
while it is open also updates the source background, text and native materials.
The source retains its pre-sheet geometry; deferred frame/content updates are
replayed before normal layout returns. Source interaction and accessibility are
suppressed until closing. Ordinary held-sheet captures verify light → dark → light
behind the sheet, with the keyboard/composer still in place. A populated sheet
still springs back and accepts recipient typing after the appearance changes.
Back and forwarding regressions pass; reloading the ordinary app with a live
source behind an empty sheet restores the full-height inbox without runtime errors.
An initially selected modal route is separate, unverified startup
coverage; the normal application starts at the inbox.
Complete presentation/close timing still needs comparison; these captures do
not prove every intermediate frame matches.
Exact selection transition/scroll anchoring, native symbols, and deletion
animation still need comparison.

The standard laugh reaction is now a separate `haha` value from the 😂 emoji,
matching the distinct choices in [Apple’s Tapback reference](https://support.apple.com/guide/iphone/react-with-tapbacks-iph018d3c336/26/ios/26).
It renders as a blue, two-line HA HA shape in the picker and reaction badge;
its glossy native artwork is still an approximation. Focused inline threads
now display reaction badges too. Their badges reopen the picker for changing or
removing a reaction, and both views read the same saved message value. Reaction
badges opt into `retainFocus`, so opening their picker preserves the current
editing session even before the context panel exists. Browser and iOS drives
verify distinct selection, replacement, removal, and shared badges; real Simulator
taps reopen badges in both thread views without losing the draft or keyboard.
Opening a badge while the keyboard is closed leaves it closed.
A bubble-bound badge wrapper was tried to improve incoming badge placement.
It preserved widths after switching from flex to block layout, but physical
iPhone badge taps regressed intermittently. Moving the reply gesture to the
wrapper also broke browser badge taps. The layout experiment was reverted at
the three-round limit. After reverting, browser and physical iPhone checks
again passed badge reopening, replacement/removal, and draft/keyboard retention.
The later sibling-slot candidate was initially reverted too, then restored
after the foreground-verified physical proof above. It supplies the inner-edge
anchor; movement with reply drags remains unfinished.

The composer uses CSS `field-sizing: content`, bounded at 138 points. Sending
shrinks the cleared composer and requests the bottom of the updated transcript.
The DOM `scrollTop` binding is applied after the batch's children/layout are
mounted, once per changed value. The transcript's explicit `scrollFollowEnd`
policy keeps the bottom visible through keyboard and composer resizing while
reading the latest message. After scrolling up, the browser uses CSS scroll
anchoring, and iOS retains a visible descendant’s position across the batch.
A shared fixture verifies that growing and shrinking content above the reader
leaves the visible message in place; it previously jumped by the changed height.
Physical Simulator taps also verify that adjustment and that the top stays at zero.
If the native anchor disappears, iOS now uses the next surviving visible
candidate. It keeps that choice through later updates until the reader scrolls;
only when no candidate survives does it clamp the old offset. The deletion
fixture exposed a 70-point iPhone jump that the browser avoided. Browser and
iPhone drives now cover multiple deletions, hidden rows, restoration, growth,
manual scrolling, and empty/top/bottom boundaries. The earlier direct-delete chat flow also preserved the next bubble, draft and
keyboard when deleting a partly clipped message. The current More/selection
flow hides the keyboard; its reading-anchor behavior remains to be compared.
An explicit send still requests the bottom. These paths have passed on iOS and web.

Sending also schedules a deterministic offline response. After three 300 ms
fixture ticks, the sender’s three-dot typing bubble appears; after fifteen ticks,
it becomes one incoming message. Another send replaces that conversation’s pending
response. Other conversations retain their own pending response, and an inline
response stays attached to its reply root. The fixture marks a received message
unread when its conversation is not open. No timer state changes while replies are
idle. Dots use authored scale/opacity transitions; UIKit and CSS execute them.
The group fixture includes the typing sender’s avatar, following [iOS 26’s group
typing behavior](https://www.apple.com/newsroom/2025/06/apple-elevates-the-iphone-experience-with-ios-26/).
Browser/iOS drives verify appearance, changing dot scale, incoming replacement,
draft retention, end following, stable scrolled reading position, conversation
isolation, and group/inline indicators. Real Simulator taps on the keyboard H key
and Send also show the typing bubble becoming a reply with the keyboard still open.
The indicator’s dimensions, pulse curve, and insertion/removal motion still need
direct comparison with the target Messages version; this is not a timing parity claim.

Leaving a conversation retains its draft and reply target in memory; sending
clears the saved draft. Sending inside a focused reply thread leaves that thread
open for another reply. Switching between conversations verifies draft isolation.

The conversation starts with the reference's two-line service banner: `iMessage`
and a small lock beside `Encrypted`. The date follows below it with a tighter
gap before the first bubble. On the fresh iOS 26.5 Simulator build, the banner
starts at y=172, the date at y=212.67, and the first short bubble at y=232.67;
the saved native reference's bubble starts at y=232.33 (one device pixel apart).
Browser touch input and Simulator pointer input preserve the banner and date
through held timestamp reveal and release; saved native crops are identical at
every held position. The new cold-baked app also preserves the banner in dark
appearance and retains an unsent draft through Back/reopen. The lock is an
authored path pending native symbol access; this verifies the added content and
spacing, not exact SF Symbol artwork or text-baseline parity.
An empty conversation shows neither service line, matching the empty native
Simulator conversation. Deleting the final message removes the banner; an
unsent draft survives Back/reopen without bringing it back, and sending restores
it. Browser and iOS drives verify that lifecycle.

Dragging the transcript left reveals each message's timestamp; releasing returns
it to its resting position. This has passed browser touch input and actual
Simulator dragging, including a vertical drag that still scrolls the conversation.
The browser executes CSS scroll snap, and UIKit adjusts the native scroll view's
deceleration destination. The timestamp container uses `scrollbar-width: none`
to hide the extra track while preserving scrolling and snap. Timestamps now center
vertically on the bubble itself, independently of sender labels, reactions,
quoted text, delivery status, and reply counts. Browser layout measurements retain
the original bubble wrapping and positions; browser and iOS drives check timestamp
centers for one-to-one and group messages. Browser touch and physical iPhone input
also retain reaction-badge taps and swipe-to-reply after this layout change.

The iOS 26.5 Simulator's local Kate Bell/John Appleseed simulation supplies a native
reference: short and three-line bubbles center their timestamps vertically.
Smooth native drags of 40/80/120/180/240 points moved the bubbles approximately
14/31/46/58/58 points. The example now uses 40% of the scroll distance, capped at
58 points, and compensates date/service labels so they stay centered. Browser
and physical iPhone captures verify the cap, the stationary labels, and return;
the iPhone's corresponding partial offsets were 11/28/44 points, so recognition
thresholds still differ slightly. A fixed scroll extent keeps the compensation
from growing the scroll range. Timestamps are right-aligned in the revealed strip
and hidden at rest. Opening/leaving a conversation resets the gesture state.

Frame-by-frame video exposed delayed native scroll callbacks causing up to
14-point return jitter. User scrolling now dispatches before painting, while
layout-generated changes still coalesce after their batch. Mandatory snapping
uses UIKit's fast deceleration rate. The remaining sub-point variation in the
recording is within the video/pixel measurement precision. The recorded return
from a held 240-point drag settled within half a point in about 0.68 seconds,
versus 0.58 seconds in Messages; the example returns faster in its early frames.
This improves the previous roughly two-second return but is not an exact curve
match.

A fresh isolated iPhone 17 / iOS 26.5 Simulator recording after the fractional
kernel-frame and explicit line-height repairs still shows the mismatch. Saved
pixels give 12/28/44/58/58-point travel for 40/80/120/180/240-point drags;
the service/date-label crops are unchanged at every held position and after
release. Using the last frame within one point of the held position as the
time origin for both recordings, Exact returns 33.67 points by 0.10 seconds
and 48.67 by 0.20; the saved native reference returns 18.67 and 38 points.
Exact reaches within half a point of rest at about 0.73 seconds, versus
0.60 in that reference. This is an agent launch with real Simulator pointer
events and UIKit scroll timing, not an ordinary-launch timing verification.
The first attempted capture opened Tapback and was rejected; an isolated
helper with explicit mouse movement deltas produced the verified drag.
Its checks require visible bubble travel, stationary labels, no Tapback
selection and complete return, rather than treating input dispatch as proof.
The temporary recording and per-frame measurements are under
`/tmp/messages-timestamp-current/` (`verified-evidence.json`). Fractional
layout alone does not fix the release curve; that remains open.
A separate UIKit-only probe reproduced the long tail using the same five-step
drag, resisted/capped geometry, fast deceleration and a delegate-selected zero
destination: about 0.72 seconds to within half a point. Enabling native paging
returned in about 0.27 seconds with either deceleration rate and introduced a
small scroll-offset overshoot. All three returned to zero, but paging did not
match Messages. The probe is under `/tmp/messages-snap-study/`; no paging change
was applied to Exact's scroll host.

A further UIKit-only comparison isolates scroll extent from deceleration rate.
Limiting the extent to 145 points (58 points of visible travel at 40%) removes
the hidden excess distance but still returns about 36 points in the first
100 ms, versus about 19 in Messages. UIKit's normal deceleration rate takes
about 2.6 seconds to return, with either extent. Native overscroll returns
about 35 points in the first 100 ms; that sample had a small nonzero release
velocity and is not a zero-velocity comparison. These variants do not fix
the curve, so neither the app's extent nor the host's deceleration was changed.

A direct pan with a UIKit spring (`mass: 1`, `stiffness: 121`, `damping: 22`,
zero initial velocity) is closer. Saved video pixels give 21.67 points returned
at 100 ms, 40.23 at 200 ms, and within half a point of rest at 0.58 seconds;
the reference gives 18.67, 38, and 0.60 respectively. The spring's RMS position
error against the reference is 1.93 points over the sampled interval. Both
recordings use the last frame within one point of the held position as their
time origin; frame phase limits the comparison. These are fitted probe
parameters, not recovered Messages parameters or an exact match. Artifacts are
under `/tmp/messages-timestamp-rates/`: `spring-video-comparison.json` and
`pixel-comparison.png` contain the pixel comparison; `comparison.json` contains
the separate scroll-delegate and presentation-layer measurements.

The next implementation needs direct hold/release of the visible translation,
using the existing motion engine while preserving vertical scroll arbitration,
reversal, release velocity, stationary labels, and keyboard behavior. A temporary
copy of Messages was prepared to test `dragX` with authored
`translate spring(121, 22, 1)` on its rows. It did not reach runtime verification:
three build attempts encountered a stale copied lockfile, a missing matching
toolchain in the external app, then a copied app/data-module identity mismatch.
The build loop stopped at the repository limit; its host modifications were
restored. `/tmp/messages-timestamp-engine/` preserves the attempted fixture and
final build error. No result from that unbuilt probe establishes Exact spring
behavior, and the app still uses its existing scroll-snap path.

The normal iPhone app also retains timestamp reversal and interactive
keyboard dismissal/reversal with the composer tracking the keyboard. Regression
drives cover inbox read/unread reveal, mute/delete, mirrored scroll offsets,
hidden-scroll restoration, and leaving a conversation during timestamp reveal.
The first inbox run missed the initial gesture and another lost the row; a direct
physical probe and the isolated full run passed without another product change.
The cause of those initial drive failures is not established.
Browser touch and physical Simulator drags verify the hidden track alongside timestamp reveal, return, vertical scrolling,
and swipe-to-reply. A separate fixture switches `auto` → `none` → `thin` → `auto`
without changing the viewport or losing scrolling; native captures show the
indicator disappearing and returning.
The app's surface and text colors follow the system appearance.

The software keyboard has now been verified through timed Simulator touches:
the viewport shrinks by its 335-point overlap, the composer rides above it,
and the bottom safe-area inset becomes zero. UIKit's interactive dismissal
tracks a downward drag; its keyboard layout guide keeps the composer adjacent
through the intermediate frames. This has been driven in the ordinary app,
outside agent mode, where the real keyboard animation runs.
Reversing the drag keeps the keyboard open, and tapping its H key edits the
draft through the native text-input path.

The visual reference is the installed iOS 26.5, pending a different user choice.
Direct inspection of its inbox and conversation informed the bottom search pill,
60-point contact avatar, floating back/name controls, and curved bubble tails.
These render on iOS and web. Bubble tails now use curved CSS clipping paths,
so their outside edges stay transparent over blurred replies and dimmed Tapbacks
in light and dark appearance. The same paths mask native views. Browser and
iPhone simulator captures verify both reply and reaction previews; a separate
clipped-child fixture verifies that real taps outside a curve reach the view
underneath.

Bubble geometry now follows additional iOS 26.5 sample measurements: a 48-point
minimum width, 40-point single-line height, 20-point line spacing, 14-point
horizontal padding, 20-point corners, 16-point transcript side insets, and
4-point spacing within a sender run. The minimum-sized bubbles center their text.
Tails extend below the bubble while staying inside its horizontal edge; the
incoming contour mirrors the sampled outgoing shape. Main messages, focused
replies, and Tapback previews share the dimensions. Group previews retain the
source's 44-point avatar indentation.
Browser and iPhone measurements verify short, multiline, and group cases, along
with reaction selection and opening focused replies. An unsupported percentage
calculation in the first group-preview attempt was replaced by nested layout;
the final group preview opens on both surfaces.

Main, focused, and preview bubbles now use the same two named tail paths.
They follow a measured Messages silhouette, including its rounded tip and the
curve turning inward above it. Three outgoing iPhone samples reduced sampled
edge error from about 0.78 points to 0.10 points; four incoming samples, including
multiline bubbles, are about 0.11 points from the independently captured native
incoming contour. These comparisons normalize to the painted bubble bottom:
they do not hide the remaining fractional body-height/rasterization difference
or establish pixel equality. Bubble frames are unchanged. Browser and iPhone
drives verify focused replies, preview restoration, reactions, and appearance
changes; real Simulator presses and a held palette drag retain draft/keyboard
behavior. An ordinary launch also reveals timestamps by 58 points, returns to
an identical blue-bubble pixel mask, and selects a reaction by double tap and
touch. `/tmp/messages-tail-contour/` holds the contours, scores, captures, and
verification results.

The short native samples measured approximately 48×40 for H/Hi, 67×40 for Hello,
and 113×40 for Hello world. Exact's iPhone results are 48×40, 67×40, and 114×40.
The long reference wraps into the same three lines at 80 points high, but its
background is roughly 262 points wide versus Exact's 277; native Messages shrinks
that balloon to its wrapped text. Exact currently limits a border-box bubble to
75% of the available row. A browser probe also keeps 277.5 points with flex,
column, block, inline-block, fit-content and grid layout. Tightening to wrapped ink
needs a separate sizing design; changing the native kernel’s CSS sizing alone would
create a host disagreement. That intrinsic-sizing difference, fractional bubble
rasterization, and sender-name repetition after pauses remain fidelity work. The initial
multiline native preview was narrower than its source; the final shared row
layout gives main/reply/preview matching measured widths and indentation.

The composer, add control, header controls, and inbox search use UIKit’s regular
glass on iOS 26, with a CSS
approximation on web and ultra-thin blur on earlier iOS. On the reference iPhone,
the resting side and bottom insets are 28 points; opening the keyboard changes
them to 16. Both controls start at 40 points high. Existing CSS safe-area lengths
and zero-minimum spacers produce that adjustment. Multiline growth and clearing
a sent draft preserve the gap above the keyboard. Browser/iOS drives check those
measurements, send/reset, and reactions retaining the draft and keyboard. Actual
Simulator taps also verify focusing the composer, keyboard H, and Send in the
ordinary app. A physical downward drag dismisses the keyboard and restores the
28-point resting insets. Appearance changes also repaint the placeholder and the window
background visible around the keyboard’s rounded corners. The microphone is now an
outlined shape, also used in the inbox search; dictation is still inactive. The
resting search bar shares the reference’s 28-point bottom inset. Glass-backed
buttons hold their content in UIKit’s effect and enable its interactive response.
A held Simulator Back press visibly enlarges the glass and release navigates back.
Browser/iOS drives also check search filtering, contact details, and closing inline
replies through the new material. A native fixture changes glass to blur, clears it,
and restores glass while retaining child actions and physical taps on visible
overflow. Native symbols and glass grouping remain unfinished.

Compose, Send, and Close use explicit clipped or rounded shapes instead of
Unicode arrows and multiplication signs. The compose pencil includes transparent
interior space; Send’s arrow stays centered in its 28-point button. Cancel gets a
64-point text target while composing. These are authored shapes, not SF Symbols;
exact symbol geometry remains part of the visual comparison.

Contact details now pushes a third keyed route with its own Back control, an
80-point avatar, 48-point action circles, an address card and Hide Alerts. The
unknown-phone layout was compared directly with MobileSMS on iOS 26.5; its card
background is `#efeff0`. Fixture phone numbers resolve to their existing contacts.
Hide Alerts changes the same in-memory mute state as the inbox swipe action.
The conversation stays mounted beneath details. Browser and physical Simulator
drives verify mute synchronization, draft retention and prior composer focus:
opening details hides the keyboard, Back restores it only if it was previously
focused, and opening another conversation does not inherit that focus. Details
uses `touch-action: pan-y` so its scroller yields horizontal back gestures.
Native swipe cancellation and completion pass both with the agent connection and
in the ordinary app with live UIKit transitions; held, cancelled and completed
swipes were captured with a real keyboard H draft. Both browser and iOS now
retain the reading position when Back restores the keyboard. Native inspection
isolated the former 160-point jump: the retained transcript grew from 313 to
636 points while details was open, clamping its offset from 260.67 to 97.67;
end-following then treated that clamp as the reader choosing the bottom.
The host now preserves the unpinned position while the route is inactive,
including while it is still attached during an animated push. It also records
UIKit's actual stored offset: its fractional quantization had been mistaken
for a user scroll, invalidating the retained position.

The bounded repair passed on its third round after an earlier loop had stopped.
Browser and iOS drives cover reading, a later manual scroll, top/end following,
and returning without a previously focused composer. Physical native back
swipes cancel and complete with the draft and anchor preserved. Ordinary launch
also passes button Back and native swipe cancellation/completion with a typed
H draft and live keyboard/navigation animations. Before/after transcript crops
differ only in a few one-channel-level pixels (14 for button Back, 6 for swipe
completion); the keyboard crop after the completed swipe is identical. Two
pre-navigation captures confirm that scrolling had settled. The evidence is in
`/tmp/messages-details-inspection/`, including the node inspections, native trace,
driver results, ordinary-launch captures, and pixel comparisons.
Calling, video, email,
contact creation/editing and blocking controls remain disabled visual placeholders.
Group details, shared-content sections, switch motion, exact symbols and the full
native contact-card presentation remain unfinished.

The inbox remains mounted behind the conversation. On iOS, UIKit presents the
Contract routes through its navigation controller. Native back swipes in the
header have passed cancellation and completion with a populated draft: cancelling
keeps the conversation and draft; completion returns to the inbox and saves it.
During an interactive pop, the viewport keeps its keyboard-sized height until
UIKit settles, so the composer stays beside the keyboard as both move sideways.
Programmatic back and reopening drafts also pass on iOS and web. The browser
makes the inactive route hidden and inert, while retaining its nodes and state.

The navigation integration took three fix rounds. Swipes over the horizontal
timestamp scroll area still go to that scroll view rather than navigation; that
arbitration remains unresolved. Header swipes have been driven as actual Simulator
touches, including native transition frames; this is not full navigation parity.

A subsequent app-only probe added `touch-action="pan-right pan-y"` to the
timestamp container, using the already-supported initial-direction policy.
With a keyboard draft, the baseline ignored rightward drags from both x=5 at
the screen edge and x=60 in the gutter beside an outgoing bubble. The candidate
allowed the edge drag to return to the inbox; the gutter drag still did not.
It was reverted because the native comparison did not establish the intended
navigation behavior. On a fresh iPhone 17 / iOS 26.5 simulated Messages session,
rightward edge and gutter drags over an empty conversation did nothing. After
populating its local simulated conversation, edge drags from x=5 and x=15 and
a header drag also did nothing. The same helper's leftward drag revealed
timestamps by about 58 points, and tapping Back returned to the inbox. A
subsequent edge drag on the simulated incoming conversation, with the keyboard
closed, also did not navigate. These controls establish that input reached the
app; they do not establish why its navigation recognizer declined those drags
or that an ordinary iMessage conversation should decline them. A reference
that demonstrates the navigation gesture is needed before this candidate can
be called a parity fix. Captures, before/candidate state and the native pixel
measurements are under `/tmp/messages-transcript-back/`. The app retains its
original timestamp touch policy.

Inline replies now use stable message IDs. Long-press Reply opens the selected
root and its replies over a blurred conversation; the composer stays in place.
Sending keeps the focused thread open, and its reply-count control reopens it
from the main conversation. Close or the exposed blurred background returns to
the conversation. UIKit supplies the ultra-thin material; the browser uses a CSS
approximation. Exact thread positioning and connecting lines,
and the opening/closing motion are still unfinished. The ordinary iOS app has
been driven with physical long-press → Reply → keyboard H → Send touches: it
adds the reply and leaves the keyboard and thread open. Browser and iOS agent
checks cover repeated replies, closing, and reopening via the reply count.
Deleting the original bubble retains the thread ID: subsequent sends, typing,
and the offline incoming reply stay in the focused thread, including when no
bubbles remain at deletion. Web and iOS drives verify this and reopening from a
surviving reply. The exact Messages deletion transition still needs comparison.
The group fixture separates sender names from message text in both views,
following the [current iOS 26 inline-reply reference](https://support.apple.com/en-gb/guide/iphone/iph82fb73ba3/26/ios/26).
Names start incoming sender sequences; 32-point initial avatars sit beside each
run's final bubble, with an 8-point gap. A run requires the same sender, direction
and day, with less than 60 seconds between adjacent messages. The focused thread
groups only its visible messages. A controlled native Simulator drive joined
30.005- and 59.004-second gaps and split a 61.002-second gap; an earlier pause of
at least 68.9 seconds split too. That brackets the chosen one-minute cutoff;
behavior at exactly 60 seconds has not been independently resolved. Native
captures also show the tighter gap within a run and a tail at its end.
Browser and iOS drives reproduce the 30/59/61 cases, including a joined run
whose displayed time crosses a minute, and measure 4-point gaps within a run
and 10 points between runs. Deleting an intervening message recomputes the gap
from the survivors' timestamps; focused replies use the same rule. The native
captures and recorded input times are under `/tmp/messages-grouping-study/`.
Group-name repetition after a pause, avatar artwork and exact fractional spacing
still need direct native comparison. Existing drives also cover changing senders,
reactions, retained drafts, typing and incoming replies.
Physical Simulator and browser touch drags also verify timestamp reveal, reply
entry with the keyboard, and cancellation on the indented group bubbles.
Time labels now use the fixture's day and first visible message in each day,
in both views; sending in an older conversation introduces a Today label.
Focused labels center across the full viewport, including group conversations.
The fixtures' short exchanges share a label, while individual times remain in
the sideways reveal. New sends and simulated replies derive precise timestamps
from the runner's seekable clock, beginning at the fixture's 9:42 AM; displayed
times follow that clock independently of the three-dot animation's 300 ms ticks.
Inbox rows use the latest message's time for both sends and simulated replies.
Deleting that message restores the preceding message's time; deleting the final
message leaves no timestamp, including while retaining a draft. A new send
restores it. Baked-module and browser/iOS drives cover these changes across a
minute boundary (`/tmp/messages-grouping-study/inbox-time-*`); the iOS captures
also check that the new row's timestamp remains inside the viewport.
The calendar labels remain fixed. The threshold for an additional same-day date
heading and exact date typography still need Messages comparison; it is separate
from the bubble-run cutoff above.
A fresh local Messages Simulator sequence kept its original Today heading after
a 301.003-second pause, then a 299.001-second pause (600.005 seconds from the
first send). All three balloons retained their tails. The current browser and
iPhone builds reproduce that heading count and the separate bubble runs; a
five-minute inactivity cutoff would introduce a mismatch in this reference.
`/tmp/messages-date-headings/` holds the input times, native and Exact captures,
OCR coordinates, and layout reads. This does not establish the threshold for a
longer uninterrupted pause, group-name repetition, or calendar rollover.
A subsequent measured 601.005-second native pause also retained the original
heading (`long-input-time.json`, `native-gap-601.png` in that directory); a
ten-minute inactivity cutoff would disagree with this reference too.
A further 901.005-second native pause still kept that heading
(`fifteen-input-time.json`, `native-gap-901.png`). The bounded five-, ten-,
and fifteen-minute exploration has not established a same-day cutoff; do
not introduce one from these samples.
Verification also exposed a host batching defect: a seek crossing typing and
incoming timers could attach the incoming view before creating it. Apple and
web now create surviving views before applying the final child lists; both
hosts have a regression for the multi-timer sequence.
The fixture assumes recipients share [read receipts](https://support.apple.com/en-gb/guide/iphone/iph5e713a045/ios).
Delivered/Read sits below the latest outgoing bubble in each view and remains
there when an incoming reply arrives. New sends show Delivered; the offline
recipient's typing start changes it to Read. Deleting the latest sent message
reveals the preceding outgoing message's saved status. These are local fixture
events, with exact receipt typography and transition motion still unverified.
Web and iOS drives verify the label's position, Delivered-to-Read change,
retention after an incoming reply, deletion, and independent focused-thread status.
The context preview also retains that view's outgoing receipt, as the native
Messages samples do. It updates from Delivered to Read while the menu stays
open, remains after an incoming reply, and disappears from an older outgoing
preview when a newer message owns the receipt. The focused thread uses its own
latest outgoing message. Browser/iOS drives verify those cases and unchanged
preview line count (`/tmp/messages-reply-motion/preview-receipts-*`). Later
matched native captures corrected the spacing: the painted Delivered label
starts 8 points below the bubble body, with its ink ending about 21 points
inside the bubble's trailing edge. The shared transcript/reply/preview style
now uses a 6⅓-point top margin and 20-point trailing margin; Exact's measured
ink gap is 8 points and trailing inset 21⅓, within one device pixel of the
reference (`/tmp/messages-delivery-spacing/`). This verifies settled placement,
not receipt appearance/transition animation or full typography equality.
UIKit keyboard notifications arrive asynchronously; immediate layout queries
can still observe the intermediate zero-height keyboard.

Right-swiping a bubble now opens its focused reply thread. The bubble follows
the drag with resistance past the threshold; reversing cancels, and release
returns it through the existing transition engine. Physical Simulator drags
verify cancellation, leftward timestamp reveal, vertical scrolling, and a
completed right swipe opening Reply and the keyboard. Chrome touch input passes
the same cases and still supports double-tap reactions. The current 64-point
threshold and 180ms return are provisional. An authored reply arrow now
appears to the bubble’s left as the drag grows, and shrinks/fades on reversal or
release. Its `swipeIndicator` policy holds opacity and scale through the existing
gesture path on web and iOS; it takes no taps or accessibility focus. Exact
indicator geometry, motion, and device haptic comparison remain unfinished.
Browser touch input and physical Simulator drags verify its reveal and return in
both the main and focused thread, with leftward timestamps and vertical scrolling
still available. The native driver captures held scroll gestures before querying
the agent: UIKit defers those queries until its tracking loop releases.
CSS `touch-action: pan-right pan-y` reserves the rightward finger drag without
taking the other directions from scrolling.

A fresh iOS 26.5 Simulator reference cannot calibrate swipe-to-reply: its
simulated conversation sends green messages, ignores the measured rightward
path and offers no Reply in its context menu. The same pointer input reveals
timestamps to the left and opens the native long-press menu. This is a limit
of that reference, not a reason to remove Reply from this iMessage example or
to treat its current constants as verified. The held path, reversal and
captures are in `/tmp/messages-reply-motion/native-held*`.

The 2026-09-10 badge-motion probe confirms the same reference limitation:
holding and moving the green SMS balloon produced a floating drag-and-drop
copy while its original and badge stayed still (`/tmp/messages-reply-badges/`).
That is not a swipe-to-reply sample. No badge-following policy was changed on
the strength of it; a native iMessage reply gesture is still needed.

That reference does support context-preview comparisons. Saved pixels for
short, medium, wide, two-line and three-line outgoing messages show enlargement
of about 15% for the short samples, bounded at roughly 26 points of extra width
for the wider samples, with proportional height growth. Their right edges stay
within about a point of the source edge. These are pixel measurements, not a
recovered UIKit formula: the source view's full bounds and highlight target
remain unknown. `corrected-components.json` in the same directory uses colour
differences to include the green gradient; earlier measurements with an absolute
green threshold truncated the lower portions of several balloons and must not
be used as heights. The shared preview now uses that bounded growth and preserves
its source text geometry; final UIKit pixel rounding/clipping still differs.
Earlier clamped captures showed a readable duplicate source behind the menu;
the source-painting repair below removes it without changing the source geometry.

A public UIKit fixture narrows the preview problem (`/tmp/messages-context-study/`,
iPhone 17 / iOS 26.5). A `UIContextMenuInteraction` with a `UITargetedPreview`
of a rounded view produces these settled preview frames, in points:

| Source size | Preview frame size |
|---|---|
| 67 × 40 | 77⅓ × 46 |
| 113 × 40 | 130 × 46 |
| 188 × 40 | 214 × 45⅔ |
| 262 × 80 | 288 × 88 |
| 280 × 40 | 306 × 44 |

These are converted public `UIView` frames in the fixture's own window, with
saved screenshots, rather than colour-threshold estimates of a Messages bubble.
They reproduce the measured Messages width growth and retain the source's
386-point right edge. The live preview's content can extend fractionally beyond
its surrounding frame; the 280-point sample paints through a 308-point content
frame. The source view's own frame, alpha and hidden flag stay unchanged. Setting
`UIPreviewParameters.backgroundColor` to the bubble colour is necessary in this
fixture: `.clear` preserved the text but removed its filled background.

That default presentation is not a usable replacement for the reaction panel
yet. A custom preview controller renders a reaction button, but a physical tap
on it calls `willPerformPreviewActionForMenuWith` and dismisses the menu, without
invoking the button. The identical button receives its action when mounted as
ordinary content; a physical tap on the native Copy menu item also invokes its
action. This isolates preview interaction from a broken button or pointer path.
It agrees with [UIKit's preview commit contract](https://developer.apple.com/documentation/uikit/uicontextmenuinteractiondelegate/contextmenuinteraction(_:willperformpreviewactionformenuwith:animator:)).
No private API was called, and no production context-menu replacement landed.
Any replacement must preserve the interactive, horizontally scrollable reaction
strip and the distinct double-tap/keyboard path as well as native enlargement;
the public preview controller alone does not supply that combination. These
fixture results do not establish Messages' animation curve or which surrounding
badges and labels belong to its preview target.

The existing Contract preview now magnifies on iOS and web by 15%, capped at
26 added points of width. It keeps the source's outside edge and vertical
center until the panel needs to clamp to the safe viewport. Following content
moves down within the panel by half the added height, counteracting the panel's
upward shift: the receipt stays at its source-relative position while the
balloon enlarges. The action card reserves the same status-row space even for
incoming or older outgoing messages without a receipt. Clamping includes the
farther extent of the enlarged balloon or following controls; reactions remain ordinary
interactive content. The `contextTarget` belongs to the balloon itself, and a
row around it preserves the source's text layout (the receipt's column had
allowed a three-line native preview to shrink from 277½ to 262 points before
magnification). The row restores matching source/preview layout on both hosts.
The current three-line Exact sample consequently grows from 277½ to 303½ points;
its source is still wider than the native Messages sample, a separate unresolved
text-sizing difference. This does not replace the custom presentation with
`UIContextMenuInteraction` or reproduce its animation and final pixel clipping.

Geometry drives on web and iOS verify scale, source-edge alignment, the unclamped
vertical center, receipt spacing, action clearance, focused replies and live
Delivered-to-Read updates. Physical Simulator long-press and double-tap drives
verify enlarged previews, reaction hits, a held horizontal palette drag and a
later reaction, outside dismissal, group incoming reactions, and keyboard/draft
restoration (`/tmp/messages-preview-enlargement/`). Those captures preceded the
source-painting repair below. An ordinary launch also verifies long-press reaction selection
and double-tap with a retained draft and software keyboard; the saved keyboard
region is pixel-identical before, during and after the double-tap reaction.
macOS retains unscaled positioning through the same nested preview structure,
verified by opening the existing reaction badge and choosing a reaction.

The receipt-anchor correction is checked separately in
`/tmp/messages-delivery-spacing/`: native short and two-line previews keep
the settled receipt's ink at the same position as the transcript. The first
two-line capture caught its appearance animation (38⅔-point ink width);
two later captures agree at 50 points and the preview's position, so the
intermediate frame is not the geometry reference. Web/iOS drives cover
outgoing, older outgoing, incoming, focused, group, double-tap and clamped
previews, live Read updates and source restoration. Physical Simulator input
covers both recognizers, reaction selection, a held palette drag, outside
dismissal and retained keyboard/draft state on the corrected build.
An ordinary launch with no agent mode verifies a physically typed/sent message:
long-press grows its 200-point balloon to 226 points while the Read ink stays
at x=339, y=712; outside dismissal restores the saved balloon and receipt crops
pixel-for-pixel. A held timestamp drag moves the outgoing edge 386 → 328
points and release restores its original blue-pixel mask. These captures and
measurements are in the same directory (`ordinary-*`).

The duplicate source is now removed through authored `opacity`, on the balloon
and receipt in the active transcript only. The source stays mounted with the
same frame and node identity, so preview placement and scroll geometry remain
valid; dismissal, reaction selection and More restore it. A focused reply's
preview leaves the main transcript's separate representation alone. Live receipt
changes still reach the retained source and appear when it is restored.

A second public UIKit fixture places a source at y=710 and its preview at y=229,
well apart: the source stops painting during preview and returns on dismissal,
while the unrelated comparison bubble stays painted. Its original frame, alpha
and hidden flag remain unchanged; pixels establish the suppression, not those
flags (`/tmp/messages-preview-source/`). Applying that visible behavior to the
custom panel removes the exposed blue duplicate in saved web and iOS clamped
captures, including the duplicate receipt. Geometry/inspection drives cover
restoration, live Delivered-to-Read updates and focused-source isolation; physical
Simulator drives repeat long/double taps, reaction hits, palette dragging,
outside dismissal and keyboard/draft retention. Switching appearance while the
source is suppressed restores its current light/dark colour on dismissal,
verified in saved pixels on both hosts. This still does not reproduce
UIKit's lift/return animation or establish Messages' exact treatment of surrounding
reaction badges, group avatars and sender labels.

This is not yet iMessage parity. Remaining work includes resolving back-swiping
over the transcript, matching the remaining Tapback presentation details,
swipe reply motion matching, timestamp settling, scroll anchoring during animated changes, typing-indicator
motion matching, message insertion motion, precise bubble
geometry, native symbols and glass materials, dark appearance refinement, and direct
visual/gesture comparison with Messages on the target iOS version. Contact actions, group details and
attachment surfaces remain unfinished. Keyboard gesture/animation parity still
needs direct comparison with Messages; text injection alone is not that proof.
