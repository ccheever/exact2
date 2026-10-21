# PR #618 Landmarks recordings

- `pr618-scroll-return.mp4`: refreshed #618 merged with main `8aac2ca9538604062899ab64ac48a1415b94323c`, using a separate Landmarks test consumer, without the residual-env R implementation. iPhone 17 Pro simulator, iOS 26.5, 402 × 874 pt. Hero y=0, height=389 pt on home and detail. Native Back and toolbar remain. Home scroll offset is 226 pt before opening the card and 226 pt after returning; the two full-resolution screenshots have 0% changed pixels. The saved scenario replay passed 19 steps with no failures; two idle steps warned about small remaining pixel changes, and a separate measured push/pop explicitly settled.
- `swiftui-vs-r.mp4`: earlier experiment, SwiftUI on the left and R on the right. R commit `482632b7f1bf9dc23087ac16cc7cc5c048ac56f4`, based on the previous #618 head `7095262d9fbc43e3146419e413d1d1a8fdea2178`. Hero 389 pt in SwiftUI vs 379 pt in R; both retain home scroll 242 → 242 pt. R puts the badge overlay 34 pt too low. R is a separate experiment, not included in #618.

The fresh #618 clip omits 2.7 seconds of recorder setup and app restart, and is resized to 804 × 1748. Argent trims static pauses. The side-by-side comparison aligns stages from independent takes; gesture speed is unchanged. Neither clip is a frame-rate or timing benchmark. These recordings do not verify iPad, rotation, keyboard or other hosts.

This branch contains only review media; it is not intended to merge into main.
