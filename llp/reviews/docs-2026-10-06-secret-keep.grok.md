# Round 1
I'll review the newest commit's two doc additions against the durable-state LLP and the store/secret implementations, and I won't change any files.The commit adds two short paragraphs. Next I’ll check each claim against the LLP and the store implementations.The grant text and the host paths are the parts most likely to be wrong. I'll read those next.There are no inaccuracies in those two additions.

Both paragraphs match the TypeScript store and the host behavior behind it.

- The grant is one `secret.keep <name>` line. `secret.keep signal.token` is a legal name (`[a-z0-9._-]{1,64}` in `grants/src/lib.rs`).
- The module API is `store.set(name, value)`, `store.get(name)` returning a string or `null`, and `store.forget(name)` (`contract/cli/src/typescript.rs`, `js/src/prelude.js`). Same-origin web code sees that `null` through the secret facade, not a missing `Map` value.
- Call them inside `answer`, including after `await`. That continuation is the host's `parse`, and the store is installed again for it (`js/src/lib.rs`). Background work has no store: `store.set` and `store.forget` fail (`js/src/door.rs`, `js/src/background.rs`).
- A real Apple launch uses the Keychain (`ibex2` `KeychainStore` / `SecItem`). A real web launch stores `exact.secret.<name>` in the page's `localStorage`, which any script on that origin can read (`host/web/glue.js`, `host/web-js/rt.js`). A real Linux launch keeps the value only in process memory and drops the write log, so it is gone when the process exits (`host/linux/src/picker.rs` `agent_secret_root` returns nothing unless `EXACT_AGENT` is set; `persist_agent_writes` then no-ops).
- A drive does not use those durable backends. With no `--storage` it stays in memory (the web glue also skips `localStorage`). With `--storage <name>` it is kept in that scratch store: files under the scratch tree on Apple and Linux, and that drive's page `localStorage` on the web.
