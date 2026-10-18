# wgpu-hal 30.0.1 — twelve local patches: five in Metal, seven in Vulkan

Complete crates.io archive, including the upstream MIT/Apache licenses and
`.cargo_vcs_info.json`. No dependency upgrade. Patch 11 offers an explicit native arithmetic-only f16 opt-in. Every changed
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
its build script prints `cargo::metadata=patches=12`, so `exact-gpu`'s build
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

## 8. A texture over an `AHardwareBuffer` (Vulkan, Android)

`src/vulkan/adapter.rs`, `src/vulkan/device.rs`. The device enables
`VK_ANDROID_external_memory_android_hardware_buffer` (and
`VK_EXT_queue_family_foreign`, which it requires) where the driver has them,
and `Device::texture_from_hardware_buffer` makes a texture whose memory is the
buffer's, as `texture_from_dmabuf_fd` does for a DMA-buf. A canvas renders
into buffers the platform's renderer draws as hardware bitmaps, in its own
pass, instead of presenting to a swapchain the compositor then samples
(Android: one `vkQueuePresentKHR`, one `dequeueBuffer` and one
`SurfaceTexture` update fewer per canvas frame). Such a texture is rendered
into every frame, so its views and framebuffers are kept as a swapchain
image's are (patch 6) and released in `destroy_texture`: without that
PowerVR made and tore down a render target per frame (`vkmem_free` 200 ms/s,
measured). Upstreamable.

The import honours `view_formats`: it sets `MUTABLE_FORMAT` and, where supported,
the image format list, as owned textures do. An sRGB view of an UNORM hardware
buffer lets a surface use hardware output encoding while the Android reader
continues sampling the same encoded bitmap. See the [Vulkan hardware-buffer
rules](https://docs.vulkan.org/spec/latest/chapters/memory.html#memory-external-android-hardware-buffer).

## 9. The queue at low global priority (Vulkan, Android)

`src/vulkan/adapter.rs`. Where the driver has `VK_EXT_global_priority`, the
device's queue asks for `VK_QUEUE_GLOBAL_PRIORITY_LOW_KHR`. A canvas's frame
is drawn on the screen by the platform's renderer (HWUI, patch 8's bitmap)
and then its compositor: when the canvas's own GPU work fills a frame, theirs
queues behind it and the frame they would show is missed. At low priority
theirs preempts ours. On a Pixel 5 (Adreno 620), a GPU-bound game canvas
showed 47.5–48.5 frames a second against 45.4 at the default priority.

## 10. Pipeline cache snapshots exclude creators (Vulkan)

`src/vulkan/mod.rs`, `src/vulkan/device.rs`. A per-cache read/write lock lets
render, compute and ray-tracing pipeline creation proceed concurrently, while
`pipeline_cache_get_data` takes exclusive access across Ash's size and data
queries. wgpu-core 30.0.1 forwards these calls without a cache lock. This makes
a snapshot stable even while specialization workers are creating pipelines.
The cache uses default flags; Vulkan internally synchronizes pipeline creation
in that mode. See [pipeline caches](https://docs.vulkan.org/refpages/latest/refpages/source/vkCreatePipelineCache.html)
and [cache data](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetPipelineCacheData.html).

## 11. Half-precision arithmetic without 16-bit uniform buffers (Vulkan)

`src/vulkan/adapter.rs`, `device.rs`, `f16.rs`, `instance.rs`, `mod.rs`, `build.rs`.
The [Vulkan storage feature rules](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDevice16BitStorageFeatures.html) distinguish arithmetic from buffer access.
Adreno 620 supports `shaderFloat16` and `storageBuffer16BitAccess`, but not
`uniformAndStorageBuffer16BitAccess`. `Instance::enable_f16_arithmetic()` opts
future adapters of that instance into granting native `SHADER_F16` with the
first two. Exact calls it only on Android, before requesting an adapter. Other
consumers retain WebGPU's full feature gate. Device creation enables only the
advertised storage bits. Metal and browser WebGPU are unchanged.

This is a deliberately narrower native guarantee than WebGPU's `shader-f16`:
16-bit uniform buffers are refused at pipeline translation on such adapters.
The recursive Naga type check covers nested structs, arrays and binding arrays;
it runs before Vulkan sees the module and gives a named linkage error. Half
immediate data is refused the same way (Naga's validator refuses it too), and
Naga polyfills unsupported f16 stage I/O with f32. Unsafe raw SPIR-V retains
its caller's Vulkan feature obligations.

Naga 30's SPIR-V `make_scalar` declares both 16-bit buffer capabilities even
for function-local f16. After proving the module has no 16-bit uniform data,
remove only its unsupported `UniformAndStorageBuffer16BitAccess` declaration.
Keep `Float16` and `StorageBuffer16BitAccess`; both are enabled. The result is
refused unless it still declares `Float16` for a half type and
`StorageBuffer16BitAccess` for half data in a storage buffer, and declares no
`StoragePushConstant16`, which the device never enables. The usual Naga
capability allowlist alone is insufficient: `make_scalar` inserts these directly.
Adapters with full uniform support keep their original SPIR-V unchanged.

Exact's forward shader buffers and stage interfaces are all f32/u32. The
renderer test translates both primitive/model paths, with/without early depth,
and checks those types; Castle Town has no custom WGSL hook. The Vulkan unit
tests translate private, workgroup, storage-buffer and stage-I/O half data
under the allowlist such an adapter builds (`f16::allow`, which `adapter.rs`
calls), and refuse uniform and immediate half data and a translation missing a
grant it needs (`cargo test --manifest-path vendor/wgpu-hal/Cargo.toml
--features vulkan --lib f16::tests`).
On the Pixel 5 (Adreno 620), Castle Town, a port of a Castle hub game kept outside
this repository, went from 87.5 to 88.7 fps in its heaviest view at 90 Hz.

This avoids another public feature bit and a fork of wgpu-core/types. It is an
Exact-native policy, not an upstreamable WebGPU feature grant. An upstream fix
should separate arithmetic from uniform storage and make Naga declare only
capabilities its output uses. `gpu/build.rs` requires the patch count in the
crate's metadata (now 12) on Android as well as Apple, and Android names the
direct HAL dependency so Cargo supplies that metadata.

## 12. A lost device's swapchain lets go of a held image (Vulkan)

`src/vulkan/swapchain/native.rs`. wgpu-core's `Surface::discard` checks the
device before it takes the acquired texture, so on a lost device a drawable
the application drops stays acquired, and the surface's drop then unconfigures
the swapchain with that texture alive. `release_resources` expected every
semaphore free and aborted the process ("Trying to destroy a
SwapchainAcquireSemaphore that is still in use by a SurfaceTexture"): on a
Pixel 5, about one cold start of Castle Town in nine, after the Adreno driver
reported GPU faults (`IOCTL_KGSL_GPU_COMMAND ... errno 71`) and the device was
lost. The semaphores a texture still holds are now left to it and logged; the
texture's drop makes no Vulkan call, so a few handles of a lost device are
never destroyed.

## Updating

Take the new archive whole, reapply the marked places (`git diff` against the
pristine archive is about 500 lines), and run `cargo test -p exact-gpu` on
macOS: `frame.rs` fails if a frame commits more than one buffer a canvas.
Patches 6 and 7 show only in an Android profile of an animated canvas (no
`RGXAddRenderTarget` or `vkmem_free` per frame, one submit per frame).

## Cargo workspace boundary

`Cargo.toml` declares an empty workspace so `cargo fmt --all` in a nested
Exact worktree does not attach this excluded path dependency to the enclosing
checkout. This is an Exact integration patch to retain when updating.
