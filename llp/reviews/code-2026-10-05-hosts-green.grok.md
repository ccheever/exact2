# Code review: exact-render and exact-linux green on main, 2026-10-05 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless; rounds at `892ebc452`, `26119ca13`, `d9db489f8`.
- **Method:** the shared briefs (sha256 `b5c9c97d5233facefea71f52025fe9a5d976232d92c64995278d2e5953ca8096`, `52d3d689c9545ea7039b11f9b29ddd8b650e717bbfed30a7d8b0b6355e119676`, `45a65593429b9199574b7b32cca7a0c28acb6b0997eb8d204be4566ec574f27d`); blind. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; opening sentences are progress narration.
- **Verdict:** round 1 LAND; round 2 LAND WITH FIXES; round 3 LAND.
- **Disposition (round 2), in `d9db489f8`:** taken as written — check the baked name first; absent only for no identity or `try_exists() == Ok(false)`; both arms pinned.

---

## Round 1

I'll review those four commits read-only against the diagnoses and the LLP constraints, starting with the review rules and the diff.The diff is small. Next I’ll check each diagnosis against the surrounding code and the LLP text it cites.**LAND**

No findings.

1. The diagnoses match the code, and each edit is the small one the failure requires. The interaction page scans the shell for the string `navigation.js` (`host/render/tests/it/main.rs:470`); the CSS comment at `host/web/index.html:80` is the only copy, and `navigation.js:238` still sets `data-exact-covered`. The clippy rewrite in `host/render/src/lib.rs` is the same branch. `witness_svg` is called only from `display_frame` (`host/linux/src/presenter/display_frame.rs:176`, `:225`), and the new cfg matches that.

2. LLP 1015.004 still refuses a missing shader by name. A rendered primary's `prepare_shaders` / `commit_shaders` failure still sets `Surfaces.error` (`host/linux/src/surfaces.rs:526-557`); `read_pack` names the shader (`host/linux/src/surfaces/shaders.rs:33`, `:52`). Headless open still skips shader text (`prepare_shaders` returns `Ok(None)` when `!rendered`). `Module::readback` still refuses `shader \`name\` has no source` (`gpu/src/lib.rs:1220`, `gpu/src/shaders.rs:139`). Those tests are unchanged. `agent::handle` still replaces the reply from that error (`host/linux/src/agent.rs:76`). A module that cannot open is only logged, which is what `a99d55103` did before `4e7078490` stored both failures in the same field. The pinned layout assertion is the same check, with the full reply in the failure text.

3. The direct-render skip is the existing guard, not a hidden gap. Lowering substitutes a component's props with the call's arguments (`contract/lower/src/lib.rs:7-10`), so `Row`'s `state a = length(map(big, y => y))` with `big=k` (`host/web-js/conformance/budget.contract:242-246`) is a `LoadResource` in the slot init. A settled tree can keep the placeholder while a checkpoint boot sees the answer (`host/render/src/lib.rs:146-154`, `:315-341`). Auto already stays on the kernel for that plan. The test allows every skip with that reason, which is the same rule `projects_as_booted` uses.

4. The uncontrolled-field gap should not block. `commit_text` reads `PropId::Value` (`host/linux/src/presenter/typing.rs:531-536`), so an unbound field commits `""`; the web sends `el.value` (`host/web/glue.js:555-557`). D4 says an unbound control holds its own state, and this host does that for checkboxes (`host/linux/src/presenter.rs:131-133`). That text behavior comes from `cbf1b3311`. This commit only moves the emoji fixture onto the controlled path D4's main rule describes (`value` + `input`, commit on Enter), which still shows a real field delivers `☕️` and the picker does not. The eager list is a compile refusal, `lower-reorder-collection` (`contract/lower/src/collection.rs:42-44`, from `425f97dab`); the disabled-grip runtime case remains.

5. No new check, no shim, and the comments state the constraint each change depends on.

## Round 2

I'll review the four commits as a round-2 pass: only what is still wrong after 26119ca13, read-only, against the named rules and LLPs.The range is five commits across eight files. I'll read the surface, direct-render, and typing paths against the LLPs those diagnoses cite.LAND WITH FIXES

1. [P2] Surface absence is wider than "no baked identity" and "no file" — `host/linux/src/surfaces.rs:527`

`present` is `Abi::file(...).is_ok_and(|path| path.exists())`. `file` (lines 49–55) resolves `current_exe` or `EXACT_NATIVE_LIBS` before the baked name, and every `Err` counts as absent. `Path::exists` is also false when `metadata` fails. A module that has a baked name, but whose directory env is unset, whose exe path cannot be read, or whose file cannot be stat'd, is only logged; `layout` still succeeds, and `android::load` and the shader pack never run. Check the baked name first. Treat the module as absent only for `Err("GPU module has no baked identity")` or `try_exists() == Ok(false)`. Every other `file` or open error, including `prepare_shaders` and `commit_shaders`, stays `self.error`. Pin both arms: a fixture module with a bad digest is the agent reply, and a missing file still returns the viewport.

The other diagnoses hold. The shell comment was the only `navigation.js` left in the interaction page after the preload is cut (`host/render/src/page.rs:81`). `budget.contract`'s lifted row initializer is the use-site resource read (`inline.rs` substitutes `big=k`), so `projects_as_booted` refuses and Auto writes the checkpoint boot; the allowlist matches that gate. The eager-list refusal is `3f8cbb165` (`lower-reorder-collection`); the disabled grip still runs. `witness_svg`'s cfg matches `display_frame`. With `EXACT_GPU_RENDER=1` and the module file present, a missing pack still becomes `shader \`{name}\` (shaders/{name}.wgsl): missing source` on `self.error`; the shader tests still assert that string. The unbound Linux field is a real LLP 1069.001 D4 gap in `commit_text` (`typing.rs:531`), from `cbf1b3311`, and this series does not touch that path. The QUEUE line is the right place for it; it should not block. No new check, script, or doc beyond that line. `surfaces.rs` stays under 1,500 lines.

## Round 3

I'll review only what is still wrong after the round-3 fixes, starting with the review rules and the four commits.The range is nine files. I'll read the surface-open path, the direct-render skip, and the LLP clauses those commits claim to follow.LAND

No findings.

The four diagnoses are right, and each edit is the smallest one that makes the host green.

1. The shell comment was the failure: an interaction page strips the `navigation.js` preload and the boot test rejects that string anywhere else. Rewording the comment keeps the check. `projects_as_booted` already refuses a plan whose slot initializer contains `LoadResource`. Props are inlined, so `budget.contract`'s `Row` state (`state a = length(map(big))`, `big=k`) is that case, and a checkpoint boot seeds the resource before `init_owned` while a settle can freeze the placeholder. Direct render returns the error and does not write a document; `same()` still fails a real mismatch. Admitting that skip does not hide a wrong page.

2. The eager list is a compile refusal since `3f8cbb165` (`lower-reorder-collection`); the disabled-grip runtime case is unchanged. The uncontrolled Linux field is a real LLP 1069.001 D4 break (`commit_text` and hardware edits read the `value` prop, so an unbound field commits `""`), but it comes from `cbf1b3311`, predates this series, and the QUEUE line is the right place for the typed-text map. It should not block.

3. LLP 1015.004 still holds. `prepare_shaders` names a missing source (`shader \`{name}\` (shaders/{name}.wgsl): missing source`) only after the module has opened, and `present` then stores that on `Surfaces.error`, which `agent::handle` returns. A missing identity or a `try_exists() == Ok(false)` file is logged and left Contract-painted. Headless `prepare_shaders` still returns `Ok(None)` without reading WGSL. The shader-pack tests still cover the named refusal.

4. `witness_svg` and its only caller, `display_frame`, share `cfg(any(linux, android, test))`.

No new check, script, or spec. The new QUEUE line is what `AGENTS.md` asks for. `surfaces.rs` is 1,378 lines.
