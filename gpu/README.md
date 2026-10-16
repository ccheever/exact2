# GPU module ABI

The host and module ship together. Before 1.0, missing required symbols refuse
loading; there is no compatibility shim. See LLP 1009 and LLP 1014.000.

| Version / revision | Child delivery export | Contract |
|---|---|---|
| GPU ABI 1 / exact-gpu 0.1.0, before `126daba5` | `gpu_child` | Unnamed child pixels and geometry. |
| GPU ABI 1 / exact-gpu 0.1.0, from `126daba5` | `gpu_child_view` | **Breaking host ABI change:** adds UTF-8 `testId` and its byte length before geometry and pixel arguments, on native and Wasm. All embedders must rebuild against this revision. |

GPU tests use `exact_gpu::fixture::device_or_skip`: only `no adapter:` skips;
request-device, configuration, validation, and device-loss errors fail.

Windows rendered modules also export `gpu_window_frame` and `gpu_window_detach`;
rebuild the host and module together. A full-window canvas can present directly
from its module-owned GPU, composing premultiplied RGBA UI layers below and above
the game. The HWND and HINSTANCE remain valid until detach or module unload.
No GPU device or texture object crosses the DLL boundary. Transformed, clipped,
multiple-canvas and backdrop-dependent layouts use ordinary capture instead.
Canvas ID zero presents the complete CPU UI frame after the game is unmounted,
so menus continue through the same swapchain instead of switching back to GDI.
Capture reuses its render target and staging buffer until size, format or device
changes. The Windows host follows the current monitor's refresh rate.

For native performance measurements, set `EXACT_AGENT_WINDOW_FRAMES` (1–10000)
and optionally `EXACT_AGENT_WINDOW_TAP` to a launch button's test ID.
`EXACT_AGENT_WINDOW_FULLSCREEN=1` uses borderless fullscreen;
`EXACT_AGENT_WINDOW_UNPACED=1` removes pacing to measure rendering capacity.
These switches require a bounded development run and are stripped in production.
