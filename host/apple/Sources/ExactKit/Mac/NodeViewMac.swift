// One presenter identity per kernel node (LLP 1008 §5); inline text stays
// unmounted as its paragraph's run data (LLP 1033). Mounted NSViews draw
// backgrounds, borders, and text are drawn; frames come from the kernel's
// layout; transforms and opacity from presentation values. Everything a
// node reaches beyond itself — the text engine, the canvases, the web
// views, the session's clock — it reaches through its presenter's session
// (LLP 1031 D1), never a global.
#if os(macOS)
import AppKit

final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

/// A scroll container that chains: a wheel event it cannot consume in its
/// dominant direction — nothing to scroll, or already at that edge — goes
/// to the next responder, so an inner `scroll` node never traps the page.
/// The web's rule (`overscroll-behavior: auto`); AppKit's default is to
/// swallow it.
final class ChainingScrollView: NSScrollView {
    /// Points per line for a wheel without precise deltas — the browser's
    /// tick.
    static let lineHeight: CGFloat = 40
    /// Which axes scroll (the node's effective `overflow_x`/`overflow_y`).
    var scrollsX = true
    var scrollsY = true
    /// `overscroll-behavior` per axis: what happens to a gesture this view
    /// has run out of room for. `auto` hands the rest to the enclosing
    /// scroller (CSS scroll chaining); `contain` and `none` keep it here.
    var containX = false
    var containY = false
    /// Whether a contained axis shows the platform's overscroll affordance —
    /// the rubber band. `contain` does, `none` does not (CSS Overscroll §3).
    var bouncesX = false
    var bouncesY = false

    /// What a wheel event does here.
    ///
    /// The decision is named, and computed apart from acting on it, because
    /// the defects this code has had were routing choices and not drawn
    /// frames: a zero-delta lift dropped before AppKit could see it left a
    /// rubber band stretched forever, and nothing about that is visible in a
    /// screenshot or reachable by an agent's wheel. Named, it is a pure
    /// function of the event and the geometry, and `ExactKitTests` checks it
    /// without an animation, a clock, or a window.
    enum Routing: Equatable {
        /// Hand the gesture to `NSScrollView` — a contained axis, whose
        /// scrolling, momentum, and rubber band are AppKit's to run.
        case appKit
        /// Scroll this view by hand: an `auto` axis with room to move.
        case here
        /// Pass it to the enclosing scroller (CSS scroll chaining).
        case chain
        /// Nothing here wants it, and nothing else may have it.
        case drop
    }

    /// The geometry a routing decision reads. Passed in so a test can state
    /// one without building a window.
    struct Extent {
        var maxX: CGFloat
        var maxY: CGFloat
        var origin: CGPoint
    }

    var extent: Extent {
        let document = documentView?.frame.size ?? .zero
        let visible = contentView.bounds.size
        return Extent(maxX: max(0, document.width - visible.width),
                      maxY: max(0, document.height - visible.height),
                      origin: contentView.bounds.origin)
    }

    /// Where `dx`/`dy` goes. `phased` is whether the event carries a gesture
    /// phase — a lift or a momentum end, which have no delta at all.
    func routing(dx: CGFloat, dy: CGFloat, phased: Bool, in extent: Extent) -> Routing {
        // A gesture's phase transitions carry no delta — the lift that ends a
        // trackpad scroll is a zero-delta `.ended`, and momentum ends the
        // same way — and AppKit's elastic state machine needs them: without
        // the lift it never learns the gesture is over, so a stretched rubber
        // band stays stretched. A contained axis is AppKit's to drive, so it
        // sees every event of the gesture, delta or not. An `auto` axis is
        // driven here and has no use for them.
        if dx == 0 && dy == 0 {
            return (containX || containY) && phased ? .appKit : .drop
        }
        // The dominant axis decides who owns the event: a gesture is one thing.
        let vertical = abs(dy) >= abs(dx)
        let room = vertical ? (scrollsY && extent.maxY > 0) : (scrollsX && extent.maxX > 0)
        // A contained axis never hands a gesture to an ancestor. Contained
        // with nowhere to go, it stops here: passing it on is exactly what
        // `contain` forbids.
        //
        // Only a *gesture* goes to AppKit. A wheel with no phases is a mouse
        // wheel, and a mouse wheel does not rubber-band on this platform —
        // elastic overscroll is a trackpad's. Clamping one here rather than
        // handing it over keeps it synchronous, and synchronous is what makes
        // it drivable: `super.scrollWheel` scrolls over several frames, so an
        // agent that wheels and then reads the offset races the animation.
        // A phased gesture gets AppKit, its momentum, and its rubber band.
        if vertical ? containY : containX {
            if !room { return .drop }
            return phased ? .appKit : .here
        }
        // Per axis: can this view move in the delta's direction? (Flipped
        // document: origin grows as content scrolls up; a negative delta
        // scrolls content up.)
        return take(vertical ? dy : dx,
                    scrolls: vertical ? scrollsY : scrollsX,
                    limit: vertical ? extent.maxY : extent.maxX,
                    at: vertical ? extent.origin.y : extent.origin.x) ? .here : .chain
    }

    private func take(_ delta: CGFloat, scrolls: Bool, limit: CGFloat, at origin: CGFloat) -> Bool {
        scrolls && delta != 0 && limit > 0 && ((delta < 0 && origin < limit) || (delta > 0 && origin > 0))
    }

    override func scrollWheel(with event: NSEvent) {
        // Precise deltas (a trackpad) are in points; a wheel's are in lines.
        let precise = event.hasPreciseScrollingDeltas
        var dx = precise ? event.scrollingDeltaX : event.deltaX * ChainingScrollView.lineHeight
        var dy = precise ? event.scrollingDeltaY : event.deltaY * ChainingScrollView.lineHeight
        let phased = !(event.phase.isEmpty && event.momentumPhase.isEmpty)
        let extent = self.extent
        switch routing(dx: dx, dy: dy, phased: phased, in: extent) {
        case .drop:
            return
        case .appKit:
            super.scrollWheel(with: event)
        case .chain:
            nextResponder?.scrollWheel(with: event)
        case .here:
            // What this view can take of it, it takes itself — never through
            // AppKit, whose nested-scroll routing may move the enclosing view
            // or animate later, doubling a delta applied here.
            if !take(dx, scrolls: scrollsX, limit: extent.maxX, at: extent.origin.x) { dx = 0 }
            if !take(dy, scrolls: scrollsY, limit: extent.maxY, at: extent.origin.y) { dy = 0 }
            let target = NSPoint(x: min(max(extent.origin.x - dx, 0), extent.maxX),
                                 y: min(max(extent.origin.y - dy, 0), extent.maxY))
            contentView.scroll(to: target)
            reflectScrolledClipView(contentView)
        }
    }
}

final class NodeView: NSView, NSTextViewDelegate, NSTextFieldDelegate {
    override func selectAll(_ sender: Any?) { presenter?.selection.selectAll() }

    let id: UInt32
    let firstDraw: () -> Void
    let kind: String
    weak var textParent: NodeView?
    var textChildren: [NodeView] = []
    var cachedTextSpec: Spec?
    var cachedTextLayout: (width: CGFloat, paragraph: Paragraph)?
    var props: [String: String] = [:]
    var style: [String: Any] = [:]
    var handlers: Set<String> = []
    var translate = CGPoint.zero
    var scale: CGFloat = 1
    var rotate: CGFloat = 0
    weak var presenter: Presenter?
    var textArea: NSTextView?
    var textAreaScroll: NSScrollView?
    var field: NSTextField?
    var scroll: ChainingScrollView?
    /// The platform view returned by the dlopened iframe arm (@ref LLP 1020 D3).
    var web: NSView?
    /// A canvas node's Metal layer (LLP 1009).
    var metal: MetalView?
    /// A canvas's children live here (LLP 1014): laid out by the kernel in
    /// the canvas's box, over the Metal layer; when the surface samples them
    /// they are painted into its children texture and this view composites
    /// at alpha 0. `needsCapture`: painted again at the next capture;
    /// `paintedThisTurn`: a draw on this turn is the capture's own.
    var overlay: FlippedView?
    var needsCapture = false
    var paintedThisTurn = false
    /// Where a canvas's surface put this direct child (LLP 1014 D5): a 3×3
    /// homography, row major, from this node's own points to the canvas's,
    /// then its depth (larger nearer); `nil` is the kernel's frame.
    /// Hit-testing inverts it, nearest child first; accessibility reports the
    /// mapped box.
    var placement: [Double]?
    /// An image node's picture, once loaded (decoded off the main thread),
    /// the source it came from, and which load is current: a completion
    /// from an older load, or for a view that was destroyed, is dropped.
    var image: NSImage?
    var imageSource: String?
    var loadGeneration = 0
    var pressed = false
    var disabled: Bool { props["disabled"] == "true" }
    /// The pointer's tracking, for a `hover` handler (LLP 1005 §3).
    var tracking: NSTrackingArea?
    /// Images loaded since launch (smoke reporting).
    nonisolated(unsafe) static var imagesLoaded: [(String, CGSize)] = []
    /// The session's text engine (LLP 1031 D12: the catalog is the session's).
    var text: TextEngine? { presenter?.session?.text }
    var canvases: Canvases? { presenter?.session?.canvases }

    /// A node with focus, blur, or key handlers takes the focus (an input's
    /// field does by itself): the web's rule that only a focusable element
    /// hears these. A pressable is in the tab order the way a `<button>` is.
    override var acceptsFirstResponder: Bool {
        if disabled { return false }
        if field != nil || textArea != nil { return false }
        if isParagraph { return true }
        return handlers.contains("press") || !handlers.isDisjoint(with: ["focus", "blur", "key"])
    }
    /// Sequential focus follows the web: a button is in the loop even when
    /// macOS "Keyboard navigation" is off (that setting would otherwise
    /// skip every non-field).
    override var canBecomeKeyView: Bool { acceptsFirstResponder && !isHiddenOrHasHiddenAncestor }
    override func becomeFirstResponder() -> Bool {
        guard !disabled else { return false }
        let ok = super.becomeFirstResponder()
        if ok, handlers.contains("focus") { presenter?.focus(id) }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { presenter?.selection.clear() }
        if ok, handlers.contains("blur") { presenter?.blur(id) }
        return ok
    }
    override func drawFocusRingMask() {
        guard field == nil, handlers.contains("press") else { return }
        let radius = number("border_radius", number("border_radius_top_left"))
        NSBezierPath(roundedRect: bounds, xRadius: radius, yRadius: radius).fill()
    }
    /// A key down at a focused node, by the web's key name. Space and Enter
    /// on a pressable fire `press`, as they do on a `<button>`.
    override func keyDown(with event: NSEvent) {
        guard !disabled else { return }
        if isParagraph, window?.firstResponder === self, event.modifierFlags.contains(.command) {
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "a": presenter?.selection.selectAll(); return
            case "c": presenter?.selection.copy(); return
            default: break
            }
        }
        let name = NodeView.keyName(event)
        if handlers.contains("key") { presenter?.key(id, name) }
        if handlers.contains("press"), name == "Enter" || name == " " {
            presenter?.press(id)
            return
        }
        super.keyDown(with: event)
    }
    /// ⌘A while this node's field is being edited. The Edit menu is the
    /// usual path; this catches it when that item is disabled (a secure
    /// field) or when the event arrives at the window rather than the app.
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if isParagraph, window?.firstResponder === self, event.modifierFlags.contains(.command) {
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "a": presenter?.selection.selectAll(); return true
            case "c": presenter?.selection.copy(); return true
            default: break
            }
        }
        if let editor = textArea, NodeView.isCommandA(event), window?.firstResponder === editor {
            editor.selectAll(nil)
            return true
        }
        if let f = field, NodeView.isCommandA(event),
           window?.firstResponder === f || window?.firstResponder === f.currentEditor() {
            NodeView.selectAll(in: f)
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
    /// Command-A with no other chord, ignoring Caps Lock / function noise.
    static func isCommandA(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown else { return false }
        let mods = event.modifierFlags.intersection(.deviceIndependentFlagsMask).subtracting([.capsLock, .numericPad, .function])
        return mods == .command && event.charactersIgnoringModifiers?.lowercased() == "a"
    }
    /// Select the field's whole value. A secure editor can ignore `selectAll:`.
    static func selectAll(in f: NSTextField) {
        if let editor = f.currentEditor() {
            editor.selectAll(nil)
            if editor.selectedRange.length == 0 {
                let n = (editor.string as NSString).length
                if n > 0 { editor.selectedRange = NSRange(location: 0, length: n) }
            }
        } else {
            f.selectText(nil)
        }
    }
    /// The web's key names for AppKit's: the function keys by their names,
    /// the rest by the character typed.
    static func keyName(_ event: NSEvent) -> String {
        switch event.keyCode {
        case 36, 76: return "Enter"
        case 53: return "Escape"
        case 48: return "Tab"
        case 51: return "Backspace"
        case 117: return "Delete"
        case 126: return "ArrowUp"
        case 125: return "ArrowDown"
        case 123: return "ArrowLeft"
        case 124: return "ArrowRight"
        default: return event.charactersIgnoringModifiers ?? ""
        }
    }
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let t = tracking { removeTrackingArea(t); tracking = nil }
        if handlers.contains("hover") {
            let t = NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect], owner: self, userInfo: nil)
            addTrackingArea(t)
            tracking = t
        }
    }
    override func mouseEntered(with event: NSEvent) { presenter?.hover(self, true) }
    override func mouseExited(with event: NSEvent) { presenter?.hover(self, false) }
    /// The editing commands of a text field's editor as key names (the
    /// characters themselves are its `change`): Enter is taken here, so it
    /// does not end the editing as AppKit would.
    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        let name: String
        switch selector {
        case #selector(NSResponder.insertNewline(_:)):
            // Enter in an input with a `submit` handler is the web's implicit
            // submission; a `key` handler hears it as Enter as well.
            if handlers.contains("submit") { presenter?.submit(id) }
            name = "Enter"
        case #selector(NSResponder.cancelOperation(_:)): name = "Escape"
        case #selector(NSResponder.insertTab(_:)): name = "Tab"
        case #selector(NSResponder.moveUp(_:)): name = "ArrowUp"
        case #selector(NSResponder.moveDown(_:)): name = "ArrowDown"
        case #selector(NSResponder.moveLeft(_:)): name = "ArrowLeft"
        case #selector(NSResponder.moveRight(_:)): name = "ArrowRight"
        case #selector(NSResponder.deleteBackward(_:)): name = "Backspace"
        default: return false
        }
        if handlers.contains("key") { presenter?.key(id, name) }
        return name == "Enter"
    }
    func controlTextDidBeginEditing(_ obj: Notification) { if handlers.contains("focus") { presenter?.focus(id) } }
    func controlTextDidEndEditing(_ obj: Notification) { if handlers.contains("blur") { presenter?.blur(id) } }

    /// Where an image source resolves, as a page resolves `src`: an `http(s)`
    /// URL as is; a relative path under the asset root (`EXACT_ASSETS`, else
    /// the current directory) and never outside it; anything else (`file:`,
    /// `..` escaping the root) does not load.
    static func resolveSource(_ source: String, app: ExactApp?) -> URL? {
        if let u = URL(string: source), let scheme = u.scheme {
            return scheme == "http" || scheme == "https" ? u : nil
        }
        return app?.resolveAsset(source)
    }

    /// Decode an image completely, off the main thread: the bitmap and its
    /// pixel size, or nil when the data is not an image (or has no pixels).
    static func decode(_ data: Data) -> (CGImage, CGSize)? {
        guard let src = CGImageSourceCreateWithData(data as CFData, nil),
              let cg = CGImageSourceCreateImageAtIndex(src, 0, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary),
              cg.width > 0, cg.height > 0
        else { return nil }
        return (cg, CGSize(width: cg.width, height: cg.height))
    }

    /// Load the image off the main thread; on the main thread — if this is
    /// still the current load of a live view — keep it, tell the kernel its
    /// size, and repaint.
    func loadImage(_ source: String) {
        imageSource = source
        // The old picture (and its size in the kernel) stay until the new
        // one has loaded, as a browser keeps showing the old `src`.
        loadGeneration += 1
        let generation = loadGeneration
        guard let url = NodeView.resolveSource(source, app: presenter?.session?.app) else {
            image = nil
            FileHandle.standardError.write(Data("exact: image \(source) is not a loadable source\n".utf8))
            presenter?.intrinsic(id, nil)
            return
        }
        let id = self.id
        let pinned = presenter?.session?.app.resolver.isComplete == true && url.isFileURL
            ? presenter?.session?.app.assetBytes(source) : nil
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let loaded = (pinned ?? (try? Data(contentsOf: url))).flatMap(NodeView.decode)
            DispatchQueue.main.async {
                guard let self, self.loadGeneration == generation, let presenter = self.presenter, presenter.views[id] === self else { return }
                if let (cg, size) = loaded {
                    self.image = NSImage(cgImage: cg, size: size)
                    NodeView.imagesLoaded.append((source, size))
                    presenter.intrinsic(id, size)
                } else {
                    self.image = nil
                    FileHandle.standardError.write(Data("exact: image \(source) did not load\n".utf8))
                    presenter.intrinsic(id, nil)
                }
                // Only now, with the picture in hand. A picture arriving is a
                // repaint under a canvas (LLP 1014 D4 b) and it is not one
                // `draw(_:)` can report: the children live in an overlay at
                // alpha 0, which AppKit does not draw. Nor can
                // `repaintThrough` carry it — a decode that lands on the boot
                // capture's own turn is exactly what `paintedThisTurn`
                // suppresses. Ask the canvas for the next turn's capture
                // directly, or a first frame whose only change is a picture
                // keeps the capture taken before it decoded.
                self.needsDisplay = true
                if let c = self.canvasAbove { c.needsCapture = true; self.canvases?.scheduleCapture() }
            }
        }
    }

    /// The view is gone: no load in flight may report for it.
    func forget() {
        textParent?.textChildren.removeAll { $0 === self }
        textParent = nil
        textChildren.removeAll()
        invalidateText()
        loadGeneration += 1
        imageSource = nil
        image = nil
        presenter?.session?.webviews.destroy(id: id)
        web = nil
        presenter = nil
    }

    init(id: UInt32, kind: String, presenter: Presenter) {
        self.id = id
        firstDraw = presenter.session?.drawReceipt() ?? {}
        self.kind = kind
        self.presenter = presenter
        super.init(frame: .zero)
        wantsLayer = kind != "text"
        // A frame change during live resize repaints at the new width
        // instead of stretching stale pixels.
        layerContentsRedrawPolicy = .duringViewResize
        if kind == "canvas" {
            let m = MetalView(frame: .zero)
            addSubview(m)
            metal = m
            let o = FlippedView(frame: .zero)
            o.autoresizingMask = [.width, .height]
            addSubview(o)
            overlay = o
        }
        if kind == "textarea" { makeTextArea() }
        if kind == "input" {
            let f = makeField(secure: false)
            addSubview(f)
            field = f
        }
        if kind == "iframe", let w = presenter.session?.webviews.create(owner: self) {
            w.frame = bounds
            w.autoresizingMask = [.width, .height]
            w.wantsLayer = true
            addSubview(w)
            web = w
        }
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }

    /// Where children go: the scroll document view, or this view.
    var container: NSView { scroll?.documentView ?? overlay ?? self }

    /// The canvas this node is painted through, if any: the nearest canvas
    /// above whose overlay holds it.
    var canvasAbove: NodeView? {
        var v: NSView = self
        while let s = v.superview {
            if let c = s as? NodeView, c.overlay === v { return c }
            v = s
        }
        return nil
    }

    /// Something under a canvas repainted outside a batch (LLP 1014 D4 b, c):
    /// the canvas captures again on this run-loop turn — unless the draw is
    /// the capture's own, or follows a batch that already captured.
    func repaintThrough() {
        guard !Capture.capturing, let c = canvasAbove, !c.paintedThisTurn else { return }
        c.needsCapture = true
        canvases?.scheduleCapture()
    }

    @objc func clipScrolled() { repaintThrough(); presenter?.refreshVisibleText() }

    /// The direct child of a canvas this node is under, when that child is
    /// placed by the surface: the node whose `placement` maps this subtree.
    var placedAncestor: NodeView? {
        var v: NSView? = self
        while let n = v {
            if let node = n as? NodeView, node.placement != nil { return node }
            if let s = n.superview as? FlippedView, s.superview is NodeView, (s.superview as? NodeView)?.overlay === s { return nil }
            v = n.superview
        }
        return nil
    }

    /// A homography applied to a point (row major, projective).
    static func map(_ h: [Double], _ p: NSPoint) -> NSPoint {
        let w = h[6] * p.x + h[7] * p.y + h[8]
        guard abs(w) > 1e-9 else { return NSPoint(x: CGFloat.infinity, y: CGFloat.infinity) }
        return NSPoint(x: (h[0] * p.x + h[1] * p.y + h[2]) / w, y: (h[3] * p.x + h[4] * p.y + h[5]) / w)
    }

    /// The inverse of a 3×3 (row major), or nil when singular.
    static func invert(_ h: [Double]) -> [Double]? {
        let (a, b, c, d, e, f, g, hh, i) = (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8])
        let det = a * (e * i - f * hh) - b * (d * i - f * g) + c * (d * hh - e * g)
        guard abs(det) > 1e-12 else { return nil }
        let inv = [e * i - f * hh, c * hh - b * i, b * f - c * e,
                   f * g - d * i, a * i - c * g, c * d - a * f,
                   d * hh - e * g, b * g - a * hh, a * e - b * d]
        return inv.map { $0 / det }
    }

    /// A window point in this node's own coordinates — through the surface's
    /// placement when this node is under a placed child (LLP 1014 D5), else
    /// AppKit's own conversion.
    func local(_ windowPoint: NSPoint) -> NSPoint {
        guard let placed = placedAncestor, let h = placed.placement, let inv = NodeView.invert(h),
              let overlay = placed.superview, let canvas = overlay.superview as? NodeView else {
            return convert(windowPoint, from: nil)
        }
        let inCanvas = canvas.convert(windowPoint, from: nil)
        let inChild = NodeView.map(inv, inCanvas)
        // The child's own points; then down to this node by the untransformed
        // hierarchy.
        return convert(inChild, from: placed)
    }

    /// The placement changed: accessibility sees the new box.
    func placementChanged() {
        NSAccessibility.post(element: self, notification: .layoutChanged)
    }

    /// Hit-testing through the surface's placements (LLP 1014 D5): a canvas
    /// whose children are placed maps the point through each child's
    /// inverse, topmost first — straight from the canvas to the child,
    /// skipping the box AppKit would test. A placed child is only where the
    /// surface put it, never at its kernel frame: the rest of the overlay
    /// (children the surface left in place) is tested in AppKit's order
    /// without them, and then the canvas itself is the hit.
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let overlay, let sup = superview else { return super.hitTest(point) }
        let placed = overlay.subviews.compactMap { $0 as? NodeView }.filter { $0.placement != nil }
        guard !placed.isEmpty else { return super.hitTest(point) }
        let inCanvas = convert(point, from: sup)
        guard !isHidden, bounds.contains(inCanvas) else { return nil }
        // Nearest first: what is seen on top is what a tap reaches.
        for child in placed.sorted(by: { ($0.placement?[9] ?? 0) > ($1.placement?[9] ?? 0) }) {
            guard let h = child.placement, let inv = NodeView.invert(h) else { continue }
            let p = NodeView.map(inv, inCanvas)
            guard child.bounds.contains(p) else { continue }
            // Into the child's superview's space, where AppKit expects it.
            let inOverlay = NSPoint(x: child.frame.minX + p.x, y: child.frame.minY + p.y)
            if let hit = child.hitTest(inOverlay) { return hit }
        }
        let inOverlay = overlay.convert(inCanvas, from: self)
        for child in overlay.subviews.reversed() where (child as? NodeView)?.placement == nil {
            if let hit = child.hitTest(inOverlay) { return hit }
        }
        return self
    }

    /// The box on screen, through the placement of the placed child this
    /// node is (or is under), for assistive technology — the same box the
    /// agent's `layout` reports.
    override func accessibilityFrame() -> NSRect {
        guard let placed = placedAncestor, let h = placed.placement, let overlay = placed.superview, let canvas = overlay.superview as? NodeView, let win = window else { return super.accessibilityFrame() }
        let corners = [NSPoint(x: 0, y: 0), NSPoint(x: bounds.width, y: 0), NSPoint(x: bounds.width, y: bounds.height), NSPoint(x: 0, y: bounds.height)].map { NodeView.map(h, placed.convert($0, from: self)) }
        let xs = corners.map { $0.x }, ys = corners.map { $0.y }
        let inCanvas = NSRect(x: xs.min()!, y: ys.min()!, width: xs.max()! - xs.min()!, height: ys.max()! - ys.min()!)
        return win.convertToScreen(canvas.convert(inCanvas, to: nil))
    }

    /// Whether this view draws in the dark appearance. The **owning view's**
    /// appearance, not `NSAppearance.currentDrawing()`: `currentDrawing()`
    /// names whatever is drawing at that instant, and colours are not all
    /// applied inside a draw — a field's `textColor` is assigned in
    /// `applyStyle`. @ref LLP 1034 D2
    var drawsDark: Bool {
        effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
    }

    /// The four channels a colour row carries, resolved. A fixed colour is
    /// the four; a `light-dark()` pair is two fours and this picks one
    /// (LLP 1034 D1). Anything else is not a colour.
    func channels(_ key: String, dark: Bool? = nil) -> [Double]? {
        let night = dark ?? drawsDark
        switch style[key] {
        case let c as [Double] where c.count == 4: return c
        case let pair as [[Double]] where pair.count == 2:
            let half = night ? pair[1] : pair[0]
            return half.count == 4 ? half : nil
        default: return nil
        }
    }

    /// Whether any colour on this node is a pair — what says an appearance
    /// change is something to this view rather than nothing.
    var hasSchemeColor: Bool {
        style.values.contains { ($0 as? [[Double]])?.count == 2 }
    }

    func color(_ key: String, _ fallback: NSColor) -> NSColor {
        guard let c = channels(key) else { return fallback }
        return NSColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
    }

    /// The appearance changed under this view. A repaint is not enough: the
    /// text engine caches a paragraph spec and a laid-out paragraph, and a
    /// `Run` carries a concrete colour, so ink from the previous appearance
    /// would survive a redisplay. Inline text nodes are not in the native
    /// hierarchy, so the paragraph that owns them is invalidated too, and
    /// the colours assigned outside a draw are re-applied. @ref LLP 1034 D2
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        guard hasSchemeColor || textChildren.contains(where: { $0.hasSchemeColor }) else { return }
        paragraphOwner.invalidateText()
        paragraphOwner.needsDisplay = true
        applyStyle(style)
        needsDisplay = true
    }
    func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
        if let n = style[key] as? Double { return CGFloat(n) }
        return fallback
    }

    /// The field for an input: `NSSecureTextField` for `type="password"`
    /// (the web's masking), a plain one otherwise; the same delegate,
    /// borderless, the node paints its own box.
    func makeField(secure: Bool) -> NSTextField {
        let f = secure ? NSSecureTextField(frame: .zero) : NSTextField(frame: .zero)
        f.isBordered = false
        f.isBezeled = false
        f.drawsBackground = false
        f.backgroundColor = .clear
        (f.cell as? NSTextFieldCell)?.drawsBackground = false
        f.focusRingType = .none
        f.isEditable = true
        f.isSelectable = true
        f.delegate = self
        f.cell?.isScrollable = true
        f.cell?.wraps = false
        f.cell?.usesSingleLineMode = true
        return f
    }

    /// The input's content box: padding and border sit on the node, the
    /// field is the text inside — CSS's rule, so a placeholder lines up
    /// with a native one.
    func fieldBox() -> NSRect {
        let uniform = number("border_width")
        return bounds.insetBy(
            left: number("border_width_left", uniform) + number("padding_left"),
            top: number("border_width_top", uniform) + number("padding_top"),
            right: number("border_width_right", uniform) + number("padding_right"),
            bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
    }

    func applyPlaceholder(_ f: NSTextField) {
        let text = props["placeholder"] ?? ""
        let font = f.font ?? NSFont.systemFont(ofSize: 17)
        if text.isEmpty {
            f.placeholderAttributedString = nil
            f.placeholderString = nil
            return
        }
        // Not `placeholderTextColor`: that tracks the window's appearance, so
        // a white field in a dark app (the night) paints a light placeholder
        // and it vanishes. Mute this field's text color — the web's
        // `input::placeholder` (`#3c3c434c` on black type).
        let ink = (f.textColor ?? NSColor(srgbRed: 0, green: 0, blue: 0, alpha: 1)).withAlphaComponent(0.30)
        f.placeholderAttributedString = NSAttributedString(string: text, attributes: [
            .font: font,
            .foregroundColor: ink,
        ])
    }

    func applyProps(set: [String: String], clear: [String]) {
        for k in clear { props.removeValue(forKey: k) }
        for (k, v) in set { props[k] = v }
        applyTextArea()
        if let f = field {
            // `type` changed between password and text: a secure field is a
            // different class on AppKit, so the field is remade in place.
            let secure = props["type"] == "password"
            if (f is NSSecureTextField) != secure {
                let n = makeField(secure: secure)
                n.frame = f.frame
                n.stringValue = f.stringValue
                n.font = f.font
                n.textColor = f.textColor
                f.removeFromSuperview()
                addSubview(n)
                field = n
            }
        }
        if let f = field {
            if let v = props["value"], f.stringValue != v { f.stringValue = v }
            applyPlaceholder(f)
            f.isEnabled = !disabled
            f.isEditable = !disabled && props["editable"] != "false"
        }
        setAccessibilityEnabled(!disabled)
        setAccessibilityIdentifier(props["testId"])
        setAccessibilityLabel(props["accessibilityLabel"])
        if kind == "image", let src = props["imageSource"], src != imageSource { loadImage(src) }
        if kind == "image", props["imageSource"] == nil, imageSource != nil { imageSource = nil; image = nil; presenter?.intrinsic(id, nil) }
        if kind == "iframe" { presenter?.session?.webviews.update(self) }
        needsDisplay = true
    }

    func applyStyle(_ s: [String: Any]) {
        style = s
        // Scrolling and clipping come from the effective overflow the host
        // wrote in (never from the node's kind): `scroll` on an axis makes a
        // scroll container that scrolls that axis; `hidden` clips.
        let ox = s["overflow_x"] as? String ?? "visible", oy = s["overflow_y"] as? String ?? "visible"
        if (ox == "scroll" || oy == "scroll") && scroll == nil {
            let sv = ChainingScrollView(frame: bounds)
            sv.drawsBackground = false
            sv.scrollerStyle = .overlay
            sv.hasVerticalScroller = true
            sv.hasHorizontalScroller = true
            sv.autohidesScrollers = true
            sv.automaticallyAdjustsContentInsets = false
            sv.contentInsets = NSEdgeInsetsZero
            sv.documentView = FlippedView(frame: .zero)
            // A scroll under a canvas repaints it (LLP 1014 D4 c).
            sv.contentView.postsBoundsChangedNotifications = true
            NotificationCenter.default.addObserver(self, selector: #selector(clipScrolled), name: NSView.boundsDidChangeNotification, object: sv.contentView)
            sv.autoresizingMask = [.width, .height]
            for child in subviews where child is NodeView { child.removeFromSuperview(); sv.documentView?.addSubview(child) }
            addSubview(sv)
            scroll = sv
        }
        if ox != "scroll" && oy != "scroll", let sv = scroll {
            // Neither axis scrolls any more: the children come back out.
            for child in sv.documentView?.subviews ?? [] where child is NodeView { child.removeFromSuperview(); addSubview(child) }
            sv.removeFromSuperview()
            scroll = nil
        }
        scroll?.scrollsX = ox == "scroll"
        scroll?.scrollsY = oy == "scroll"
        // `overscroll-behavior` (CSS): `auto` chains, `contain` keeps the
        // gesture and bounces, `none` keeps it and does not.
        let bx = s["overscroll_behavior_x"] as? String ?? "auto"
        let by = s["overscroll_behavior_y"] as? String ?? "auto"
        scroll?.containX = bx != "auto"
        scroll?.containY = by != "auto"
        scroll?.bouncesX = bx == "contain"
        scroll?.bouncesY = by == "contain"
        // Set with the style and not per event: elasticity is what AppKit
        // reads to decide whether a gesture may stretch past the end, and
        // writing it while one is in flight disturbs the machine it enables.
        let ex: NSScrollView.Elasticity = bx == "contain" ? .allowed : bx == "none" ? .none : .automatic
        let ey: NSScrollView.Elasticity = by == "contain" ? .allowed : by == "none" ? .none : .automatic
        if let sv = scroll, sv.horizontalScrollElasticity != ex { sv.horizontalScrollElasticity = ex }
        if let sv = scroll, sv.verticalScrollElasticity != ey { sv.verticalScrollElasticity = ey }
        scroll?.hasHorizontalScroller = ox == "scroll"
        scroll?.hasVerticalScroller = oy == "scroll"
        clipsToBounds = ox == "hidden" || oy == "hidden"
        styleTextArea()
        if let f = field, let t = text {
            f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic")
            f.textColor = color("text_color", .black)
            applyPlaceholder(f)
            f.frame = fieldBox()
        }
        // CSS z-index: a WKWebView's remote layer otherwise paints over later
        // siblings (the account mark on the deck).
        layer?.zPosition = number("z_index")
        needsDisplay = true
    }

    func applyTransform() {
        let b = bounds
        var t = CGAffineTransform(translationX: translate.x, y: translate.y)
        t = t.translatedBy(x: b.midX, y: b.midY).rotated(by: rotate * .pi / 180).scaledBy(x: scale, y: scale).translatedBy(x: -b.midX, y: -b.midY)
        layer?.setAffineTransform(t)
    }

    override func layout() {
        if let s = presenter?.session, s.firstLayoutMs == nil { s.firstLayoutMs = ExactEnv.wall() }
        super.layout()
        if field != nil { field?.frame = fieldBox() }
        layoutTextArea()
    }

    override func draw(_ rect: NSRect) {
        repaintThrough()
        if Capture.capturing, kind == "canvas", let rep = canvases?.readback(view: self) {
            // A canvas nested under a canvas painted through its surface: its
            // picture into the ancestor's capture (LLP 1014); its own Metal
            // layer is not seen there.
            let picture = NSImage(size: bounds.size)
            picture.addRepresentation(rep)
            picture.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
        }
        // The first pixel is on its way: the GPU module may load now (LLP
        // 1009 D4), on the next turn. A batch's own attempt runs before the
        // display pass and finds no first draw yet; an app with no later
        // batch — no image, no timer, no motion — would never load it
        // (found by the readback fixture, LLP 1014).
        if presenter?.views[id] === self { firstDraw() }
        let radius = number("border_radius", number("border_radius_top_left"))
        let path = NSBezierPath(roundedRect: bounds, xRadius: radius, yRadius: radius)
        let bg = color("background_color", .clear)
        if bg.alphaComponent > 0 {
            bg.setFill()
            path.fill()
        }
        let borderColor = color("border_color", .clear)
        let uniform = number("border_width")
        let top = number("border_width_top", uniform), right = number("border_width_right", uniform)
        let bottom = number("border_width_bottom", uniform), left = number("border_width_left", uniform)
        // A uniform border on a rounded box follows the curve (the web's
        // rule). Four edge rects would square the corners and show as nubs.
        if radius > 0, top > 0, top == right, right == bottom, bottom == left {
            let inset = top / 2
            let stroke = NSBezierPath(roundedRect: bounds.insetBy(dx: inset, dy: inset), xRadius: max(0, radius - inset), yRadius: max(0, radius - inset))
            stroke.lineWidth = top
            stroke.lineJoinStyle = .round
            borderColor.setStroke()
            stroke.stroke()
        } else {
            let sides: [(String, NSRect)] = [
                ("border_width_top", NSRect(x: 0, y: 0, width: bounds.width, height: top)),
                ("border_width_bottom", NSRect(x: 0, y: bounds.height - bottom, width: bounds.width, height: bottom)),
                ("border_width_left", NSRect(x: 0, y: 0, width: left, height: bounds.height)),
                ("border_width_right", NSRect(x: bounds.width - right, y: 0, width: right, height: bounds.height)),
            ]
            for (key, r) in sides where number(key, uniform) > 0 {
                color(key.replacingOccurrences(of: "width", with: "color"), borderColor).setFill()
                r.fill()
            }
        }
        if kind == "image", let img = image {
            // CSS object-fit over the content box (the frame inside border
            // and padding), clipped by the border box's radius: `fill`
            // stretches, `contain`/`cover` keep the ratio, `none` is the
            // natural size, `scale-down` the smaller of none and contain;
            // an unknown value is the initial `fill`.
            let fit = style["object_fit"] as? String ?? "fill"
            let content = bounds.insetBy(
                left: number("border_width_left", uniform) + number("padding_left"),
                top: number("border_width_top", uniform) + number("padding_top"),
                right: number("border_width_right", uniform) + number("padding_right"),
                bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
            let natural = img.size
            var size = content.size
            if natural.width > 0 && natural.height > 0 {
                let sx = content.width / natural.width, sy = content.height / natural.height
                let s: CGFloat?
                switch fit {
                case "contain": s = min(sx, sy)
                case "cover": s = max(sx, sy)
                case "none": s = 1
                case "scale-down": s = min(1, min(sx, sy))
                default: s = nil // fill
                }
                if let s { size = CGSize(width: natural.width * s, height: natural.height * s) }
            }
            let origin = CGPoint(x: content.minX + (content.width - size.width) / 2, y: content.minY + (content.height - size.height) / 2)
            NSGraphicsContext.current?.saveGraphicsState()
            path.addClip()
            NSBezierPath(rect: content).addClip()
            img.draw(in: NSRect(origin: origin, size: size), from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
            NSGraphicsContext.current?.restoreGraphicsState()
        }
        let textDirty = Capture.capturing || canvasAbove != nil ? rect : rect.intersection(presenter?.textVisibleRect(self) ?? visibleRect)
        if isParagraph, !textDirty.isEmpty {
            // The same paragraph the kernel measured at this width, painted.
            let spec = paragraphSpec()
            if let ctx = NSGraphicsContext.current?.cgContext, let paragraph = paragraphLayout() {
                presenter?.selection.draw(self, paragraph: paragraph, spec: spec, dirty: textDirty)
                TextEngine.draw(paragraph, spec: spec, in: bounds, context: ctx, dirty: textDirty)
            }
        }
        if Capture.capturing, let picture = Capture.web[id] {
            // Remote WebKit layers supply their own picture for this capture
            // turn, at the node's normal hierarchy position (@ref LLP 1020 D4).
            picture.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
        }
    }


    /// A click counts even when it is the one that activates the window —
    /// the web's rule (a click on an unfocused page still clicks). AppKit's
    /// default swallows it, which made a `tap` sent before the window became
    /// key vanish (found driving the app by hand over stdin).
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    // Press: down and up inside the bounds. A pressed node that does not
    // take the focus ends the editing, as a click on a button blurs a page's
    // input; a click nothing consumes reaches the viewport, which does the
    // same (a click on the page's ground).
    override func mouseDown(with event: NSEvent) {
        guard !disabled else { pressed = false; return }
        if isParagraph, !handlers.contains("press"), !hasPressableAncestor {
            window?.makeFirstResponder(self)
            presenter?.selection.begin(self, event: event)
            return
        }
        if acceptsFirstResponder { window?.makeFirstResponder(self) }
        if handlers.contains("press") {
            if !acceptsFirstResponder { window?.makeFirstResponder(nil) }
            pressed = true
        } else { super.mouseDown(with: event) }
    }
    var hasPressableAncestor: Bool {
        var next = superview
        while let view = next {
            if let node = view as? NodeView, node.handlers.contains("press") { return true }
            next = view.superview
        }
        return false
    }
    override func mouseDragged(with event: NSEvent) {
        if isParagraph && !hasPressableAncestor { presenter?.selection.drag(event) }
        else { super.mouseDragged(with: event) }
    }
    override func mouseUp(with event: NSEvent) {
        if isParagraph && !hasPressableAncestor { presenter?.selection.end(self, event: event); return }
        guard !disabled else { pressed = false; return }
        guard pressed else { return super.mouseUp(with: event) }
        pressed = false
        if bounds.contains(local(event.locationInWindow)) { presenter?.press(id) }
    }
    func controlTextDidChange(_ obj: Notification) {
        if !disabled, handlers.contains("change") { presenter?.change(id, field?.stringValue ?? "") }
    }
}
#endif
