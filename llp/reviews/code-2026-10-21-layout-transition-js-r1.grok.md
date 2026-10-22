# Code review: The web JS target writes a state-dependent -exact-layout-transition (GitHub #809), round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `fdb3caa8b4`.
- **Method:** one brief, blind to the other reviewer; 2 rounds (the standing rule for a host change), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review commit `fdb3caa8b4` against the kernel and the wasm host, read-only, and report defects as P1/P2/P3.The commit is the web JS layout-transition binding. I'll read the diff and compare it with the kernel and the wasm host.The diff is in. Next I’ll compare the new binding with the kernel’s CSS writer and the wasm host’s presence path.No P1. The headline path is right: a named curve or spring is written as the wasm host's custom property, `none` clears that property and still sets the paint bit, and a same-commit gain or loss matches `presence-glue.js`. Two P2s and one P3.

## P2 — A blank row is held and journaled as a refusal

`runner/src/instance.rs` clears a style before `set_dynamic` when `StyleValue::text_unsets` is true (`instance.rs:1043`), and it does not journal. That covers `""`, whitespace, and `unset` (`kernel/src/style.rs:340`). The live mask does not hold the row, so `host/web/src/document.rs:679` does not set bit 256.

`layout_transition` never asks `text_unsets`. It calls `set_dynamic`, and `Transitions::parse` accepts `""` and whitespace as `Transitions::NONE` and sets the mask (`host/web-js/src/style.rs:106`). Those strings go into the table (`style.rs:83`). The paint predicate is "key is in that table", so a blank value writes `260` (`host/web-js/src/paint.rs:203`). The node and its parent then isolate. The kernel does not.

Skipping `blank()` is the wrong side of that rule (`host/web-js/src/rows.rs:649`). The comment says the kernel refuses a blank. It clears one. A computed `""` or `"   "` is not in the table, so `layout_transition_map` (`style.rs:101`) journals `invalid layout-transition value …; unset (the JS target writes only the values the code names)` and returns null. `unset` fails `set_dynamic`, so it takes that same journal path. The runner clears both with no line.

`none` is a different value and is handled correctly: not an unset, mask on, no CSS, paint bit on.

The unit test locks the miss. `""` is one of `VALUES` (`layout_transition_tests.rs:16`), and the oracle is `set_dynamic` plus `mask.has` (`layout_transition_tests.rs:123`), so it expects `260` for a blank.

**Fix:** Treat `text_unsets` as a silent clear. Keep `blank()` on this row, and treat `/^\s*unset\s*$/i` the same way, before the invalid journal. Exclude those strings from the paint keys. Leave `none` in the table with a null declaration and the paint bit set.

## P2 — Every bound view is a style op on every commit

`lt` puts the element in `Bound` for its whole life (`host/web-js/rt.js:973`). `bound()` turns every connected one into `{op:"style", css:"--exact-layout-transition"}` (`rt.js:974`). `tail` passes that list to `Pres.before` (`rt.js:207`), and `Present` passes it again to `Pres.after` (`rt.js:986`), including from `settled()` (`rt.js:232`), which list and reorder also call.

Wasm does not. `glue.js` calls `before` with the real batch (`host/web/glue.js:605`), applies ops, then `after` (`glue.js:878`). A style op exists only when that node's style changed, and its `css` is the new declaration.

Two effects:

1. **Cost.** `presence-glue.js:284` measures every style op whose css contains the property, plus `tracked`. A `none` row has no declaration, so it is not tracked and wasm does not measure it. JS still measures every bound view, every commit, because the synthetic css is the property name. Reduced motion is the case this change is for, and it forces layout on all of those rows on every commit.

2. **Ghost parents.** `presence-glue.js:350` treats every style op as `restyled`. For a ghost parent that is itself bound, `after` (`presence-glue.js:352`) copies the inline `isolation`, which is already the forced `isolate`, over the restore value `paint.js` stored on `$ghostIsolation`. When the exit ends, the parent stays `isolation: isolate`. That is harmless while the row is actually held (`none` or a curve both set the policy bit). It sticks when the binding is present but the row is not held: blank, null, or a value outside the table. Wasm recaptures only when that node's style changed.

A move, reparent, or removal of a view that really has the declaration is fine: it is in `tracked`, `before` measures it, and `onEnd` drops it from `Bound` (`rt.js:973`). An exit still goes through `Leave` → `Pres.exit`, which `stop`s and untracks. The first commit before the module loads is also fine: `Pres` and `Present` are null, `pr` starts the load, and `createPresence` seeds `tracked` from the DOM. Wasm does not hold a layout-only batch either.

**Fix:** Emit a style op only when the declaration changes. `before` should measure a not-yet-tracked view only when this commit's value declares the property (the state is already updated; read the binding untracked). Views that already declare it are measured via `tracked`. `after` should get a style op only for a gain or a loss, so tracking updates and `restyled` matches a real wasm style write.

## P3 — The tests would not catch either bug

`each_named_value_writes_what_the_kernel_writes_and_a_computed_one_is_unset` checks the map against `css_text`, and that part is sound for named curves, springs, `none`, and `opacity 1s`. `the_paint_flag_is_set_where_the_kernel_holds_the_row` (`layout_transition_tests.rs:116`) uses the same `set_dynamic` oracle, so a fix for the blank mask would fail the test.

Nothing runs `before` / `after`. The browser test (`layout-transition.test.mjs:64`) covers slide, jump, same-commit gain, and same-commit loss, and it waits for `presenceLive` first. It does not cover a blank, a reparent, a removal, a ghost parent, or the reduced-motion measure. `presence-bound` only sees settled geometry, as the commit says.

Caps are fine and untested here: a literal past `MAX_TRANSITIONS` or the linear-stop cap fails `set_dynamic`, stays out of the table, and is unset, which is what the kernel does. The generated table is `serde_json`, so it is valid JS for these strings. Other rows still go through `blank()`; only this style id skips it.

**Fix:** Drive the paint bit from the runner's result (`unsets` then `set_dynamic`), including `""`, `"   "`, and `"unset"`. Add a presence test where a ghost parent has a bound row that is not held, and assert isolation is restored. Add a reduced-motion commit that asserts `none` rows are not measured.
