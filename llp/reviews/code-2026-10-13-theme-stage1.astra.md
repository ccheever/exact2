# Code review: LLP 1121 stage 1 (theme and its reach), round 1, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `097903c5ad86423aad04882b8afb0938809179ed4a8c5fb11d43b1e2734184af`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 5 MAJOR, 11 MINOR; Grok 2 MAJOR, 4 MINOR).
- **Disposition:** all fixed in round 2 (0e81ae8b9)

---

**The changes need fixes before shipping.**

1. **MAJOR — Dynamic `AccentColor` defeats authored accents.** [host/web-js/src/style.rs:422](/private/tmp/rv-stage1/host/web-js/src/style.rs:422) inserts every entry into the exact-name map before excluding tint roles. With theme `AccentColor="#c8102e"`, a data-bound `"AccentColor"` becomes fixed red beneath `accent-color="#00f"`. The literal path retains the role and inherits blue. Apply the tint exclusion to both maps.

2. **MAJOR — Role-valued accent overrides also defeat authored accents.** [kernel/src/style/roles.rs:520](/private/tmp/rv-stage1/kernel/src/style/roles.rs:520) follows an override naming another role without preserving the nearest accent. `AccentColor="-exact-system-red"` therefore makes `color="AccentColor"` red beneath an authored blue accent. The preceding branch preserves precedence only for non-role values.

3. **MAJOR — The new accent serialization creates CSS self-references.** [kernel/src/style/roles.rs:514](/private/tmp/rv-stage1/kernel/src/style/roles.rs:514) now emits `var(--exact-accent, <theme colour>)`. Both [the static writer](/private/tmp/rv-stage1/host/web/src/css.rs:342) and [the JS writer](/private/tmp/rv-stage1/host/web-js/src/style.rs:676) recognize only the old `var(--exact-accent, AccentColor)` spelling. Consequently, `accent-color="AccentColor"` writes `--exact-accent:var(--exact-accent, <colour>)`, invalidating that property instead of inheriting it.

4. **MAJOR — Apple drops overrides for tint-following roles other than `AccentColor`.** [kernel/src/style/roles.rs:304](/private/tmp/rv-stage1/kernel/src/style/roles.rs:304) preserves `LinkText`, `ButtonText`, `link` and `symbol` whenever either Apple platform treats them as tint. However, [ThemeRoles.swift:53](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/ThemeRoles.swift:53) installs only the `AccentColor` override. For example, `LinkText="#f00"` still renders with native tint/link colour. The cross-platform OR also suppresses overrides on macOS for roles that only follow tint on iOS.

5. **MAJOR — Supported native defaults remain outside theme reach.** [PresenterIOS.swift:1373](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1373) and [PresenterMac.swift:1212](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/Mac/PresenterMac.swift:1212) retain their platform page backgrounds without consulting the theme. Thus `background="#f00"` leaves an unstyled page unchanged. Field placeholders likewise bypass a `placeholder` override at [NodeViewIOS.swift:931](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:931). These surfaces have supported colour setters and fall within D4’s reach ruling.

6. **MINOR — A second presenter with the same theme misses its tint.** [Session.swift:1054](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/Session.swift:1054) initializes root tint only when the shared table changes. A second iOS session receiving an identical table therefore keeps its platform tint. This occurs without conflicting themes; initialize each presenter when its boot table arrives.

7. **MINOR — Theme providers discard the view’s elevated appearance.** [ThemeRoles.swift:33](/private/tmp/rv-stage1/host/apple/Sources/ExactKit/ThemeRoles.swift:33) forwards only light/dark to `textChannels`. An override naming a dynamic background role consequently resolves a dark sheet using base-level colours. Forward the provider’s relevant traits, including `userInterfaceLevel`.

8. **MINOR — `GrayText` misses the default web stylesheet.** [kernel/src/style/roles.rs:324](/private/tmp/rv-stage1/kernel/src/style/roles.rs:324) emits `--exact-graytext`, while disabled controls in the active neutral stylesheet read [--exact-gray-text](/private/tmp/rv-stage1/host/web/neutral.css:77). The changes to `index.html` are overridden by that later stylesheet.

9. **MINOR — Capitalized app names resolve differently across targets.** [kernel/src/style/theme.rs:72](/private/tmp/rv-stage1/kernel/src/style/theme.rs:72) treats any capitalized or dashed name as a case-insensitive role. With `Brand="#f00"` followed by `brand="#00f"`, the kernel resolves `"brand"` to red; the JS exact-name map resolves blue. Preserve actual role identity instead of inferring it from spelling.

10. **MINOR — Duplicate spellings of one role are admitted.** [contract/syntax/src/parser/theme.rs:42](/private/tmp/rv-stage1/contract/syntax/src/parser/theme.rs:42) checks only identical names. `secondary-label="#f00"` plus `-exact-secondary-label="#00f"` passes, although D3 defines them as the same override. Native lookup selects the first; generated JS properties select the last. Reject duplicates after canonicalizing roles.

11. **MINOR — Reserved CSS keywords are accepted as names.** [contract/lower/src/theme.rs:108](/private/tmp/rv-stage1/contract/lower/src/theme.rs:108) rejects names that parse as colours, but permits `none`, `inherit`, `currentcolor`, etc. This violates D3 and causes observable divergence: a theme entry named `none` replaces a dynamic SVG `"none"` on JS, while the kernel retains `Paint::None`.

12. **MINOR — Declared ICC colours cannot be theme values.** [contract/lower/src/lib.rs:290](/private/tmp/rv-stage1/contract/lower/src/lib.rs:290) checks theme values before binding colour profiles at line 328. A valid `color(--brand …)` therefore fails inside a theme. Runtime binding also needs adjustment: [runner/src/bridge.rs:187](/private/tmp/rv-stage1/runner/src/bridge.rs:187) recognizes profiles only in the original string, so a theme name would not activate its underlying profile.

13. **MINOR — JS substitutes names inside deliberately unsupported functions.** [host/web-js/src/style.rs:443](/private/tmp/rv-stage1/host/web-js/src/style.rs:443) replaces tokens inside function arguments. A data string `light-dark(brand, #000)` becomes valid CSS on JS, while the kernel rejects it because theme names are not allowed inside `light-dark()` halves. The runtime map needs the same component boundaries as the kernel.

14. **MINOR — Cache validation still scans the theme on every string row.** [runner/src/bridge.rs:231](/private/tmp/rv-stage1/runner/src/bridge.rs:231) counts and compares every entry before returning the cached pointer, including for non-colour strings. This adds O(rows × theme size) work. The allocation test cannot detect it; cache the table with its owning plan and measure row-setting time as theme size increases.

15. **MINOR — The required colour lint integration is missing.** [contract/lower/src/warnings.rs:126](/private/tmp/rv-stage1/contract/lower/src/warnings.rs:126) does not pass theme declarations into `one_scheme`. Moving a fixed background colour into a named entry silences `lower-colour-one-scheme`. LLP 1095 §5 explicitly requires these lints to understand theme literals before landing.

16. **MINOR — Clearing overrides silently clashes with an existing session.** [kernel/src/style/roles.rs:284](/private/tmp/rv-stage1/kernel/src/style/roles.rs:284) reports replacement only when both tables are nonempty. Booting an unthemed runner clears another runner’s overrides without the promised journal entry. The accepted per-thread limitation still needs that diagnostic.

The parse-scope restoration and persistent Apple owner thread look consistent with the approved design. I found no separate defect in either `roles` batch decoder path. The added tests miss the accent cases above and the repeated-presenter case.

Both requested Cargo tests were attempted but failed before compilation because the read-only sandbox denied temporary target-directory creation. In-memory execution of the actual JS mapper template and accent guard reproduced the mapping/self-reference issues. `git diff --check` passed; files remain unchanged.

**VERDICT: fix**
