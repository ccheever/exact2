# Code review: the host's data names reserved (contract/lower/src/dataset.rs), 2026-10-05 (grok)

- **Family:** xAI. `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd` a detached worktree at the reviewed commit; read-only by instruction.
- **Method:** one brief (sha256 `d0f3833ff0d30d425b76efb8cae6c4138b8f1b2860e9cf2420d2bdd07a8ba59c`), shared with astra. One round, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition:** Taken: `agent-view`, `app-src` and `seq` reserved too (80 words); a census of every `*.contract` under ~/projects, ~/.tuft/projects and /private/tmp found no authored use of any of the six. One round, as asked; not reviewed again.

---

The three added words are the right spellings, in order, and both test commands pass. The reserved list is still short of other names the web host writes.

## Findings

[P2] Reserve `app-src` — `contract/lower/src/dataset.rs:22`

The JavaScript host writes `data-app-src` on an image whose source is `app:/` (`host/web-js/src/rows.rs:560`, `host/web-js/symbols.js:60`). `HOST_WORDS` has no `app-src`, and `reserved` only treats `exact`, `bitmap`, and `symbol` as prefixes, so an app can still author `data-app-src` on that same image. `every_data_name_the_host_writes_is_a_reserved_word` does not see it: that test only walks `props_of`, and this name is applied later.

[P2] Reserve `agent-view` — `contract/lower/src/dataset.rs:22`

`host/web/glue.js:1181` and `host/web-js/agent.js:362` set `data-agent-view` on every live view during `axStamp`. That word is not reserved, so an app's `data-agent-view` is overwritten by the stamp and is indistinguishable from the view id.

[P3] Reserve `seq` — `contract/lower/src/dataset.rs:22`

`host/web-js/dev.mjs:220` writes `data-seq` on the dev checkpoint script. The sibling checkpoint attributes `activate`, `digest`, and `wasm` are already reserved.

## Assessment

`624d1b306` adds `context-preview`, `headedited`, and `shared-element` to `HOST_WORDS` and raises the length from 74 to 77. Those spellings match the attributes the host actually writes: `contextPreview` and `sharedElement` are explicit `data-context-preview` and `data-shared-element` (`host/web/src/element.rs:874`, `host/web/src/element.rs:795`); `headEdited` has no DOM name, so it falls through to `data-` plus the prop name in ASCII lowercase, `data-headedited` (`host/web/src/element.rs:946`). The array is sorted, has no duplicates, and the declared length is 77.

`cargo test -p exact-web --lib dataset_tests` passed (3 tests, including `every_data_name_the_host_writes_is_a_reserved_word`). `cargo test -p contract-lower --lib dataset` passed (2 tests). That web test only checks names `props_of` emits for non-SVG props, so a green run leaves the marker half of the list unchecked. SVG-only props stay off the list on purpose: the compiler sets them only on `fe*` and `filter`, which write the SVG attribute name.

**Verdict: READY WITH CHANGES**
