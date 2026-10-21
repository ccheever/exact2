# Code review: LLP 1121 stage 1 (theme and its reach), round 2, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `045867d097115785aeeed45234b02df07073217127abe1068357f9f2b5b62bc7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 4 MAJOR, 4 MINOR; Grok 2 MAJOR, 2 MINOR).
- **Disposition:** fixed in 81bcf1f10 but the Linux painter honouring an authored accent for text, recorded in QUEUE as pre-existing

---

The fixes are incomplete. Eight findings remain:

1. **MAJOR — Tint-role values can recurse indefinitely.** [kernel/src/style/roles.rs:367](/private/tmp/rv-stage1/kernel/src/style/roles.rs:367). The compiler accepts `AccentColor="LinkText"`. Resolving that accent reaches `LinkText`, which this function redirects to the accent’s override—`LinkText` again. `ColorValue::resolve` then recurses without termination. Cycle checking must include these implicit tint dependencies.

2. **MAJOR — Linux still ignores authored accents when painting text.** [kernel/src/style.rs:729](/private/tmp/rv-stage1/kernel/src/style.rs:729). With a red theme accent and an authored blue `accent-color`, `color="AccentColor"` still resolves red. [The text painter](/private/tmp/rv-stage1/host/linux/src/paint/inline.rs:21) supplies only the scheme, never the node’s inherited accent. Redirecting additional roles to the theme accent fixes their default colour but leaves Round 1’s precedence failure intact.

3. **MAJOR — The pointer cache can return a previous plan’s colours.** [runner/src/bridge.rs:238](/private/tmp/rv-stage1/runner/src/bridge.rs:238). The cache retains addresses without retaining their owning plan. After that plan is dropped, another plan can reuse both allocations and the same theme length, satisfying this comparison despite different entries. Cache identity needs an owning lifetime or unique generation.

4. **MAJOR — Placeholder reach remains incomplete.** [NodeViewIOS.swift:931](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:931) fixes single-line UIKit fields only. [UIKit text areas](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:222), [Mac fields](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:855), and [Mac text areas](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:187) still bypass the theme’s `placeholder` override.

5. **MINOR — Rejected boots still publish overrides.** [runner/src/runner.rs:809](/private/tmp/rv-stage1/runner/src/runner.rs:809). Publication still precedes fallible slot initialization, settlement and tree creation. A compiled fixture with `n=0` and `bad=1/n` fails initialization with `SlotType`, after replacing the table; no rollback restores the running session’s overrides.

6. **MINOR — Profile declarations are still missing at two theme readers.** [runner/src/bridge.rs:268](/private/tmp/rv-stage1/runner/src/bridge.rs:268) parses role overrides without binding `plan.profiles`; [the JS theme map](/private/tmp/rv-stage1/host/web-js/src/style.rs:415) does likewise. Declared ICC values now compile, but profile-valued overrides and dynamic JS names are silently omitted. The new scope in `set_plan_style` covers neither reader.

7. **MINOR — JS substitution still crosses component boundaries.** [host/web-js/src/style.rs:445](/private/tmp/rv-stage1/host/web-js/src/style.rs:445). Accepted theme names `top` and `inset` corrupt dynamic gradient directions and shadow syntax. Executing this mapper produced `linear-gradient(to #112233ff, #000, #fff)`. Its bounded parentheses pattern also still substitutes `brand` inside `light-dark(rgb(0 0 0 / calc(1)), brand)`.

8. **MINOR — The lint does not follow theme aliases or exact names.** [contract/lower/src/warnings.rs:925](/private/tmp/rv-stage1/contract/lower/src/warnings.rs:925). With `base="#fff"` and `card="base"`, a background naming `card` produces no warning: only `literal_colours("base")` runs. Case-insensitive lookup also conflates the now-distinct `Brand` and `brand`. Use the resolved table and its name-matching rules.

26 targeted tests passed using existing stage-1 binaries. Fresh builds and native UI runs were not performed. The worktree remains unchanged.

VERDICT: fix
