# Render server buffers every static file with no size cap

**Status:** Closed
**Resolution:** Fixed unbounded buffering: assets above 16 MiB stream uncached with bounded hashing/copy buffers; background compression reads are also capped. Socket regression verifies 16 MiB+1 wasm bytes, length, HEAD, ETag/304 and ordinary glue. Existing bounded workers still serve socket I/O until timeout; no separate executor was added.
**Systems:** Render server
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1048.000

Rendered pages are refused above 16 MiB (`MAX_PAGE` in `host/render/src/serve.rs`). A static file is `std::fs::read` of the whole file into the worker that also renders pages, with no ceiling and no streaming. `static_file` serves anything under `dist` that is not a page, including `/.exact/` and large assets.

One request for a large wasm, video, or blob holds that worker until the read finishes and keeps the bytes until the response is written. The in-memory page cache is already bounded (64 locations, 32 MiB). Static responses are not.

Stream the file, or refuse above the page cap, and do not occupy a render worker for the copy. Done when a file larger than the page cap cannot pin a worker or grow the process by the file's size, and ordinary `glue.js` / `app.wasm` responses still succeed.
