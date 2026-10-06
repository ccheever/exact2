**Linux host follow-ups** (LLP 1015 landed 2026-08-29; r2 the same day: vello on
the GPU is the main painter, tiny-skia the fallback and pixel oracle; green on
`expo-build-1000` (CPU), the minisforum (Vulkan/llvmpipe), and headless on macOS
(Metal)). The DRM path ran on the minisforum 2026-08-29 (LLP 1015 §6: 1920×1080 @ 60 Hz on
RADV, boot to first flip ~160 ms), seen and driven over the host's own VNC server
(`EXACT_VNC=1`) since the KVM was unplugged; evdev itself has still carried no real
event. Owed, in order: **a KMS surface for the GPU**
(`VK_KHR_display`) so the frame is presented, not read back (17–19 ms of latency
per frame on Metal today); `canvas` on this painter's device; the Chrome
comparison of a pixel fixture (the font is pinned since 2026-08-29 — LLP 1015
§3 — and `smoke linux` is green on a Mac and on the builder); a font cache when a machine's scan (25 ms
for 787 faces on a Mac) matters; lifting the shared ~120 lines of orchestration
out of `host/apple` and `host/linux`; one wgpu when vello moves to 30.

*Filed under “Next, in order (2026-08-29)”, item 1.*
