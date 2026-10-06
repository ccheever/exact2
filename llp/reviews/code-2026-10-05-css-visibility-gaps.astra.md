# Round 1
Found six defects in `a621bd2d7`:

1. **[P1] Focus resignation can re-enter an unfinished batch.** [CssVisibility.swift:42](/Users/admin/projects/exact2-wt-vis/host/apple/Sources/ExactKit/CssVisibility.swift:42) synchronously ends editing from `applyStyle`. Editing-end delegates call `commitEdit`, which invokes `onChange` immediately on both platforms; only `blur` uses the deferred event queue. For an edited input/textarea with a `change` handler, this recursively applies a newer batch before the original finishes, allowing remaining old operations to overwrite newer changes.

2. **[P2] Hidden inline ancestors block events from visible text.** [InlineText.swift:162](/Users/admin/projects/exact2-wt-vis/host/apple/Sources/ExactKit/InlineText.swift:162), and the equivalent stops at lines 171/186, break `visible outer link/handler → hidden span → visible text`. The leaf passes `inlineRunShows`, but lookup stops at the hidden intermediate span, losing the outer link, press, or hover handler. Visibility should reject the hidden hit leaf without cutting off [ancestor event propagation](https://dom.spec.whatwg.org/#dispatching-events).

3. **[P2] Removing an iOS material can blur a visible descendant.** The new [Backdrop.swift:130](/Users/admin/projects/exact2-wt-vis/host/apple/Sources/ExactKit/Backdrop.swift:130) guard triggers [NodeViewIOS.swift:1022](/Users/admin/projects/exact2-wt-vis/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1022), which removes the material before reparenting its children. A focused input explicitly computing `visibility: visible` inside a glass parent temporarily leaves the window and loses focus. This path neither preserves focus with `GlassGroups.moving` nor restores it afterward.

4. **[P2] Explicitly referenced hidden labels now become empty.** [Accessibility.swift:18](/Users/admin/projects/exact2-wt-vis/host/apple/Sources/ExactKit/Accessibility.swift:18) affects `labelledBy` and `accessibleDescription`, not just live announcements. A visible control referencing a hidden plain-text paragraph through `aria-labelledby` or `aria-describedby` now loses that text. [Accessible-name computation explicitly includes directly referenced hidden content](https://www.w3.org/TR/accname-1.2/#computation-steps).

5. **[P2] The tvOS focus fix remains ineffective for press targets.** [CssVisibility.swift:59](/Users/admin/projects/exact2-wt-vis/host/apple/Sources/ExactKit/CssVisibility.swift:59) requests an update, but [RemoteTVOS.swift:17](/Users/admin/projects/exact2-wt-vis/host/apple/Sources/ExactKit/IOS/RemoteTVOS.swift:17) still returns `true` for hidden nodes with `press` handlers. They remain eligible, `focusReturn` can choose them again, and `remoteSelect` still activates them. [Requesting a focus update does not itself make the current view ineligible](https://developer.apple.com/library/archive/documentation/General/Conceptual/AppleTV_PG/WorkingwiththeAppleTVRemote.html).

6. **[P2] macOS backdrop blur still paints while hidden.** [Backdrop.swift:197](/Users/admin/projects/exact2-wt-vis/host/apple/Sources/ExactKit/Backdrop.swift:197) computes blur without checking visibility. With `backdrop_blur > 0` and no `backgroundMaterial`, `updateMaterial` still installs/retains `layer.backgroundFilters`. The new `materialRequest` guard does not govern this macOS path.

The own-responder checks do not ordinarily blur a distinct visible descendant or repeatedly emit blur on unrelated style applications. The material teardown above is a separate problem.

Static review only; native tests were not run in the read-only environment. No files changed.
# Round 2
One concrete defect in `0b90bb016`:

- **[P2] Visible descendants still cannot activate hidden inline ancestors** — [InlineText.swift:96](/Users/admin/projects/exact2-wt-vis/host/apple/Sources/ExactKit/InlineText.swift:96). The new `!value.hidden` guard also applies through `activateInline`. On iOS, tapping visible text inside a hidden link resolves the hidden ancestor in `inlineActivationTarget`, then `activateInline` rejects it. macOS inline `press` handlers and ancestor hover handlers encounter the same rejection. Visibility must be checked on the original hit separately from the ancestor receiving the event.

Both `VisibilityMacTests` passed using the existing compiled bundle. The new test checks link resolution, but never activation, so it misses this regression. No files changed.
# Disposition

Round 1: the focus resign moved to the next main-queue turn; inline walks stop only at a hidden innermost run; the material gate and the `accessibleText` change were reverted (material teardown reparents children and frees a glass group; aria-labelledby reads hidden referenced text), both QUEUE; tvOS refuses hidden nodes. Round 2: `inlineEnabled` no longer refuses hidden runs (a visible run's click bubbles to a hidden container); the agent's tap by id refuses a hidden run instead. Left in QUEUE: hiding a Mac field or text area mid-edit ends the edit synchronously inside the style batch (`applyCssVisibility` sets `isHidden`, from e28279b3b), so its `change` applies a batch inside the batch.
