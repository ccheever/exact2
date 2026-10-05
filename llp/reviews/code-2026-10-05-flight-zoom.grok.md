# Code review: shared-element flights on Apple, the drawn image in points, the flight layer's rank, the enclosing presentation, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x20`, plain output.
- **Method:** one brief (sha256 `223be43c32fc33715ecb871aa002327f8fdf8120f37969b7288a35a2052187ee`), shared with astra. Round 1, blind to the other review. Reviewed the uncommitted diff in a detached worktree at `536a054e1`. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** nothing to take. The noted coverage gaps (an interrupted image flight through `showFlight`, real navigation/tab/modal containment, macOS execution, both ends inside one modal) are DEFERRED; the Signal Clone agent films cover the route/overlay case end to end.

---

The interpolation, the paint-order change, and the container choice are correct. **READY.**

No findings.

The three fixes do what they describe. Frame 0 and the last frame match the old formula, an interrupted flight still joins the current drawing, and the container rule keeps a flight inside one modal.

**Interpolation.** `flightImage` in `FlightsIOS.swift:292` and `FlightsMac.swift:222` lerps each end’s own drawn rectangle in points. At progress 0 that is `start × from.size`; at progress 1 it is `end × to.size`. Those are the same rectangles the old fraction-times-mixed-box formula produced at the two ends, so the endpoints do not move. In between, the image’s on-screen origin is the lerp of the two endpoints’ origins because the box origin and the in-box offset are each linear. For the reported cover `326×245` opening into contain `402×874` (natural `1000×750`), the heights are 245 and 301.5, and the midpoint height is 273.25 rather than the old bulge of about 377.

**Interrupted flights.** `beginFlight` still stores `flightLook.image` divided by the flying view’s current bounds (`FlightsIOS.swift:63`, `FlightsMac.swift:62`). The next flight’s `from` rect is that same bounds converted into the flight layer, so at progress 0 the new formula multiplies the fraction back out to the image rectangle that was on screen. The flight then moves in points from that rectangle to the new destination’s fit. An ancestor scale that made `from.size` differ from `bounds.size` would jump at the join; that was already true of the old progress-0 path, which also did `fraction × from.size`.

**macOS flip.** `flightImage` uses only `from.size` and `to.size`. `FlightLayer` and `NodeView` are flipped and their layers are geometry-flipped, while the content view is not, but converting a bounds rectangle between those layers keeps width and height positive. The fit fraction comes from `RasterGeometry.rect`, whose origin is the top of the box, and the image sublayer’s frame is in that same flipped layer space. The flip changes the converted origin, which this formula does not read.

**`setPaintForeground()`.** The default call stores the plane `Int64.max` (`PaintOrder.swift:174`). Dense ranks are index steps of 0.001 from zero, so this plane is always the last index. Adding it does not move any lower sibling, including a document plane at rank 1 plus 0.0005. `FlightLayer.hitTest` returns nil, and `hitOrder` (`PaintOrder.swift:141`) continues after nil, so the layer does not take clicks. `syncModal`’s front-to-back test (`Accessibility.swift:308`) only counts a `NodeView` or a view controller’s view, so the flight layer does not cover or uncover a modal. `dropEmptyLayer` (`FlightsIOS.swift:282`) removes the layer without clearing the plane. The association dies with the layer, and a stale `Int64.max` key on the parent does not change surviving siblings’ positions. The shadow map drops mirrors that the next capture does not see.

**Weak `root`.** A `weak` field in a struct is legal here, and copies are registered: releasing the view zeros the local copy, a copy stored in a class `let`, and a further copy. `liftFlight` reads it once while the presentation is on screen (`FlightsIOS.swift:188`). If it has already gone, the flight uses B’s root.

**`isDescendant(of:)`.** This matches the hierarchy the change is aimed at. A route under a navigation or tab controller stops `presentationRoot` at `RouteController.view` (`FlightsIOS.swift:278`). An overlay in the root document stops at the host controller’s view, which really does contain that route, so the flight is parented there and the `Int64.max` plane paints above the overlay. A flight whose both ends are inside one sheet or fullscreen modal captures the modal’s view, or a route inside it, and cannot be promoted past it: a presented controller’s `parent` is nil, so the walk stops. A page sheet or `.overFullScreen` presentation lives in a transition view that is a sibling of the presenting view, so a close from that modal down to a thumbnail behind it still flies in B, under the sheet. That behavior is unchanged, and it is the case this enclosure test does not claim to cover.

**Tests.** `testAFlyingImageMovesInPointsBetweenItsTwoDrawings` locks endpoint identity, linear width and height, and a spring step past the end. The two container tests fail if the flight always stays in B or always moves to A. The paint test fails if the layer is left at depth 0 under a document-plane sibling. They do not drive an image through `showFlight`, do not interrupt a flight, and do not place both ends inside one modal.
