# Panel: LLP 1024 Native modules — a tag that is not a host change (fable)

- **Family:** Anthropic: Claude Fable (`claude-fable-5`), the orchestrating Claude Code session itself — the same session that built the capsule, ran the peers, and edited the fold. It did **not** author the RFC (grok-4.6 did, in an earlier session), so it counts as a panelist; it does not count as an independent *reviewer* of its own fold, which is why the fold is recorded as an edit for Charlie to accept, not an approval.
- **Method:** a three-voice panel at Charlie's request, 2026-08-31. Two rounds, same briefs and gates as the peers (see the grok artifact). Blindness was kept by file mtimes: round 1 written T17:52:07Z, before the first peer output existed (grok T17:55:39Z, sol T18:15:09Z); round 2 written T18:19:05Z after reading both peers' round-1 (that is round 2's design) and before either peer's round 2 existed (grok T18:24:16Z, sol T18:32:10Z). Unlike the peers, this panelist had live repository access; it used it between rounds to verify disputed claims (the results were injected into the shared round-2 brief, marked as orchestrator-verified evidence, visible to all three equally).
- **Gates:** as the grok artifact; this panelist's positions were files in the panel workdir, hashed into no packet of its own (peers received them verbatim inside their round-2 packets, whose hashes are recorded).
- **Runs:** round 1 T17:52:07Z; round 2 T18:19:05Z.
- **Disposition:** folded into LLP 1024 r2 — §8 records the panel. Round-1 catches that drove revisions: the false "custom element is `display: block` — CSS, no deviation" claim (custom elements take no UA rule and are `display: inline`; verified live: the web host's CSS emitter never writes `display` unless the row is set, so D2's tag change would have shipped web-inline/native-block — the fixed-row fix was adopted 3-0), the plan-bytes-reach-`dlopen` path exposure on `nativeViewModuleName` (closed structurally by the A2 shape; the host grammar gate kept as the key check), the missing `ReplyFn`/version handshake, and `renamed()` refusals surviving on custom elements. In round 2 it moved to the roster-at-bake typo net (abandoning its round-1 warning), to the one-app-artifact shape, and yielded its round-1 keep of in-process replace and its composite-the-snapshot Linux position.

---

## Round 1 — position (verbatim)

# LLP 1024 round-1 position (Fable): the shape is right; harden the seam where plan bytes meet dlopen, and fix the one CSS claim that is false

Panel round 1, blind. Judged against `rules-RULES.md`, `rules-DEFERRED.md`, the cited exact2
LLPs, and the code as it stands.

## 1. Overall position

The spine — hyphenated tag → `NativeView` → sibling artifact → one host arm — is right, and it
is honestly derived from what already shipped: the box exists (`kernel-tables-schema.json:14`,
props 34/35 at lines 54–55), the Apple host already emits `"native"` for it
(`host-apple-host.rs:646`), the ABI it copies is real and complete enough to have survived
WebKit process death and sandbox escapes (`host-apple-webarm-WebArm.swift:365–405`), and the
inode rule it inherits is written down where it was learned (`host-apple-build.mjs:227–228`).
The refusal of ModuleIR, of cargo features, and of a ninth agent operation are all correct.

I would materially change four things, in leverage order:

1. **The host must validate `nativeViewModuleName` before it touches a path.** (MATERIAL — §4)
2. **The ABI as tabled cannot answer a snapshot, cannot detect version skew, and never states
   its threading contract.** (MATERIAL — §3)
3. **The D1 claim "a custom element is `display: block` … CSS, no deviation" is false as a
   statement about CSS**, and the repo's core rule makes that a real defect, with a clean fix.
   (MATERIAL — §2)
4. **`$EXACT_MODULES` should be a dev-build mechanism, refused in release**, parallel to
   `EXACT_DEV_PLAN`. (ADVISORY, close to material — §4)

## 2. The admission rule (D1)

**The grammar-only admission test is right.** The web's rule (a hyphen admits, `texxt` stays
refused) is the correct test precisely because it needs no registry — a declaration list to
check tags against would be the first step back toward ModuleIR, and the compiler cannot know
the runtime roster anyway. The typo'd hyphenated tag (`ghosty-terminal`) rendering an empty box
is also the web's own behavior — an unupgraded custom element is inert — so the parity oracle
supports the RFC. But the RFC should *say* the failure is loud where agents look: `tree` reports
`{unavailable: true}` **with the module name**, and `logs` carries one line naming the tag and
the search paths tried. An agent driving `node scripts/agent.mjs web tree` must be able to see
the typo without a screenshot.

**Keep `renamed()` refusals on custom elements.** `contract-lower-tags.rs:296–351` exists so
`fontSize=13` is refused with the CSS name. If unknown attrs on a hyphenated tag silently become
module props, `fontSize` becomes a legal module prop and the migration guard is gone exactly
where authors will be writing the most unfamiliar markup. Rule: on a custom element, the global
attr table binds first (as the RFC says), `renamed()` spellings are still refused, and only then
does the leftover become a props key. Cheap, and consistent with tags.rs's own header ("there is
no fallback attribute" — the fallback here is scoped, deliberate, and should still exclude the
known-bad spellings).

**MATERIAL — the `display` claim is wrong.** D1 says a custom element "is `display: block` with
auto sizes … CSS, no deviation." In CSS, a custom element — defined or not — has **no UA style
rule; it is `display: inline`** (which is why every custom-element tutorial begins with
`:host { display: block }`). The kernel has no `inline` (`kernel-tables-schema.json:80`,
`Display: block|flex|grid|none`), and `llp-1001-kernel-v1.spec.md:56` keeps a declared-deviations
list for exactly this. Two honest fixes; I prefer the first:

- **Give the tag a fixed `display: block` row**, the way `column` carries fixed rows
  (`contract-lower-tags.rs:65–71`). Then the block-ness is authored plan content, not a silent
  kernel default; the web host emits the same explicit style on the element; the parity oracle
  compares against HTML that also says `display:block`. No kernel deviation, one line of
  lowering.
- Or declare the deviation in 1001's list. Weaker: it leaves the parity harness lying about
  bare custom elements.

**Unlocking `load`/`message` on hyphenated tags is right** — they are the module lifecycle
exactly as they are the iframe's (`contract-lower-tags.rs:168–169`).

## 3. The ABI (D4)

Copying the iframe ABI is right — but the copy dropped load-bearing parts of the original and
inherits none of its immunity to version skew.

- **MATERIAL — no reply channel.** WebArm's create takes *both* callbacks — `EventFn` and
  `ReplyFn` (`host-apple-webarm-WebArm.swift:14–19, 365–369`) — and `snapshot` answers through
  `reply` with a token (`:349–358, 386–389`). D4's table gives `exact_native_create(id, name,
  props_json, event_fn, ctx)` and an `exact_native_snapshot(handle, token)` with no way to send
  the pixels back. Add `reply_fn` to create, verbatim from the iframe shape.
- **MATERIAL — no version handshake.** The iframe dylib is built with the host, in the same
  `build.mjs` run, so skew is impossible. A module is the opposite by design: an app-side
  artifact iterated against a host binary its author did not build. The premise of the RFC —
  "adding `ghostty-terminal` does not edit the host" — creates the skew the iframe never had.
  Add `exact_native_abi(void) → u32` (a single ABI major), checked immediately after `dlopen`;
  mismatch is per-node `{unavailable: true}` plus a logged line naming both numbers, never a
  crash inside the module's `create`.
- **State the threading contract in one sentence.** WebArm is main-thread and trampolines its
  own async work through `DispatchQueue.main` (`host-apple-webarm-WebArm.swift:106`). The ABI
  doc should say: every export is called on the platform's UI thread; `event`/`reply` may be
  called from any thread and the host makes them safe. An ABI that doesn't state this gets it
  wrong in the first third-party module.
- **Resize needs no export on Apple/web** — the platform view is a subview of the node's box and
  observes its own bounds; worth one sentence so nobody adds a resize export later. Linux is the
  open half (see Q3).
- For the PTY consumer: the ABI is not what's missing — instance-state survival is, and the RFC
  already cuts it with the right return trigger (§5). Agree.

## 4. Lookup and lifecycle (D5, D6, D7)

**MATERIAL — the module name is plan bytes, and plan bytes are network bytes.** LLP 1023 moves
plans over the LAN, and its own Stage-1 incident was a *foreign plan booting a local binary*.
`nativeViewModuleName` is a `str` prop read from the plan (`kernel-tables-schema.json:54`), and
D5 concatenates it into filesystem paths that reach `dlopen`. A byte-doctored plan carrying
`../../../tmp/evil` as a module name must die at the host, not at `dlopen`. The RFC's grammar
lives in the *compiler*; the *host* trusts the wire. Rule to add to D5: before any path
construction, the host re-validates the name against the same custom-element grammar (ASCII
letter start, contains `-`, `[a-z0-9-]` only, no `/`, no `.`, bounded length); an invalid name
is `{unavailable: true}` with a logged refusal. This is the same class of fix as 1023's app_id
gate, and it costs ten lines.

**`$EXACT_MODULES` should not exist in release builds.** In dev it is exactly `EXACT_DEV_PLAN`'s
peer and fine. In a release/store build the roster is the bundle, full stop — on macOS, library
validation under the hardened runtime already refuses other-team dylibs (the signing posture of
LLP 1018 D7), so honoring the env var there buys nothing but an attack-surface conversation.
One sentence: dev builds honor the env var; release builds search the bundle only. (ADVISORY,
but I'd land it with the seam.)

The rest of D5/D6/D7 is right: generationed inodes are the recorded lesson
(`host-apple-build.mjs:227–228`); no-`dlclose`-for-Swift is correct and the park-v1 cost is
honest; replace-only-that-module without a plan reload is the correct division (the host holds
the node's current props to replay into v2); the web shell-element-holding-a-pointer is the same
trick as swapping the `NSView` inside `NodeView` and is the right answer to
`customElements.define` being once-only. One addition to D7: the shell's constructor should set
the explicit `display` from §2 so the DOM element and the kernel agree even before upgrade.

**Owed paragraph: focus and keys.** A terminal eats keystrokes. On Apple the platform view will
become first responder and the host's key handler must not fight it — presumably iframe's
standing answer (WKWebView takes its own keys). The RFC lists `key`/`focus`/`blur` as events a
module may emit but never says who owns the keyboard while the module's view is focused. One
paragraph, iframe-parity. (ADVISORY.)

## 5. The landing (D8) and the cuts (§5)

Fixture-first is right, and the fixture's assertions should include the *failure* path: the
smoke drives a plan naming a module that does not exist and asserts `{unavailable: true}` plus
the logged line — the empty-box behavior is a contract, so it gets a test.

**Pick the packaging authority.** D3 waffles: "a hyphenated tag in the contract (or a `modules/`
directory next to `gpu/`)." Pick the directory: `modules/<tag-name>/` is the roster, `build.mjs`
copies what is there, gated exactly like GPU's `existsSync(gpu/Cargo.toml)`
(`host-apple-build.mjs:178`), and *warns* when the contract names a hyphenated tag with no
module in the roster (that warning is the typo-catcher from §2, at build time, where it's
cheapest). The contract cannot be the authority — the compiler is app-agnostic and the plan is
a wire artifact.

The cuts table is good. Agree in particular with cutting intrinsic-size (the `Image` seam
exists when needed), instance snapshot, and eval. The §2 claim "adding `ghostty-terminal` does
not edit `schema.json`" is honest — complexity lands on the module author, which is the point —
provided the ABI hardening in §3 lands, because that is what makes the host safe to *not* edit.

## 6. The four open questions

1. **One artifact per tag. Yes.** The iterate-independently argument is decisive: a Ghostty
   rebuild must not relink the fixture. GPU's inner table stays GPU's
   (`apps-caltrain-gpu-lib.rs` is the app's own crate; different situation). iOS signs a
   `Frameworks/` directory of many dylibs routinely.
2. **Close it as a non-question.** Hyphenated names are the web's reserved-for-authors space;
   HTML has promised not to ship hyphenated element names. Delete the question, keep one line.
3. **Linux: composite the snapshot.** The RFC's own recommendation wants the fixture's pixels
   in `screenshot`, and the D4 table has no render path — those conflict. Resolution for v1:
   the Linux host calls `exact_native_snapshot` at commit time and composites the returned
   buffer into its scene (fixture is static; fine). Live per-frame module rendering on Linux is
   a cut with a named return trigger (a module that animates). Alternative — `{unavailable}`
   on Linux entirely — is acceptable but wastes the one host that can run headless in CI.
4. **`fs.watch` first, SSE as the envelope-shaped sequel. Yes** — and the ping should carry
   the generation path, not a bare "changed," so the host never re-scans.

## 7. What the RFC misses

Covered above; the two that are genuinely missing rather than under-specified: **host-side name
validation** (§4 — the one catch I would block on) and **the focus/keys paragraph** (§4). One
small correction: the web host's `NativeView → "div"` mapping lives in `host/web/src/host.rs`
(the wasm host, `tag_for`-equivalent), not in the glue — worth citing precisely when D2 edits it.

## Three decisions for round 2

1. **Admission**: grammar-only at lowering (my position) vs any form of declared roster gating
   the compiler — and whether the build-time warning on roster-miss is enough of a typo net.
2. **Linux pixels in v1**: composite-the-snapshot (my position) vs `{unavailable}` on Linux
   until a real consumer.
3. **ABI hardening as landing-blockers**: reply-fn + version export + name validation +
   threading sentence in the first landing (my position) vs deferring any of them.

---

## Round 2 — final positions (verbatim)

# LLP 1024 round-2 (Fable): two positions moved, one held, one pressed

Deltas only. Where I am silent on §B, I confirm.

## A1 — Admission: I move to (ii), grok's roster-at-bake

Sol's round-1 convinced me a warning is too weak, but not that Contract needs new grammar. The
web's own model settles the shape: the parser accepts any hyphenated name, `customElements.
define` is the *runtime* declaration, and an undefined element is inert. Exact's analogue of
`define` is the module artifact in the app's roster — packaging, not language. So: grammar-only
at lowering (with A5's lowercase/reserved-name tightening); the app's `modules/` roster is the
declaration; bake — which already boots the runner and lints the first frame (LLP 1017 P1d) —
refuses a hyphenated tag with no roster entry as `bake-unknown-module`, naming the tag and the
nearest roster name, inside the ~20 ms dev loop. The author sees: (1) typo → named bake
diagnostic with a suggestion; (2) declared-but-unloadable on this host → empty box, `tree`
module status, one `logs` line; (3) platform-unsupported (Linux) → same, `state: "unavailable"`.

Against (iii): the compiler is app-agnostic and the plan is a wire artifact; a `module`
declaration puts a packaging fact into language surface that LLP 1017 just fought to keep
minimal, and a consistent typo (declaration + use) defeats it anyway. If Charlie wants
compile-time errors regardless, sol's name-only form is the only acceptable variant — never a
schema. My vote: (ii).

## A2 — Artifact shape: I move to sol's one app artifact with an inner table

Grok's relink argument is real but priced wrong for v1: the repo has one fixture module and
zero consumers. One app-scoped artifact — a `modules/` crate beside `gpu/`, one stable load
name, a hand-written tag→factory table, absent when the app has none — is the GPU precedent as
actually built (`apps-caltrain-gpu-lib.rs`, `host-apple-build.mjs:177-196`), and it deletes the
v1 machinery all three of us flagged: no search order, no first-hit-wins, no per-tag generation
symlinks, no `$EXACT_MODULES` directory. It also closes my round-1 path-traversal catch
structurally — the module name becomes a table key, never a filesystem path. (Keep the ten-line
PCEN re-validation at the host boundary anyway: the name still becomes a DOM tag on the web and
an error string everywhere, and plan bytes are network bytes.) Dev loop under this shape: edit
the modules crate → `build.mjs` writes a new inode → restart page/process — exactly GPU's loop
today, consistent with cutting live-swap (§B6). Per-tag artifacts return on a named trigger:
two unrelated large modules in one app with measured relink or co-loading cost. `create` keeps
its `name` argument so the split never changes the C surface (grok r1's point, kept).

## A3 — Bounds: I hold with observation; `set_bounds` is a named cut

The v1 platforms both deliver bounds natively: the host sizes `NodeView`, the platform view
fills it (`autoresizingMask`, `web?.frame = v.bounds` on every frame op —
`host-apple-macos-Presenter.swift:290-295, 859`), and the module observes its own view, scale
included (`backingScaleFactor` / `devicePixelRatio`). That is the iframe contract verbatim, and
a second size channel is two sources of truth. Sol's headless argument is right about the
future and wrong about the landing: v1 has no headless module target (Linux is unavailable,
§B8). The versioned function table makes `set_bounds` additive later; cut it with trigger "a
module target with no platform view, or a measured observation gap (IME-driven resize, scale
change missed)." State the observation contract in D4 in one sentence so no module author
invents an export.

## A4 — `display`: pressed, with the fix as round 1 stated it

The evidence is in the brief and is verified code, not spec-lawyering: `css.rs` never emits
`display` unless the row is set; swapping `div` for the custom-element tag makes a bare module
tag inline in the browser and block on native. Grok's round-1 endorsement of the RFC's "CSS, no
deviation" sentence is the one factual error left standing from the draft. Fix: the
custom-element tag lowering emits a fixed `display: block` row (the `column` precedent), the
web shell carries `display:block` baseline against pre-upgrade flash, and the RFC's D1 bullet
is rewritten to say what is true: the web's default for a custom element is inline; Exact's
module *tag* declares block the way `column` declares its direction — a tag row, not a kernel
deviation.

## A5 — Snapshot: optional by capability bit; the fixture implements it

Sol is right that ordinary views composite in normal capture and WKWebView's path exists for
remote layers; grok is right that D8.3's smoke must prove the tokened handshake. Both hold at
once: `snapshot` is optional (a capability bit in the versioned table), the host falls back to
normal capture when absent, and the fixture *implements it* precisely because the first real
consumer (Ghostty, Metal-backed) will need it. The smoke asserts both paths: the fixture's
color via the handshake, and a second no-snapshot fixture (or the same module with the bit off)
via plain capture.

## B — Confirmations, two with notes

All fifteen confirmed. B1: the versioned entry should be sol's single exported constructor
returning a size-versioned function table — it subsumes grok's `exact_native_abi()` and makes
every later addition (`set_bounds`, `agent_input`) non-breaking. B6: cutting live-swap also
resolves my round-1 position; I held replace-in-place in round 1 and I yield to the
restart-shaped precedent — GPU and iframe iterate by new inode + next process, the PTY dies
either way, and D7's pointer registry existed only to serve the thing being cut.

## C — The converged plan as I now understand it

A hyphenated, lowercase, non-reserved tag lowers to `NativeView` carrying the tag name, a fixed
`display: block` row, bound known attributes, and a literals-only canonically-sorted JSON
aggregate in `nativeViewProps`; the app declares its modules by having them — a `modules/`
crate per app, one sibling artifact per platform with an inner tag→factory table, copied by
`build.mjs` when present, checked at bake so a typo is a named diagnostic in the dev loop. The
host has one arm: on first create *after the paint gate* it loads the artifact (dlopen /
injected script), asks the table for the tag's factory, inserts the platform view into the
node's box, and replays props; missing or failing modules are a per-node status in `tree` and a
line in `logs`, never a boot refusal. The ABI is one versioned constructor returning a function
table — create (with event *and* reply callbacks), platform_view, set_props, destroy, optional
snapshot — with written threading rules; bounds are observed, not pushed. Iteration is a new
inode and a page/process restart; live-swap, `set_bounds`, `agent_input`, computed props,
Linux loading, per-tag artifacts, and any module ping are named cuts with return triggers. iOS
stays a frozen signed roster; nothing native ever crosses the LAN; the landing waits for a
named consumer and implementer, and the v1 bar does not move.

**Blocking if folded tonight:** nothing, provided A1 lands as (ii) or (iii) — not round-1 (i) —
and A4's fix lands. If grok holds per-tag artifacts or sol holds the Contract declaration after
this round, the fold records the split as an open question for Charlie with the panel's 2-1
vote noted honestly.
