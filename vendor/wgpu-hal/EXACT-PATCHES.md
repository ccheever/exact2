# wgpu-hal 30.0.1 — three local patches, all in the Metal backend

Complete crates.io archive, including the upstream MIT/Apache licenses and
`.cargo_vcs_info.json`. No feature change or dependency upgrade. Every changed
place is marked `EXACT (EXACT-PATCHES.md, n)` in the source.

- Upstream: https://github.com/gfx-rs/wgpu (`wgpu-hal/`)
- Exact release archive: https://static.crates.io/crates/wgpu-hal/wgpu-hal-30.0.1.crate
- Archive SHA256: b6b7fb58561a792bc237628ba0792e332de418fefe145f13b5ed8201e6d52f58
- Upstream VCS revision: 40f4a34ebaf56f9a046231f54125ad046239d3f3
- Implemented: 2026-09-30, the rest lane of the performance program; vendoring
  ruled by the orchestrator the same day (LLP 1009 D7).
- Only `exact-gpu` links this version (LLP 1009 D1); the 2D canvas's vello
  uses wgpu-hal 29 from crates.io, untouched.

## Why

A GPU canvas that animates (a shader row in a list) commits its frame 120
times a second. Through wgpu 30 on Metal one canvas frame was five or six
committed `MTLCommandBuffer`s where an `MTKView` draw commits one:

- wgpu-core opens a hal command encoding around every render pass for
  resource transitions (`"(wgpu internal) Pre Pass"`, wgpu-core
  `command/render.rs`), one at the front of each submitted encoder and one
  `"(wgpu internal) Present"` transition encoding at its end
  (`device/queue.rs`), and keeps a pending-writes encoding open. Metal has
  no transitions: all of them are empty. wgpu-hal made an `MTLCommandBuffer`
  at every `begin_encoding` and committed each one.
- `Queue::present` committed one more command buffer per surface texture.

Each committed buffer is a trip through Metal's submission queue into the
kernel and a completion notification back: on an iPad Pro M1, 43 ms/s of
submission and 23 ms/s of completion for one canvas, where UIKit's `MTKView`
costs 26 and 13 (2026-09-30, `~/bench/xheavy/rest/traces`).

## 1. An encoding that encodes nothing has no Metal command buffer

`src/metal/command.rs`, `src/metal/mod.rs`, `src/metal/device.rs`.
`begin_encoding` records the label and checks the outstanding-buffer limit;
the `MTLCommandBuffer` is made by the first encoder that needs one
(`CommandEncoder::ensure_cmd_buf`: a blit, render, compute or
acceleration-structure encoder). `end_encoding` returns a `CommandBuffer`
whose `raw` is `None` when nothing was encoded; `Queue::submit` commits only
the ones that have one and hangs its completion handler and signals on the
last of those (the extra "Signal" buffer is still made when none has one);
the residency-set code walks the same subset. `raw_command_buffer()` is
`None` until an encoder has been opened.

Nothing a caller can observe changes except the count of command buffers.
**Upstreamable as it stands** (QUEUE.md has the line).

## 2. A frame's presentations can ride its submit

`src/metal/mod.rs`, `src/metal/surface.rs`.
`metal::Queue::present_with_next_submit(true)` asks that queue's next
`submit` to put `presentDrawable:` for every surface texture its command
buffers draw into on the last command buffer it commits, as an `MTKView`
draw does. One submit consumes the request. A texture presented that way is
marked, and a later `Queue::present` of it commits nothing.

Opt-in, per submit, per queue, because it is wrong in general: a frame drawn
over several submits would be shown after the first. `exact-gpu` draws each
canvas's frame in one submit (LLP 1009 D7) and asks for it in `Module::flush`
unless a surface failed after recording (its drawable is in the submit and
must not be shown). It then lets the texture go rather than calling
`present`, because wgpu-core's `present` allocates a submission of its own,
which with nothing in it commits a buffer only to signal its fence.
Upstream would want this as an option on the surface configuration; it is
not proposed yet.

## 3. Counts, and a name for Cargo

`src/metal/mod.rs`: `metal::Queue::counts()` returns how many command buffers
the queue has committed and how many surface textures it presented with a
submit and alone — what `gpu/tests/it/frame.rs` holds patches 1 and 2 to
(one committed buffer a canvas frame; a failed canvas never presented, the
others once).

`Cargo.toml` and `build.rs`: the package has `links = "exact_wgpu_hal"` and
its build script prints `cargo::metadata=patches=2`, so `exact-gpu`'s build
script sees `DEP_EXACT_WGPU_HAL_PATCHES`. Cargo applies `[patch.crates-io]`
only from the workspace root: an app in a workspace of its own that lacks the
line would link the published crate. `gpu/build.rs` refuses that build and
prints the line to add.

## Updating

Take the new archive whole, reapply the marked places (`git diff` against the
pristine archive is about 250 lines), and run `cargo test -p exact-gpu` on
macOS: `frame.rs` fails if a frame commits more than one buffer a canvas.
