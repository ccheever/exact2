# Round 1
No concrete defects found in HEAD `834ebbc2`.

- A seek started by `update()` sets `seeking` synchronously, so the new branch avoids double-seeking. A queued `loadedmetadata` finds `state.seek === null`; `early` suppresses duplicate metadata without swallowing the new seek events.
- Attaching before metadata preserves the wasm path. Audio uses the same semantics. Invalid times remain rejected, and the seek does not change paused/autoplay policy.
- An ongoing seek bypasses the new branch. Later installs return through `update()`, so unchanged bindings do not repeat it.

Setting `currentTime` at readyState 1 does **not always** produce `seeking`/`seeked`: HTML permits returning without those events when `seekable` is empty. That also applies to the existing metadata-triggered seek; it is not a regression introduced here. [HTML seeking algorithm](https://html.spec.whatwg.org/multipage/media.html#seeking)

All **7 focused Bun tests passed**, as did additional in-memory edge-case checks. Browser verification was blocked by the read-only sandbox’s temporary-file restriction. The Bun stand-in cannot establish actual browser event generation. No files changed.