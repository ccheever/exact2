# Code review: the variant gallery on the JS dev loop, G3, round 3 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `3063f158ab5f69a2fa5bdb0970e4419e8560015cdc44ae92eeba6ee0ec8e905d`), shared by both reviewers, each blind to the other's review; HEAD `f3246883eb`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (3 SHOULD-FIX); round 2's closed.
- **Disposition:**
  Nothing blocks from either reviewer. The SHOULD-FIXes are taken post-round, without another round (noted in SIGNOFFS); the file's owner sees the dev.mjs diff first:
  - Astra 1 (inspect could restart the reader, or a Cargo fallback on the app's target): the table tool is the reader's binary only while fresh by its dep-info, asked without building anything; otherwise there's no table yet. The builder, with no fresh compiler of the loop's, runs its Cargo in the gallery's own target.
  - Astra 2 (an inspect table asked with no page open, after idle): the quiet period is armed first. Tested with a stand-in table process stopped after it.
  - Astra 3, Grok 1 (a retry built twice): a run takes up the failure before it (`failed` cleared at its start), so a page joining the retry adds none.
  - Grok 2 (inspect storage keys shared by the app's and the gallery's pages on one origin): inspect-dev.js keeps its keys under `install`'s `key`, and the gallery's client passes `exact.dev.inspect:gallery`. The app's client text is byte-identical.

No BLOCKER found. The `page: 'gallery'` wire field is correct. Three SHOULD-FIX issues remain.

1. **SHOULD-FIX — Gallery inspection can still restart the app’s design reader and contend for its Cargo target.** [host/web-js/dev.mjs:447](/tmp/rv-dt-g3r3/host/web-js/dev.mjs:447)  
   `tableTool()` calls `toolFresh()`, which calls `buildTool()` when compiler sources changed. Thus inspecting the gallery can restart the app’s reader. Its fallback, `webCompiler()`, can also return `cargo run`, which gallery inspection executes without a private target. The [gallery builder](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.mjs:108) likewise retains the app’s environment on fallback. Only the **contract generator’s** Cargo fallback is isolated. Use side-effect-free compiler selection and a gallery-owned target throughout.

2. **SHOULD-FIX — Inspection can start after idle cleanup without rearming cleanup.** [host/web-js/dev-gallery.mjs:206](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.mjs:206)  
   `/dev-inspect.json` returns before `ensure()` and never calls `quiet()`. After the gallery becomes idle, its dist remains and its table cache is cleared. Requesting this endpoint then starts a new inspection process with no open gallery stream and no idle timer. A slow or stuck process can outlive the quiet period indefinitely. Arm cleanup before inspection and cover this case with the recorded-PID test.

3. **SHOULD-FIX — Requests joining a retry still queue an unnecessary second build.** [host/web-js/dev-gallery.mjs:147](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.mjs:147)  
   `failed` remains set throughout a retry. After a failed initial build, reopening the page starts a retry; its waiting page’s SSE request sees `busy && failed` and sets `again`. Even if that retry succeeds, another complete build and reload follows. Distinguish the previous failure from demand arriving during the current attempt.

Round 2 disposition, using the supplied A/B numbering:

| Items | Status at HEAD |
|---|---|
| A1 | **Closed.** Notifications are deferred at [dev.mjs:499](/tmp/rv-dt-g3r3/host/web-js/dev.mjs:499), [575](/tmp/rv-dt-g3r3/host/web-js/dev.mjs:575), and [844](/tmp/rv-dt-g3r3/host/web-js/dev.mjs:844); reload notification follows publication at [612](/tmp/rv-dt-g3r3/host/web-js/dev.mjs:612). |
| A2, B2 | **Closed for inspection-process ownership and cache separation.** Inspection uses the gallery’s tracked `step()` and private table cache at [dev-gallery.mjs:163](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.mjs:163). Findings 1–2 identify remaining inspection paths. |
| A3, B4 | **Closed.** Refresh seeds the committed digest at [dev-gallery.mjs:89](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.mjs:89). |
| A4, B1 | **Closed for the first successful activation**, at [dev-gallery.mjs:147](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.mjs:147). Failed-build retries retain the related defect in finding 3. |
| A5 | **Closed for the contract fallback**, which uses its own target at [dev-gallery.mjs:94](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.mjs:94). Overall compiler isolation remains finding 1. |
| B3 | **Closed.** Direct contract execution checks depfile freshness at [dev-gallery.mjs:91](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.mjs:91). |
| A6, B5 | **Closed for the reported generator/builder coverage gap.** The new test records parents and children, exercising explicit stop and idle cleanup at [dev-gallery.test.mjs:197](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.test.mjs:197). |
| A7 | **Closed.** Headless HTML gets a base tag at [dev-gallery.mjs:235](/tmp/rv-dt-g3r3/host/web-js/dev-gallery.mjs:235). |

The remaining routing, stream separation, map retention, symlink checks, wasm 404, wire validation/listing, and 1.15 documentation look correct. Default app-client expansions compared byte-for-byte equal to `origin/main`.

Nothing was changed. No builds or test suites were run.
