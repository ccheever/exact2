# Code review: LLP 1122 T2b, the JS target, step 2 (the inspect table and row keys), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `bd83cb81f`.
- **Method:** one brief (sha256 `1be2a7b0e86d243b60dbb36766c5de46b074141f92911948f323ebe377575165`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (6 medium, 1 low).
- **Disposition:** taken in the most part.
  - 1: each row's value is the static tree's own, as the kernel resolved it, in each app tuple: one tree per tuple, so a relative length is resolved against its parent and root. Tested with `font-size=20 padding-left="1em"`. Limit: the root is 16 px; a pinned root font size is not applied to the table's lengths.
  - 2: `node`'s own rule decides what is literal (`vm::is_literal`). The page reports an expression's row as own only while it is set on the element (the page wrote it; `data-x-pin` for a pin). A class that is off, or a clear, falls through to what is inherited or initial.
  - 3: an expression's row reads the tokens its value names now: the custom properties in what the page wrote, mapped back by the table's `props`. The text is the token's name when it names one alone, else what was written. Limit: an expression mixing tokens with other text isn't given the runner's exact text.
  - 4: limit. A text style or heading chosen by an expression is not a style source the table knows.
  - 5: a row a control's start stops is in the site's starts even when its value is the initial one. The page checks starts before walking up. Tested: a button under `text-transform="uppercase"`, and the test checks no view above sets an initial row the table doesn't start.
  - 6: each region leaves the registry when its scope ends (`onEnd`). A region not yet connected is skipped, not dropped.
  - 7: in part. The Rust tests now cover relative lengths and control starts. The release check stays a subsequence check plus no dev words, because an equality check would need main's file. The runtime half is checked against the wasm page, node for node, by step 3's test.

---

1. **Medium — Literal values are serialized before kernel resolution.** [inspect.rs:107](/tmp/rv-dt-js2/host/web-js/src/inspect.rs:107)  
   For `text font-size=20 padding-left="1em"`, the table reports padding `16`; the runner reports `20`. `set_binding_style` creates a provisional value using the initial font size; the kernel resolves it during apply. Re-parsing each binding into an isolated patch discards that context. **Fix:** preserve relative-value metadata and resolve against the selected instance’s environment; use resolved kernel values for cases fully determined at build time.

2. **Medium — A dynamic binding does not necessarily own its row.** [inspect.rs:80](/tmp/rv-dt-js2/host/web-js/src/inspect.rs:80)  
   In `contract/corpus/styles.contract`, the inactive class supplies no padding. The runner clears those rows; the table unconditionally includes them as dynamic own rows. Expressions returning `inherit`, `unset`, or invalid values have the same problem. At mode boundaries, a cleared binding can instead receive a boundary-implied row. **Fix:** distinguish potentially bound rows from currently set rows and track successful set/clear state in development. Use the runner’s `vm::is_literal` for classification; its definition is broader than `style::literal`.

3. **Medium — Dynamic token provenance is discarded.** [inspect.rs:119](/tmp/rv-dt-js2/host/web-js/src/inspect.rs:119)  
   With `state c = "text.primary"` and `color=c`, `node` reports a dynamic row with token names and text `"text.primary"`. The table emits null `k` and `x`. Existing DOM bookkeeping retains mapped CSS such as `var(--x-text-primary)`, which cannot reproduce the original evaluated text exactly. **Fix:** retain the evaluated binding text and token attribution before CSS mapping, in development-only runtime metadata. Include inherited and boundary-implied consumers of that text.

4. **Medium — Platform text-style sources are frozen from an incomplete tree.** [inspect.rs:82](/tmp/rv-dt-js2/host/web-js/src/inspect.rs:82)  
   For `state chosen = "-exact-headline"` and `text font-size=chosen`, the static tree omits the size. Consequently, the table omits the text-style source for weight and leading, although the live runner reports `source:"style"` from that text node. Dynamic headings have this problem too; the existing `conformance/text-style.contract` exercises them. **Fix:** describe the dependencies that determine text-style sources and resolve them from current instance state, rather than treating `s` as unconditional.

5. **Medium — Initial-valued control resets lose their inheritance boundary.** [inspect.rs:90](/tmp/rv-dt-js2/host/web-js/src/inspect.rs:90)  
   Put an ordinary button beneath `text-transform="uppercase"`. The kernel’s control start resets transformation to `none`; `node` reports `initial`, including for its title. Because `none` equals the global initial value, this code drops the entry. The table’s prescribed nearest-own-ancestor lookup then reaches `uppercase`. **Fix:** encode which rows stop inheritance, even when their values equal the global initial value. A value comparison cannot replace the reset mask.

6. **Medium — The registry retains disposed regions and their rows.** [rt.js:1231](/tmp/rv-dt-js2/host/web-js/rt.js:1231)  
   Repeatedly opening and closing a conditional containing an `each` adds permanent strong references to its anchors, row objects and scopes. Cleanup occurs only when `$exactRowOf` reaches that entry; inspecting an earlier matching region can leave later dead entries untouched indefinitely. Connectivity also treats disposed content retained for an exit animation as live. **Fix:** unregister through `onEnd` on the owning scope and use scope lifetime for validity. The new test never exercises this lifecycle.

7. **Low — The tests do not establish the claimed parity.** [inspect.test.mjs:14](/tmp/rv-dt-js2/host/web-js/inspect.test.mjs:14), [inspect_tests.rs:97](/tmp/rv-dt-js2/host/web-js/src/inspect_tests.rs:97)  
   The release test would accept an empty stripped runtime: it checks only that surviving lines form a subsequence. The Rust tests inspect boot snapshots, omit row-key values from instance comparisons, skip dynamic values, and largely trust the runner-provided source when choosing assertions. Boundary-implied rows receive only a boundary-flag check. **Fix:** assert exact release equality and compare reconstructed reports across class changes, clears, dynamic tokens, text styles, multiple axes, and collection lifecycles.

The `project` extraction appears behavior-preserving: operation and projection order are unchanged. Type names, region-chain construction, tuple ordering and boundary detection agree with the runner on inspection. Matching connected single-element and bracketed rows appears sound across reorders; nested lazy regions still require adoption before their registry entries exist. Runtime overhead includes a registry entry per region, a field per row, and linear lookup scans; no timing evidence establishes its cost.

I verified in memory that stripped `rt.js` exactly equals `HEAD~1`—117,314 bytes. Both `--production` and plain non-dev builds strip it and exclude the inspect table. I found no release-path code change, but did not establish `app.js` or complete artifact equality. Builds and tests were not run because they write files.

**Verdict: Request changes—the release gating looks correct, but the table cannot yet reproduce `node` reliably, and the registry leaks disposed regions.**
