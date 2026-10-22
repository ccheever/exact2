# LLP 1140: Discord framework description

**Type:** Transcript
**Status:** Draft
**Systems:** Discord / libdiscord / gpui-native
**Author:** Unattributed; saved by Charlie Cheever
**Date:** 2026-10-21
**Revised:** 2026-10-21
**Related:** None

Saved from `discord-framework.description.md` on 2026-10-21. The original
text follows unchanged; its source and original date were not recorded.

## Transcript

**1. "How does the Rust code work together with native code? Can you show an example with a real screen? I guess they've already built some."**

Yes, about 45 screens are already written in Rust, including chat, channel list, DMs, profile, settings and voice. A good small example is the "Connecting…" banner. It's the Rust version of React Native's `GlobalStatusIndicator`, and it lives in `crates/wasm-app/src/screens/connection.rs` on the `cap/libdiscord-mobile` branch.

**Step 1: Rust decides what to show.** The screen watches the connection state and picks a banner:

```rust
fn wanted(network_online: bool, connection: Connection) -> Banner {
    match (network_online, connection) {
        (false, _) => Banner::NoConnection,                 // "No connection."
        (true, Connection::Connected) => Banner::Hidden,
        (true, Connection::Connecting | Connection::Offline) => Banner::Connecting, // "Connecting…"
    }
}
```

**Step 2: Rust describes the UI.** This works like JSX in React, but with Rust method calls (trimmed):

```rust
impl Render for ConnectionBanner {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .h(px(48.0))
            .justify_center()
            .items_center()
            .gap_2()
            .child(spinner().size_4())   // loading spinner
            .child(div().text_color(TEXT_MUTED).child("Connecting…"))
    }
}
```

**Step 3: Rust sends instructions, not pixels.** The framework compares this render with the previous one and sends only the changes to the native side. This is a simplified example:

```json
{"ops": [
  ["create", 812, "view",    {"direction": "row", "height": 48, "gap": 8}],
  ["create", 813, "spinner", {"width": 16, "height": 16}],
  ["create", 814, "text",    {"text": "Connecting…"}],
  ["children", 812, [813, 814]]
]}
```

**Step 4: The native host follows the instructions.** On Android, `UIManagerModule.kt` reads each instruction:

```kotlin
when (op.getString(0)) {
    "create"   -> create(op.getInt(1), op.getString(2), op.getJSONObject(3), lists)
    "update"   -> views[op.getInt(1)]?.let { v -> update(v, op.getJSONObject(2), lists) }
    "children" -> views[op.getInt(1)]?.let { v -> children(v, op.getJSONArray(2), lists) }
    "delete"   -> delete(op.getJSONArray(1))
}
```

Then `ViewKinds.kt` turns each view type into a real Android view:

```kotlin
"view"    -> GestureLinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
"text"    -> TextView(context).apply { typeface = Fonts.get(context, "gg sans", 400, false) }
"spinner" -> ProgressBar(context).apply { isIndeterminate = true }
```

iOS does the same in `UIManagerModule.swift`, but creates UIKit views (`UIStackView`, `UILabel`, `UIActivityIndicatorView`).

**Step 5: Events go back to Rust.** When the user taps a view, the host sends a message back to Rust:

```kotlin
host.emit(JSONObject().put("type", "ui").put("id", id).put("event", event).put("payload", payload))
```

Rust updates its state, renders again, and sends new instructions. Three seconds after the banner appears, for example, Rust sends instructions that replace the spinner with a static icon.

So the same Rust screen runs on both Android and iOS. Only the small host that creates the views is written separately for each platform.

[2:54 PM]
**2. "Why would this Rust/native version be faster than JS, if it still uses a shared framework in the middle?"**

Fair question. The basic setup is the same as React Native: logic on one side, native views on the other. The speed gain comes from the logic side:

- **Compiled code.** Rust runs as machine code. Hermes interprets JS bytecode and also has a garbage collector, which causes pauses.
- **Less work per update.** In the RN app, one store change re-runs many selectors and React re-renders component trees. In libdiscord, only the views that actually changed update, and only the changed rows are sent.
- **Faster startup.** There's no huge JS bundle to load and no hundreds of stores to set up.
- **Less memory**, which helps most on low-end Android.

The shared framework itself costs very little, because it's the same compiled code on every platform.

Caveats:
- When the app runs as a WASM module (needed for over-the-air updates), it's much slower: about 137–204 ms to start, versus 14 ms in the native build.
- Chat scrolling is currently slower than in an earlier version of the libdiscord app itself.
- I found no published head-to-head comparison with the current React Native app, so "faster than JS" is the goal, not something proven yet.

---

**3. "Is this a well-known Rust framework, or something Discord created?"**

Both:
- **Desktop** uses **GPUI**, an open-source framework made by the Zed code editor team. It draws everything on the GPU.
- **Mobile** uses **`gpui-native`**, which Discord wrote itself. It copies GPUI's API, so the code looks the same, but it doesn't use GPUI's code. Instead of drawing on the GPU, it creates native Android and iOS views.
- **The other parts** are standard open-source tools: wasmtime, Pulley and wasmi to run WASM, plus tokio, serde and SQLite.

The general idea isn't new; React Native works the same way. What's new is Discord's combination: GPUI's API, native mobile views, over-the-air updatable WASM, and one shared Rust core under every app.
