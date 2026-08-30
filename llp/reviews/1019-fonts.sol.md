# Panel: LLP 1019 Fonts — declared, not loaded (sol)

- **Family:** OpenAI: `codex exec --json -m gpt-5.6-sol -c model_reasoning_effort=ultra -s read-only --skip-git-repo-check -`. Verified against the rollout `~/.codex/sessions/2026/08/30/rollout-2026-08-30T09-04-54-01a0536a-….jsonl`: `"model":"gpt-5.6-sol"`, `"effort":"ultra"` — requested and reported agree.
- **Method:** a panel at Charlie's request, 2026-08-30 — LLP 1016 §6's sense: not a refine loop, no verdict binds, no approval given or withheld. Two rounds. **Round 1 blind** (neither panelist saw the other's answer); **round 2 mutually visible** (each read the other's round-1 answer in full and was asked to write only where they differ). Workdir = a read-only panel capsule, no network: LLP 1019 r1, the panel brief, exact2's four host text paths (`host-web-css.rs`, `host-apple-Text.swift`, `host-apple-measure.rs`, `host-linux-text.rs`), the kernel text IR and its style rows, `contract-lower-tags.rs`, LLP 1005 and 1011, both app contracts (Caltrain and Weird Castle), the rules (`RULES.md`, `NOT-DOING.md`, `CLAUDE.md`), and exact1's whole font system (`fonts-and-assets.md`, `assets-fonts-{state,registry}.ts`, the Apple and Windows registries, the 2026-07-02 Apple text-stack audit, the LLP 0470 limitations excerpt).
- **Gates:** LLP 1019 r1 sha256 `159cfed094e038dc70abc700684df34cc75dc9c3c247f9041ee8afb8574a48a1`; round-1 brief sha256 `36c3e6bf507284200fe1320011a2e1d2e03b13ef7c26fd78b57faf5e087ac3b0`; round-2 brief sha256 `941d016f69d4ee6defcc9c7888e5380168ef63593bcb2934e4d55750af1f5774`; capsule (evidence files, panel answers excluded) sha256 `2e8c5c8e28bc43be56856c3a82671bd61df66c67e8ce3b81dffb349983bac5b9`. No capsule file was modified while either panelist was in flight.
- **Runs:** round 1 2026-08-30T16:04:53Z → T16:17:08Z; round 2 → T16:25:52Z.
- **Disposition:** folded into LLP 1019 r2 §11. The panel's factual catches against the live repo were re-verified by the author before folding — see §11 for which held.

---

## Round 1 — position (verbatim; the CLI's narration lines before the document marker are dropped)

# LLP 1019: Font panel position

## 1

**Position**

Compile-time declarations and compiler-assigned IDs are the right v1 boundary. Delete the stronger claim that “nothing resolves at runtime.” The compiler should freeze identity; hosts still resolve asset paths, parse/register faces, match styles and weights, and perform glyph fallback.

This does not foreclose an OTA release that atomically replaces the plan and its asset root. It intentionally forecloses a font arriving independently as runtime data, which is appropriate while the update economy is explicitly out of v1 (`exact2/NOT-DOING.md:36-51`).

**Reason**

Stable per-plan IDs eliminate exact1’s worst coherence machinery: styles had to re-resolve after loading (`exact1/assets-fonts-state.ts:54-60`), Windows tracked registry generations (`exact1/windows-fonts.rs:71-82`), and Apple separately invalidated measurement caches (`exact1/PlatformTypes-font-registry.swift:78-95`).

But IDs are stable only for one plan. A new plan may reuse ID 10 for different bytes. Apple’s present font and paragraph caches are process-global (`exact2/host-apple-Text.swift:63-69`), so plan replacement must install a new plan-scoped catalog and discard ID-dependent caches. “No `unloadFont`” may be true at the application API; “nothing to unload” is false for process-global native registration.

The RFC must also decide whether `font-family` is literal-only. Current dynamic styles accept generic runtime numbers or strings (`exact2/1005-plan-and-runner-v1.spec.md:146-155`), but D2 never explains how `font-family=preference` becomes a validated stack ID. For v1, I would permit literals and closed control-flow choices among literals, not arbitrary data strings.

The compiler work is negligible. The development-loop cost is font I/O, parsing, registration, and web readiness. The approximately 20 ms plan restart (`exact2/CLAUDE.md:30-32`) remains plausible when the font catalog is unchanged and loaded handles can be reused by content identity; changing font bytes is a slower font-preflight restart and must still respect the 100 ms restart/first-frame budgets (`exact2/RULES.md:35-43`).

**What would change my mind**

A real v1 requirement for downloaded, document-provided, or arbitrary user fonts. I would then keep stable declared aliases but allow a resource to populate one, accepting explicit readiness, invalidation, and relayout semantics.

## 2

**Position**

An ID should name an ordered stack, and concrete system family names should not be built into the portable Contract vocabulary. However, D2/D3’s table design is wrong: a declared family and an interned stack are different entities.

**Reason**

D2 proposes `fonts(name, faces)` and `faces`, while D3 says `font_family` identifies an entire list (`1019-fonts.rfc.md:145-176`). The plan needs at least:

- `faces`: source plus weight/style or variable ranges
- `families`: declared alias plus face range
- `stacks`: ordered member range
- `stack_members`: declared-family reference or generic-family enum

`font_family: u16` should index `stacks`, with semantic bounds validation. These belong in `plan/tables/format.json`, the existing declaration authority (`exact2/1005-plan-and-runner-v1.spec.md:22-44`), not in a side manifest.

The Linux claim that the host can simply expand a stack is unsupported. Shaping currently receives one `Family::SansSerif` (`exact2/host-linux-text.rs:289-297`); the families slice inside `fontdb::Query` is only a face query. Linux’s own comments show uncontrolled fallback selecting unrelated installed fonts (`exact2/host-linux-text.rs:209-216`). If ordered per-glyph cascade cannot be implemented, mixed-script text, emoji, wrapping, and even the primary Latin face can diverge. That must be a declared host limitation, not “what the platform does anyway.”

“No concrete built-ins” is right at the Contract level. Hosts still map generics to concrete installed fonts, as `sans_family` already does (`exact2/host-linux-text.rs:157-197`). The RFC must also assign distinct meanings to `ui-sans-serif` versus `sans-serif`; its slash notation currently leaves a wire decision unresolved.

**What would change my mind**

A four-host prototype showing that one simpler table can unambiguously represent declared aliases, faces, generics, ordered stacks, and per-glyph fallback without sentinel conventions or host string rediscovery.

## 3

**Position**

“One resolver function” is a useful code-review smell, not the invariant. The enforceable invariant is one plan-scoped font catalog and one canonical resolution result—or shaped paragraph—shared by measure and paint.

Family-first matching is correct. Linux’s `snap_weight` is an adapter workaround, not cross-host policy.

**Reason**

Apple’s real guarantee is stronger than a shared helper: measurement obtains cached `CTLine`s and painting draws those same lines (`exact2/host-apple-Text.swift:41-55,114-124,185-218`). Linux similarly keeps the shaped cosmic-text `Buffer` for both operations (`exact2/host-linux-text.rs:105-116`). Preserve that dataflow.

All cache identities must gain plan/catalog and stack identity. Apple’s font key currently contains only size, weight, and italic (`exact2/host-apple-Text.swift:67-69`). Linux’s snapped-weight cache is only `(weight, italic)`, and its natural-line-height cache also omits family (`exact2/host-linux-text.rs:142-146,264-329`). Both become incorrect immediately after adding a second family.

The exact1 audit shows why function count alone fails: the main measure/paint paths shared `styledPlatformFont`, while later text consumers bypassed it and one omitted weight (`exact1/2026-07-02-d3-apple-text-stack-review.md:84-127`). Because Contract attributes are not tag-specific (`exact2/contract-lower-tags.rs:149-266`) and Weird Castle contains text inputs (`exact2/weird-castle.app.contract:123-124`), LLP 1019 must either route input fonts through the same catalog or reject `font-family` there. Otherwise resolver number two appears immediately.

Normatively specify CSS’s stages: within each stack family, match style and then weight using CSS’s full ordering, handle variable ranges, perform per-glyph family fallback, and only then consider synthesis. `snap_weight` currently returns one scalar for one generic family (`exact2/host-linux-text.rs:254-297`); that scalar cannot be reused across fallback families with different inventories.

Agreement should be tested through resolved face identity and painted geometry, not merely by asserting that 400 and 700 have different measured widths as OQ4 proposes (`1019-fonts.rfc.md:292-297`).

**What would change my mind**

A host architecture where measurement and painting cannot share shaped output. In that case I would require a shared immutable face descriptor plus adversarial parity fixtures at both consumers.

## 4

**Position**

D5 is false as written. Native hosts can block on local registration, but `@font-face` is asynchronous. The honest v1 design is a host-owned font-ready boot barrier before the first visible application frame.

**Reason**

Emitting `@font-face` neither fetches nor decodes every face synchronously. Omitting `font-display` still selects the browser’s `auto` policy. Without a barrier, the first web frame may use fallback metrics or invisible glyphs and later reflow, contradicting “no loading state, no FOUT” (`1019-fonts.rfc.md:194-204`).

The web host should create/load every declared `FontFace`, await all attempts, add successful faces to `document.fonts`, and only then mount or reveal the Contract tree. exact1 already demonstrates the necessary host mechanism by awaiting `face.load()` (`exact1/assets-fonts-registry.ts:247-271`). This is not application JavaScript and creates no application-observable font state, but it is still host loading.

Failures need semantics. I would mark a failed declared family unavailable, continue through its stack, emit a boot diagnostic, and present only after that fallback decision is settled. Native hosts should do the same when CoreText registration or font parsing fails. Silent registration failure is unacceptable because it changes layout while pretending the requested face succeeded.

This preserves the web as face-selection oracle at the first visible, font-ready frame. It also makes font fetch/decode part of the 100 ms cold-first-frame budget (`exact2/RULES.md:41-43`). If that budget cannot hold, the alternative is to accept FOUT explicitly and define parity as eventual; LLP 1019 cannot claim both immediate first frame and synchronous web fonts.

**What would change my mind**

Measured evidence that the readiness barrier cannot meet the boot budget for the actual bundled Weird Castle font, together with a governing decision that eventual parity and browser-native reflow are acceptable.

## 5

**Position**

The RFC poses a false binary. CSS first chooses the best real face; synthesis is a later permission when the selected family still lacks the requested weight or style.

Recommendation: synthesize — confidence 82%

**Reason**

A request for 600 against real 400 and 700 faces should select the real face required by CSS’s weight-matching order—normally 700 in that case—not synthesize a 600 face. Synthesis matters when, for example, the family has only upright regular yet bold or italic is requested.

After real-face matching, I recommend CSS’s default because “web is the standard” is binding (`exact2/RULES.md:51-53`; `exact2/CLAUDE.md:7-17`). Native algorithms need not be pixel-identical; they must preserve the requested semantic distinction. exact1’s documentation chose no synthesis (`exact1/fonts-and-assets.md:358-399`), while Apple shipped a synthetic oblique anyway (`exact1/2026-07-02-d3-apple-text-stack-review.md:95-100`), demonstrating that an unspecified policy will drift.

V1 should promise only weight and style synthesis, not small-caps. A future `font-synthesis="none"` is the correct brand-quality escape hatch. If nearest-face wins instead, web must explicitly emit `font-synthesis: none` and the CSS deviation must be recorded rather than emerging accidentally from Linux’s current limitations.

**What would change my mind**

A four-host fixture using the actual Weird Castle face showing materially unacceptable metric or visual divergence, or evidence that controllable weight/style synthesis on Linux requires enough new raster machinery to violate the scope and boot budgets.

## 6

**Position**

Require declarations in v1. Generics are the only families usable without one.

**Reason**

Accepting arbitrary `"Menlo"` strings silently makes one Contract depend on host installation state and recreates exact1’s concrete-name/alias problem (`exact1/assets-fonts-state.ts:21-44,107-128`). Linux’s system library is still useful through portable generics; its `sans_family` already resolves those against installed fonts (`exact2/host-linux-text.rs:157-197`).

The RFC’s Helvetica example is currently inconsistent: it says Helvetica is declared like any family, but D1’s only declaration syntax requires a file (`1019-fonts.rfc.md:126-143,169-172`). For v1, that means “bundle and declare it.”

If an editor or document viewer later needs installed fonts, add an explicit nondeterministic source such as `local("Menlo")` under a declared alias and require a portable fallback stack. Do not make undeclared strings acquire that behavior implicitly.

**What would change my mind**

A named real application that must render user-selected or document-specified installed fonts across its supported hosts, with an explicit availability and fallback contract.

## 7

**Position**

Do not implement LLP 1019 as written. Implement a narrower bundled-font slice only after Charlie names Weird Castle as the v1-driving app or makes the required scope trade.

**Reason**

The binding scope defines v1 by one real app, currently recommends Caltrain, and excludes features that app does not require (`exact2/NOT-DOING.md:5-12`). Moving work into scope requires naming what it unblocks and removing something else (`exact2/NOT-DOING.md:141-144`). Weird Castle is a legitimate reason, but that trade has not been recorded.

The RFC also incorrectly says Weird Castle needs D1–D5 and nothing beyond (`1019-fonts.rfc.md:247-254`). The app requests 600, 700, and 800 (`exact2/weird-castle.app.contract:83,104-106,122-145`), so using the brand family requires D6. It contains no italic requirement, so D7—not D6—is the unevidenced scope addition.

The decisions most likely to force rewrites are:

- The separate face/family/stack representation and validation.
- Whether a single source means a static 400-normal face or compiler-inspected variable axes. One `weight: u16, italic: bool` face cannot represent the variable-family shorthand promised by D1 (`1019-fonts.rfc.md:136-149,217-218`).
- Whether `font_weight` remains integer 100–900, becomes CSS’s wider/fractional domain, or records an explicit deviation (`exact2/kernel-text.rs:19-26`).
- Cross-host file formats, build-time metadata/path validation, Contract aliases versus embedded family/PostScript names, and corrupt-file behavior. exact1 had to preserve both embedded identifiers (`exact1/PlatformTypes-font-registry.swift:61-74,102-114`).
- Plan replacement, simultaneous kernels, native process-global registration, and cache teardown.
- CSS inheritance and the scope of `font-family` on text inputs and other text-bearing controls.
- A real bundled-font fixture covering face identity, weight, italic, missing glyphs, and stack order. It should extend the existing `test` check; native four-host sweeps should remain asynchronous rather than becoming a sixth blocker (`exact2/RULES.md:9-29,78-84`).

Before implementation, the RFC also needs an implementer/date, an explicit human authorization for an agent to add a font registry (`exact2/RULES.md:58-67`), and—if linked into the current set—a document archived to preserve the 15-document budget (`exact2/RULES.md:11-13`).

**What would change my mind**

A recorded scope trade naming Weird Castle, the actual font files and required faces, and a minimal prototype demonstrating the typed plan tables and web readiness path within the boot and check budgets.

## The one thing I would change

Replace D2/D3 with one normative, plan-scoped font model: typed `faces`, `families`, `stacks`, and `stack_members`; explicit generic members; `font_family` validated as a stack index; variable ranges or an explicit static-only restriction; and host caches scoped to that plan identity. Until that model is unambiguous, every resolver, readiness rule, and parity test is being built against an unstable binary contract.

---

## Round 2 — cross-response, mutually visible (verbatim)

# OpenAI / GPT-5.6 Sol

## 1

Recommendation: synthesize — confidence 78%

A moved me from 82% to 78% because Linux synthesis may be real implementation work, and exact2 can diagnose incomplete bundled-face declarations earlier than a browser can.

That does not override the binding rule: defaults and behavior that can follow CSS do so unless the deviation is unavoidable (`exact2/CLAUDE.md:9-17`; `exact2/RULES.md:51-53`). Cross-host faux-bold algorithms may differ visually, but nearest-face can erase the requested distinction entirely—bold becomes regular, italic becomes upright. The portable guarantee should be semantic contrast, not pixel-identical synthesis.

Weird Castle’s literal 800-weight wordmark should ship a real 800 face or variable range (`exact2/weird-castle.app.contract:104`); make an incomplete declaration a diagnostic, not the framework-wide fallback policy. The cost asymmetry is also weaker than A claims: macOS and iOS share one implementation, and nearest-face still requires suppressing Apple’s existing italic-trait request (`exact2/host-apple-Text.swift:82-89`). A bounded Linux spike proving that synthesis requires a new raster subsystem would move me to nearest-face as a declared CSS deviation.

## 2

Neither round-one statement was exact. I was right about the scope gate but too strict about its sequencing.

The binding scope still has one unnamed v1 app, recommends Caltrain, and says everything unnecessary to that app does not exist (`exact2/NOT-DOING.md:5-12`). Weird Castle being an external consumer (`exact2/CLAUDE.md:27-29`) does not authorize font work; that file explicitly says it is nonbinding (`:3`).

However, the trade need not precede coding: `NOT-DOING.md:141-144` requires it in the same PR. A thin prototype is permissible, but font support cannot be authorized or landed as v1 work until Charlie records what it unblocks and what leaves scope. An agent also needs explicit human approval to add the registry (`exact2/RULES.md:64-67`).

## 3

Declare authored, ordered per-glyph CSS stacks out of the thin v1. Preserve ordinary platform last-resort glyph fallback so emoji and unsupported scripts still render.

A found an Apple omission I accept: merely adding `family` to `Text.font` does not preserve an authored cascade. But returning one `PlatformFont` is not itself the blocker; that font can be created from a descriptor carrying the CoreText cascade. The existing shared `CTLine` measure/paint dataflow can remain (`exact2/host-apple-Text.swift:103-142,185-196`).

A is wrong on Linux. `fontdb::Query.families` selects one face for `snap_weight` (`exact2/host-linux-text.rs:269-286`); shaping still receives one `Attrs.family` (`:289-302,380-399`). Cosmic-text’s later fallback searches the machine’s fonts and can select unrelated faces (`:209-216`); it does not continue through the authored list.

My “may be unimplementable” was too categorical: the capsule proves missing plumbing, not impossibility. Still, Weird Castle’s motivating wordmark is ASCII and provides no evidence for this scope (`exact2/weird-castle.app.contract:92-106`). Keep faces/families/stacks distinct, but make v1 stacks single-member and reject comma lists until Linux has a working cascade prototype.

## 4

Beyond the three established catches:

- A says exact1 allocated an ID when loading resolved (`panelist-A-grok.md:7`). It reserved the ID before starting/awaiting the asynchronous load (`exact1/assets-fonts-registry.ts:119-147`); only resolution began exposing that ID after `loaded` became true (`exact1/assets-fonts-state.ts:114-119`). The coherence diagnosis remains correct, but the timing claim is not.

- A’s Linux “families vector” claim is false for the reason in §3.

- A says LLP 1019 has no date (`panelist-A-grok.md:134`), but it is dated 2026-08-30 (`1019-fonts.rfc.md:7`). I repeated that mistake in round one and retract it. Only the implementer is absent, and the binding implementer rule applies to specs; 1019 remains an RFC (`exact2/RULES.md:58-60`).

## 5

Charlie should decide one thing first:

**Does Weird Castle replace Caltrain as the one application that defines v1?**

If yes, record the displaced work and authorize the thin bundled-font implementation in the same PR. If no, shelve LLP 1019 for now. Every technical font decision is downstream of that scope choice.
