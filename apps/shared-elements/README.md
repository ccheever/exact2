# Shared Elements

The LLP 1013.000 example: one screen per form of a shared-element flight.
A node's `sharedElement` names it; when a commit destroys the node holding a
name and creates another holding it, the new one flies from where the old one
was shown, on either one's `layout-transition`.

| Screen | Try | What it shows |
|---|---|---|
| Grid | Tap a photo; Close | A live image flies from its card into the viewer and back; its crop opens from `cover` to `contain`. |
| Grid | Close while it opens | The flight reverses from where it is. |
| Grid | `+3` twice, then Close | The grid scrolls to the photo's card before it lands there (D5). |
| List | Open a row, `+25`, Close | A virtualized row that was never built is built, scrolled into view and landed in (`scrollIntoView`). |
| Cards | Tap a card | A box with children grows into a detail: its surface flies, its children keep their places. |
| Tasks | Tap a task | The task flies between the two lists. |
| Zoom | Tap a photo | A fullscreen route zooms out of it on iOS 18 (UIKit's zoom, `navigationSource`). |

## Run it

```sh
bun host/web/dev.mjs --app shared-elements                  # web (CSS View Transitions)
bun host/apple/build.mjs --ios shared-elements-apple --run   # iOS simulator
bun host/apple/build.mjs shared-elements-apple --run         # macOS
cargo run --release -p shared-elements-linux                 # Linux: arrivers appear in place
```

Drive it with the agent's clock, for example:

```sh
bun scripts/agent.mjs ios --app shared-elements "tap open-p3" "clock +120" \
  "screenshot mid.png window" "clock settle" "tap close" "clock settle"
```

On iOS and macOS a flying view lives above the document, so take screenshots
with `window` to see it mid-flight.

## Per host

- **iOS, macOS:** the arriver is the live view, lifted above its route's
  content, interpolated between the leaver's rectangle and its own place on
  the engine's curve; interruptible; reduced motion lands at once.
- **Web:** CSS View Transitions (`startViewTransition`), the same pairs and
  curve; a new transition skips a running one.
- **Linux:** not drawn (stage 1); the journal says so once.
