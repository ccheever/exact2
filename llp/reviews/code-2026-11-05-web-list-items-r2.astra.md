# Code review: web list item roles, round 2, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `949f2a0ec3` (one source commit on `99fd8ca92c`).
- **Method:** one brief (sha256 `0eaff63ce2b771c0f1fda904ba8dce5f3ded5458abe31bc8c73abf4065c9290b`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree and verification files.

Reviewed only `949f2a0ec39c43909ec9c6add8a59669d5f14c1f` against `99fd8ca92c`.

**1. Adversarial correctness**

1. **R1 settlement 1 — Reject as complete. P2: navigation still has physical-child assumptions.** The radio/tablist fixes work, but [document.js:135](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r2/host/web-js/document.js:135) and line 218 still inspect `p.children`. For `list role="tabpanel"`, production-source probes show the inactive panel losing server-rendered `visibility:hidden` and `inert`, and lazy opening selecting an unselected panel. [input-glue.js:1007](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r2/host/web/input-glue.js:1007) likewise reads the wrapper as the route container: the unwrapped control moves focus to the new route; the wrapped case makes no focus call. Use logical traversal here and sweep `titleOf` at line 989. Add compiled regressions for list-backed route containers/tabpanels covering server visibility, lazy opening and push/back focus, demonstrated failing before the fix.

2. **R1 settlement 2 — Accept.** Logical text-flow grouping and admission work in direct and nested source probes. Compiled coverage passes on all three variants; the settled negative controls reach the expected fragment failures.

3. **R1 settlement 3 — Accept.** `listRow` selects the real row through resize surfaces, and removal now lets presence stop resizing before unwrapping. Role-change and interrupted-exit regressions pass, with meaningful negative controls.

4. **R1 settlement 4 — Accept.** Grouped-card, empty-edge, press and sidebar selectors recognize wrappers. The compiled tests and isolated stylesheet/press controls support the settlement.

**New P2 — The fallback classifier skips browser-recognized roles.** [navigation.js:39](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r2/host/web/navigation.js:39) and [document.rs:1302](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r2/host/web/src/document.rs:1302) omit DPUB and Graphics roles recognized by [Chromium](https://chromium.googlesource.com/chromium/src/+/9dffde6c9419b8cf0caa061a4e2f192607916b40/third_party/blink/renderer/modules/accessibility/ax_object.cc). Production-helper probes show:

- Owner `role="doc-chapter list"` incorrectly produces a `listitem` wrapper.
- Row `role="doc-tip listitem"` incorrectly produces a `presentation` wrapper.
- Graphics equivalents fail identically.

The 82-role audit is accurate for its chosen vocabulary, but insufficient for browser fallback precedence. Correct both classifiers and add native-collector and Rust-renderer regressions for earlier recognized module roles on owners and rows, including removal controls.

**2. Simplification**

Accept the shared `listRow` and logical traversal approach. Complete its use in navigation consumers; no additional abstraction is needed.

**3. Writing**

Accept the corrected fit-source path, website guidance and commit-message structure. The prototype qualification is accurate. The claim that authored fallback roles take precedence remains broader than the implementation until the new finding is fixed.

**4. Verification**

Accept closure of the R1 wasm-adoption and full-test comparison gaps. The adoption probe preserves original links and local state. Full Apple-package controls reproduce the final worker-start failure on main and candidate.

Final results now show:

- Build, clippy, formatting, workspace, compiler checks, boot and semantics completed successfully.
- All 18 structure tests and the native accessibility collector passed.
- Ordinary-list conformance and both WebKit comparisons passed.
- Structure conformance reports 12 differences; the newly available main controls reproduce the underlying extra `route-saved` behavior.
- Caps has only the existing `foldables.md` violation.
- [The final broad web run](/tmp/exact-list-finalized-web.log) has ten modal-test compiler-prerequisite refusals. Those checks still need a successful rerun.

I ran read-only source/DOM probes, not browser or compiled-runtime tests. This snapshot lacks `target/` and `node_modules`. No files, GUI apps, Firefox or other agents were used.

**Verdict: not ready. Two P2 findings remain unresolved; this is not a clean round.**