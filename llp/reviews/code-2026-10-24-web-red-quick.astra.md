# Quick review: web main-red fixes (x2/web-red, 7 commits), Astra (gpt-6-astra, max)

Five **RIGHT**, one **NIT**, one **WRONG**.

1. **NIT — `8bd14c61ad`** — [fonts-boot.test.mjs:110](/tmp/iosfix-rv-web1/host/web-js/fonts-boot.test.mjs:110). `bafdeea824` correctly explains the single `'400 700'` face. The test expectation is correct and minimal. Coverage nit: the fixture now exercises only one face; another file/style would preserve the original “load every face” coverage.

2. **RIGHT — `9cf5e85cf2`** — [build.mjs:586](/tmp/iosfix-rv-web1/host/web-js/build.mjs:586). `d1e75d73f5` introduced optional `source.tools`. Adding its name to the existing filter is the correct product fix; suppression remains restricted to generated glue.

3. **RIGHT — `16ef6d5b22`** — [build.test.mjs:157](/tmp/iosfix-rv-web1/host/web-js/build.test.mjs:157). `ac3af26807` made parsing necessary even without a backend override. Passing `false, parseSync` matches production configuration. Correct test repair.

4. **RIGHT — `e3b2afdb8b`** — [files.test.js:8](/tmp/iosfix-rv-web1/host/web-js/files.test.js:8). `3387fdd581` added the import. Supplying the real `liveById` repairs the mock’s interface without weakening behavior.

5. **RIGHT — `5e19858a48`** — [rt.js:445](/tmp/iosfix-rv-web1/host/web-js/rt.js:445), [ts-data.js:151](/tmp/iosfix-rv-web1/host/web-js/ts-data.js:151). `025471540c` caused the collision. All accesses reconcile: `ts-fetch.js` increments/decrements numeric `call.held` through `call[kind]`; `held.js` reads it; `reply` writes `call.wanted`; the module callback reads it. Ticket `t.held` remains unchanged. Both forwarded and held ceremony completions retain D11’s unwanted-response check, disposal, and `AbortError`.

6. **RIGHT — `f28c48ef26`** — [agent-bare.test.mjs:92](/tmp/iosfix-rv-web1/host/web/agent-bare.test.mjs:92). `416f51f1f4` moved the implementation. Correct test-only path update; deadline assertions remain intact.

7. **WRONG — `2495e962b5`** — [neutral.css:69](/tmp/iosfix-rv-web1/host/web/neutral.css:69). The reported row diagnosis is correct, and unconstrained compact buttons retain content-plus-insets sizing. However, the blanket change removes other content minimums:
   - Padded alert actions now override the less-specific `min-width: min-content` at [neutral.css:389](/tmp/iosfix-rv-web1/host/web/neutral.css:389), allowing clipped equal-width actions instead of wrapping.
   - `min-height: 0` permits padded buttons in a constrained flex column to shrink below their label height. That is an additional behavior change beyond the row fix. The automatic minimum applies along either main axis. [Flexbox §4.5](https://www.w3.org/TR/css-flexbox-1/#min-size-auto)

   Preserve those content minimums while narrowing the shrinking behavior.

Read-only validation: 5 files tests and 7 agent-bare tests passed, plus an in-memory counter/cancellation probe. CSS findings are from cascade/layout analysis; browser/build suites were not rerun. No files changed.

## Narrow re-check of the neutral.css fix (Astra)

**NIT — [host/web/neutral.css:65](/tmp/iosfix-rv-web2/host/web/neutral.css:65).** The comment should say alerts retain their existing minimum. Padded alerts compute to `auto`, matching `origin/main`: the nested rule’s specificity `(1,3,1)` beats both alert `min-content` selectors. [Nesting specificity](https://www.w3.org/TR/css-nesting-1/#nest-selector).

The behavior is otherwise **RIGHT**:

- Line 72’s `(1,4,2)` wins for `opt-b`, allowing its 58px share, while excluding both alert structures.
- `min-height: auto` preserves the constrained-column minimum.
- No additional cascade defect identified for bars, toolbars or sheet headers; authored minimums and sheet width overrides still win.

Static review only. Browser geometry wasn’t rerun; the selected stylesheet tests failed to load because `rolldown/utils` is missing. No files changed.

Taken: the neutral.css fix narrowed to the inline axis outside alerts (min-height stays auto), and its comment nit. fonts-boot's coverage nit (a second file or style) is left: the test now asserts the one-face behaviour bafdeea824 chose.
