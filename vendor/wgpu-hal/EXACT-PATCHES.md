# wgpu-hal 30.0.1 — seven local patches: five in the Metal backend, two in Vulkan's

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

## 4. A word when the next submit has been scheduled

`src/metal/mod.rs`: `metal::Queue::on_next_submit_scheduled(f)` keeps `f`
for the queue's next `submit`, which adds it as a scheduled handler on the
last command buffer it commits — Metal calls it, on a thread of its own, once
that buffer is scheduled, which is after any `presentDrawable:` riding it has
been handed to the compositor (patch 2). With nothing to schedule it is
called at once. One submit consumes it.

`exact-gpu` uses it for a canvas's first frame: a presenter that reuses a
`CAMetalLayer` keeps it hidden until the new canvas's picture is with the
compositor (LLP 1068 §4.5, "a presentation signal before unhiding"), because
the layer still shows the last picture presented to it. Not proposed
upstream: wgpu has no presentation callbacks, and a general one would belong
on the surface.

## 5. An sRGB surface is color-matched on macOS

`src/metal/surface.rs`, `configure`: for `SurfaceColorSpace::Srgb` upstream
sets the layer's `colorspace` to nil, "the layer's default, which treats
contents as sRGB". That holds on iOS. On macOS a nil `CAMetalLayer.colorspace`
means no color matching: the values are shown as the display's own, so an
sRGB canvas is oversaturated on a P3 panel, its colours change with the
display and its preset, and so does every capture of it (Caltrain's macOS
canvas reference moved by R +3, G −3, B +1 at the bright end). On macOS
`Srgb` sets `kCGColorSpaceSRGB`; iOS is unchanged. The web shows a canvas as
sRGB, so this is the browser's picture too. **Upstreamable as it stands.**

## 6. A swapchain image's views and framebuffers outlive the frame (Vulkan)

`src/vulkan/mod.rs`, `src/vulkan/device.rs`, `src/vulkan/command.rs`,
`src/vulkan/adapter.rs`, `src/vulkan/swapchain/native.rs`.
wgpu-core makes a new texture for every acquired swapchain image, the GPU
module a new view of it every frame, and wgpu-hal caches framebuffers per
command encoder, emptied whenever the encoder is reset — so an animated
canvas made a `VkImageView` and a `VkFramebuffer` every frame and destroyed
both a few frames later. On PowerVR (Pixel 10) each framebuffer is a render
target the driver sets up at the first kick (`RGXAddRenderTarget`, ~15% of
the thread flushing the xheavy shader row) and tears down on its
`vkmem_free` thread (~80–100 ms/s at 120 Hz).

Now each swapchain image has one identity for the swapchain's life
(`NativeSwapchain::identities`), a view of it is made once per image and
view description and kept on the device (`DeviceShared::surface_views`;
`TextureView::cached`, which `destroy_texture_view` leaves alone), and a
render pass whose attachments are all such views uses the device's
framebuffer (`DeviceShared::surface_framebuffers`). Both go when the
swapchain's resources are released (`forget_surface_images`, after its
`vkDeviceWaitIdle`), or with the device. Nothing a caller can observe
changes but the count of views and framebuffers made. Upstreamable.

## 7. PRESENT is an ordered texture usage (Vulkan)

`src/vulkan/adapter.rs`: `get_ordered_texture_usages` includes `PRESENT`.
wgpu-core's submit leaves a drawable in PRESENT, and `Queue::present` asks
the device tracker for PRESENT again expecting no barrier ("If it's already
in PRESENT, this produces no barriers and we can skip the submission"); with
PRESENT not ordered the tracker emits PRESENT → PRESENT, and present
submits that barrier alone: a second submission, fence and kick per canvas
frame. Nothing on a queue touches an image in PRESENT, so the barrier is
never needed. Upstreamable (arguably a wgpu-core fix instead).

## Updating

Take the new archive whole, reapply the marked places (`git diff` against the
pristine archive is about 500 lines), and run `cargo test -p exact-gpu` on
macOS: `frame.rs` fails if a frame commits more than one buffer a canvas.
Patches 6 and 7 show only in an Android profile of an animated canvas (no
`RGXAddRenderTarget` or `vkmem_free` per frame, one submit per frame).
