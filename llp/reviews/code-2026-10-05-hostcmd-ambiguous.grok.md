# Code review: ambiguous host-command calls refused, LLP 1089 D1 amended (58f264433..38b6fddf9), 2026-10-05 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, one fresh session with `--cwd` a detached worktree at `38b6fddf9`. `--always-approve` because plan mode cancels shell reads; the brief instructs read-only, and the worktree was clean afterwards.
- **Method:** the shared brief (sha256 `a601e783ad4fe7f0c8e492b6b249dd634fce0194b536697ece07121cb7b1eb26`); round 1; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all five fixed in `25a4539ca`:
  1. *Also checked as the host command.* After the checks, an error at an ambiguous call's span is dropped unless it is the ambiguity itself (span-matched, so it holds after lifting); the test compiles `close("swiped")` on a `close` prop and asserts the one id.
  2. *Navigation.* As astra #2.
  3. *§6 and D1's first bullet.* Both now say the amendment refuses the call.
  4. *The `close` comment.* Says binding stays legal and calling is refused.
  5. *The grammar's "Otherwise".* Now "When `name` is not a host command, it calls…".

---

I'll review this commit read-only against the approved ruling: statement calls that name both a host command and an in-scope action must be refused, with no self-wrapper exemption. First I'll read the review guidance and the diff.The commit is one change on top of `origin/main`. I'll read the full diff and the call-resolution path it touches, then check the ruling against every statement position.The refusal itself is in the right place: `ambiguous` walks every authored action body, including `if` and `match` arms, before `expand_file` and lifting, and only against that component. Bindings such as `press=close` and `press=setScheme("light")` stay legal, and another component's action of the same name does not count. These are the holes around that check.

1. **Should-fix** — `contract/types/src/actions.rs:188` (with `contract/types/src/checks.rs:1091`). An ambiguous call is still typechecked as the host command. `Viewer`'s `close: action` and `close("swiped")` inside `if` (the fixture LLP 1089 §4 renamed to `hide`) reports `syntax-call-ambiguous` and then `type-close` ("`close()` takes no arguments"). The second error tells the author to delete the argument, which leaves a call the new diagnostic says must be renamed. Same for `preventDefault`, `stopPropagation`, `share`, `postMessage`, and the other commands with their own argument checks. `calls.rs:726` only calls `close()` with no arguments, so the stack never shows up. Skip `check_command` for a statement `ambiguous` already reported (match the call span; after lifting, the statement no longer sits on the component that declares the prop).

2. **Should-fix** — `contract/cli/src/symbols.rs:243`. `contract symbols` prints `syntax-call-ambiguous` with the right id, message, and span, and no related location. The declaration span is thrown away when the error is built as a `TypeError` (`contract/types/src/lib.rs:1262`) and restored only by `authored_action_hint` (`contract/cli/src/symbols.rs:915`), which only `compile_file_output` calls (`contract/cli/src/lib.rs:673`). `contract build --json` and `contract lean` go through that hint, so their related location is filled in (`sources.resolve` then sets the file from `span.source_id`). Run the same hint in `symbols_json` before `sources.resolve`.

3. **Should-fix** — `llp/1089-action-composition.rfc.md:564`. §6 still says `type-call-ambiguous` was not taken because Caltrain's `setScheme` and files' `share` "compile today." After this commit that wrapper is `syntax-call-ambiguous`. D1's first bullet still says the host-command rule is "today's rule, unchanged" (`llp/1089-action-composition.rfc.md:87`), which is what leaves finding 1 typechecking the call as the command. Say the 2026-10-05 amendment refuses that call, and that §6's r1 rejection is superseded.

4. **Nit** — `contract/syntax/src/lib.rs:125`. The `close` entry still says an action prop of that name "is bound, never called," which is the old host-wins rule. Binding stays legal; the call is now refused. Say that.

5. **Nit** — `docs/contract-grammar.md:192`. The new refusal sentence sits between "is a host command when `name` is one" and "Otherwise it calls an action…", so "Otherwise" now reads as "when it was not refused." A plain `focus()` is then both the host command and an action call. Point "Otherwise" at "when `name` is not a host command."

Verdict: LAND WITH FIXES

## Round 2, 2026-10-05

- **Method:** grok as round 1, `--cwd` a detached worktree at `25a4539ca`; brief sha256 `8c7494f86b03dec9a4fba721ae0046665ba273494d70d78fed1f775dee618325` (the fixes `38b6fddf9..25a4539ca` against this review's round-1 findings and dispositions). Blind to astra's round 2.
- **Transcription:** stdout, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** both fixed in `2cdc57f87`:
  1. As astra's round-2 #1 and #2: `check_command` and `posts::check_targets` skip a recorded ambiguous call, matched by span; the retain is deleted, and the ambiguity is computed before `check_targets` runs.
  2. The `HOST_COMMANDS` header now says a same-named action, `action` prop or inject refuses the call.

---

I'll review the fixes read-only against the five round-1 findings and the full change from `58f264433` to `25a4539ca`. First I'll read the review guidance and both diffs.The fix commit is one change on top of the original refusal. I'll read both diffs and the call-resolution path they touch.The fix diff is small. I'll read the refusal, the error filter, and how symbols attach the declaration.Items 2 through 5 match the code. Item 1 only drops an error whose span is the call's name, so host-command diagnostics on the arguments still come out, and the surface check for `postMessage` is recorded first.

1. **Should-fix** — `contract/types/src/lib.rs:1303` (with `contract/types/src/checks.rs:836` and `contract/types/src/posts.rs:57`). The retain keeps any diagnostic that is not exactly the call name's span. A `share: action` prop with `share("nope")` in its component still reports `type-share-argument` on the string ("`share` takes named arguments: `share(title=…, text=…, url=…)`") beside `syntax-call-ambiguous`. The same is true of the argument-span checks for `postMessage`, `saveFile`, the file pickers, `scrollIntoView`, and `showNotification`. `posts::check_targets` runs at `contract/types/src/lib.rs:1259`, before the ambiguity is pushed at line 1262, and reports `type-post-message` on the surface string. `contract build` prints that with the ambiguity; `compile_path` and `contract symbols` return only that first error, so the declaration hint never runs. The retain also sits after `children` is captured at line 1281. Deleting a prefix `type-close` shifts a later child-span error from `check_root` under that cutoff, and `prefer_call_sites` (`contract/types/src/lib.rs:1416`) then keeps a consequence it would have hidden. The new test only compiles `close("swiped")`, whose `type-close` uses the name span (`contract/types/src/checks.rs:1091`). Skip `check_command` and `posts::check_targets` for a statement `ambiguous` already recorded, matching the call span, and delete the retain.

2. **Nit** — `contract/syntax/src/lib.rs:79`. The `HOST_COMMANDS` header still says a statement naming a command is the command, even inside an action of the same name. `action setScheme` / `setScheme(s)` is `syntax-call-ambiguous`. Say that a same-named action, action prop, or inject refuses the call.

Verdict: LAND WITH FIXES
