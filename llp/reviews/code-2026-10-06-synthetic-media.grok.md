# Round 1
I'll review the newest commit on `host/web/media-glue.js` and its bun test without changing anything, starting with the diff and the HTML media behavior the fix depends on.The commit is a small attach-time seek. I'll read the glue, its test, and the call sites that decide when `currentTime` is set.No findings.

The new branch in `installMedia` is the opening seek the glue already performs on `loadedmetadata`, done at attach when that event has already been scheduled. It matches the HTML seek algorithm and the wasm host path.

Setting `currentTime` at `readyState` 1 always starts a seek. The setter updates the official position and seeks whenever `readyState` is not `HAVE_NOTHING`, including when the new time equals the current one. `seeking` is set true before the setter returns, and `seeking`, `timeupdate`, and `seeked` are queued in that order. The only case that drops the events is an empty `seekable` range, which aborts in parallel before those tasks are queued. That is the same condition the existing `loadedmetadata` assignment already depends on, and a cached file with metadata has a duration and a seekable range.

`update()` runs first. If the bound time differs from `el.currentTime`, that assignment has already set `seeking`, so the new branch does not assign again. If the times are equal, `update()` skips on purpose and this branch performs the one seek. A `loadedmetadata` that is still queued does not seek a second time: at `readyState` 1, `state.seek` stays null, and `early` swallows the duplicate report. The branch does not add `seeking`, `timeupdate`, or `seeked` to `early`, so the timeout that clears those names does not drop the events this assignment queues.

The wasm path is unchanged. The branch sits inside `if (el.readyState)` and requires `readyState === 1`. Attaching at `readyState` 0 still stores `state.seek` and seeks from the `loadedmetadata` listener.

Audio uses the same `HTMLMediaElement` seek steps. The conformance `audio` does not bind `currentTime`, and `props.currentTime != null` keeps the branch off, which the new test covers. A bound time on an `audio` element takes the same path as `video`.

An invalid `currentTime` is already rejected in `update()` (`invalid-value`, no assignment). The new check, finite and `>= 0`, accepts the same set of values as that range (`0` through `Infinity`).

`paused` and `autoplay` are applied in `update()`, before this assignment. `paused="true"` calls `pause()` on an element that is already paused, which fires nothing, then the seek runs while paused. `paused="false"` calls `play()`; at `readyState` 1 that does not itself seek unless playback has ended, and a later seek does not reject the play promise. The synthetic plan starts at `paused=true` and has no `autoplay`.

`seeking === true` skips the assignment, so an in-flight seek is not aborted and restarted. Its queued `seeking` and `seeked` still reach the listener. A later `installMedia` is `update()` only: an unchanged `currentTime` is not assigned again, and a changed one seeks once through the existing path.

The new test locks the failure that shipped: `currentTime` `"0"` at `readyState` 1 assigns once, `loadedmetadata` is reported, and the follow-up `seeking` and `seeked` are not swallowed. The stand-in never sets `seeking` on assignment, and the test never uses a non-zero time, so it would not show that `update()` and this branch both assign when the times differ. On a real element the first assignment sets `seeking` and the second does not run.
