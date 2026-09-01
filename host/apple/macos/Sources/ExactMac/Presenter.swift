// The presenter: one NSView per kernel node, in a flipped root, applying the
// host's batches. Backgrounds, borders, and text are drawn; frames come from
// the kernel's layout; transforms and opacity from presentation values.
import AppKit

final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

/// Milliseconds from script start to the first node's first draw.
nonisolated(unsafe) var firstDrawMs: Double? = nil
/// Milliseconds from script start to the first node's first layout pass.
nonisolated(unsafe) var firstLayoutMs: Double? = nil

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

    override func scrollWheel(with event: NSEvent) {
        // Precise deltas (a trackpad) are in points; a wheel's are in lines.
        let precise = event.hasPreciseScrollingDeltas
        var dx = precise ? event.scrollingDeltaX : event.deltaX * ChainingScrollView.lineHeight
        var dy = precise ? event.scrollingDeltaY : event.deltaY * ChainingScrollView.lineHeight
        if dx == 0 && dy == 0 { return }
        let doc = documentView?.frame.size ?? .zero
        let origin = contentView.bounds.origin
        let visible = contentView.bounds.size
        // Per axis: can this view move in the delta's direction? (Flipped
        // document: origin grows as content scrolls up; a negative delta
        // scrolls content up.)
        let maxX = max(0, doc.width - visible.width), maxY = max(0, doc.height - visible.height)
        let takeX = scrollsX && dx != 0 && maxX > 0 && ((dx < 0 && origin.x < maxX) || (dx > 0 && origin.x > 0))
        let takeY = scrollsY && dy != 0 && maxY > 0 && ((dy < 0 && origin.y < maxY) || (dy > 0 && origin.y > 0))
        // The dominant axis decides who owns the event (a gesture is one
        // thing); what this view can take of it, it takes itself — never
        // through AppKit, whose nested-scroll routing may move the enclosing
        // view or animate later, doubling a delta applied here.
        let dominantTaken = abs(dy) >= abs(dx) ? takeY : takeX
        guard dominantTaken else { nextResponder?.scrollWheel(with: event); return }
        if !takeX { dx = 0 }
        if !takeY { dy = 0 }
        let target = NSPoint(x: min(max(origin.x - dx, 0), maxX), y: min(max(origin.y - dy, 0), maxY))
        contentView.scroll(to: target)
        reflectScrolledClipView(contentView)
    }
}

final class NodeView: NSView, NSTextFieldDelegate {
    let id: UInt32
    let kind: String
    var props: [String: String] = [:]
    var style: [String: Any] = [:]
    var handlers: Set<String> = []
    var translate = CGPoint.zero
    var scale: CGFloat = 1
    var rotate: CGFloat = 0
    weak var presenter: Presenter?
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

    /// A node with focus, blur, or key handlers takes the focus (an input's
    /// field does by itself): the web's rule that only a focusable element
    /// hears these. A pressable is in the tab order the way a `<button>` is.
    override var acceptsFirstResponder: Bool {
        if disabled { return false }
        if field != nil { return false }
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
    static func resolveSource(_ source: String) -> URL? {
        if let u = URL(string: source), let scheme = u.scheme {
            return scheme == "http" || scheme == "https" ? u : nil
        }
        let base = ProcessInfo.processInfo.environment["EXACT_ASSETS"] ?? FileManager.default.currentDirectoryPath
        let root = URL(fileURLWithPath: base, isDirectory: true)
            .standardizedFileURL.resolvingSymlinksInPath()
        let url = root.appendingPathComponent(source)
            .standardizedFileURL.resolvingSymlinksInPath()
        let rootPath = root.path.hasSuffix("/") ? root.path : root.path + "/"
        return url.path == root.path || url.path.hasPrefix(rootPath) ? url : nil
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
        guard let url = NodeView.resolveSource(source) else {
            image = nil
            FileHandle.standardError.write(Data("exact: image \(source) is not a loadable source\n".utf8))
            presenter?.intrinsic(id, nil)
            return
        }
        let id = self.id
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let loaded = (try? Data(contentsOf: url)).flatMap(NodeView.decode)
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
                if let c = self.canvasAbove { c.needsCapture = true; canvases.scheduleCapture() }
            }
        }
    }

    /// The view is gone: no load in flight may report for it.
    func forget() {
        loadGeneration += 1
        imageSource = nil
        image = nil
        webviews.destroy(id: id)
        web = nil
        presenter = nil
    }

    init(id: UInt32, kind: String, presenter: Presenter) {
        self.id = id
        self.kind = kind
        self.presenter = presenter
        super.init(frame: .zero)
        wantsLayer = true
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
        if kind == "input" {
            let f = makeField(secure: false)
            addSubview(f)
            field = f
        }
        if kind == "iframe", let w = webviews.create(owner: self) {
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
        canvases.scheduleCapture()
    }

    @objc func clipScrolled() { repaintThrough() }

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

    func color(_ key: String, _ fallback: NSColor) -> NSColor {
        guard let c = style[key] as? [Double], c.count == 4 else { return fallback }
        return NSColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
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
        }
        setAccessibilityEnabled(!disabled)
        setAccessibilityIdentifier(props["testId"])
        setAccessibilityLabel(props["accessibilityLabel"])
        if kind == "image", let src = props["imageSource"], src != imageSource { loadImage(src) }
        if kind == "image", props["imageSource"] == nil, imageSource != nil { imageSource = nil; image = nil; presenter?.intrinsic(id, nil) }
        if kind == "iframe" { webviews.update(self) }
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
        scroll?.hasHorizontalScroller = ox == "scroll"
        scroll?.hasVerticalScroller = oy == "scroll"
        clipsToBounds = ox == "hidden" || oy == "hidden"
        if let f = field {
            f.font = Text.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic")
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
        if firstLayoutMs == nil { firstLayoutMs = wall() }
        super.layout()
        if field != nil { field?.frame = fieldBox() }
    }

    override func draw(_ rect: NSRect) {
        repaintThrough()
        if Capture.capturing, kind == "canvas", let rep = canvases.readback(view: self) {
            // A canvas nested under a canvas painted through its surface: its
            // picture into the ancestor's capture (LLP 1014); its own Metal
            // layer is not seen there.
            let picture = NSImage(size: bounds.size)
            picture.addRepresentation(rep)
            picture.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
        }
        if firstDrawMs == nil {
            firstDrawMs = wall()
            // The first pixel is on its way: the GPU module may load now
            // (LLP 1009 D4), on the next turn. A batch's own attempt runs
            // before the display pass and finds no first draw yet; an app
            // with no later batch — no image, no timer, no motion — would
            // never load it (found by the readback fixture, LLP 1014).
            DispatchQueue.main.async { canvases.loadIfNeeded(); frames.run(frames.motion || canvases.wantsFrames) }
        }
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
        if kind == "text", let text = props["text"] {
            // The same paragraph the kernel measured at this width, painted.
            let spec = textSpec(text)
            if let ctx = NSGraphicsContext.current?.cgContext { Text.draw(Text.paragraph(spec, width: bounds.width), spec: spec, in: bounds, context: ctx) }
        }
        if Capture.capturing, let picture = Capture.web[id] {
            // Remote WebKit layers supply their own picture for this capture
            // turn, at the node's normal hierarchy position (@ref LLP 1020 D4).
            picture.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
        }
    }

    /// The paragraph spec from this node's rows, with CSS's defaults for the
    /// rows it does not set (the kernel's defaults are CSS's).
    func textSpec(_ text: String) -> Spec {
        let align: Int
        switch style["text_align"] as? String { case "center": align = 1; case "right": align = 2; case "justify": align = 3; default: align = 0 }
        let c = (style["text_color"] as? [Double]) ?? [0, 0, 0, 255]
        return Spec(
            runs: [Run(text: text, size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic", lineHeight: number("line_height"), letterSpacing: number("letter_spacing"))],
            align: align, lineClamp: Int(number("line_clamp")), color: c)
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
        if acceptsFirstResponder { window?.makeFirstResponder(self) }
        if handlers.contains("press") {
            if !acceptsFirstResponder { window?.makeFirstResponder(nil) }
            pressed = true
        } else { super.mouseDown(with: event) }
    }
    override func mouseUp(with event: NSEvent) {
        guard !disabled else { pressed = false; return }
        guard pressed else { return super.mouseUp(with: event) }
        pressed = false
        if bounds.contains(local(event.locationInWindow)) { presenter?.press(id) }
    }
    func controlTextDidChange(_ obj: Notification) {
        if !disabled, handlers.contains("change") { presenter?.change(id, field?.stringValue ?? "") }
    }
}

/// The viewport: a click that reached it — on no node that takes the focus
/// or a press — ends the editing, as a click on a page's blank ground blurs
/// the field (LLP 1008 §9).
final class PageScrollView: NSScrollView {
    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(nil)
        super.mouseDown(with: event)
    }
    /// AppKit turns automatic titlebar insets back on when this view
    /// becomes a window's content view, which leaves a black strip the
    /// height of the titlebar above the document (the night, the deck).
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        pinInsets()
    }
    override func tile() {
        super.tile()
        pinInsets()
    }
    func pinInsets() {
        if automaticallyAdjustsContentInsets { automaticallyAdjustsContentInsets = false }
        if contentInsets.top != 0 || contentInsets.left != 0 || contentInsets.bottom != 0 || contentInsets.right != 0 {
            contentInsets = NSEdgeInsetsZero
        }
    }
}

final class Presenter {
    /// The document: the roots live here, content-sized like a page.
    let root = FlippedView(frame: .zero)
    /// The viewport over it: the window's content view, scrolling like a browser's.
    let viewport = PageScrollView(frame: .zero)
    var views: [UInt32: NodeView] = [:]
    /// The native menu arm (LLP 1021 D3).
    lazy var menus = MenuHost(presenter: self)
    /// The first root's `viewportFit` prop (`"cover"` or nothing), as of the
    /// last batch; `onViewportFit` fires when it changes. macOS maps `cover`
    /// to a full-size-content window (the titlebar overlays the viewport;
    /// its height is `safe-area-inset-top`). @ref LLP 1008 §9
    private(set) var viewportFit: String?
    var onViewportFit: (() -> Void)?
    /// The safe-area insets the kernel was given: the titlebar under
    /// `viewport-fit=cover`, zero when the viewport is the content view
    /// below it. Reported to the agent as `env`.
    var insets = NSEdgeInsetsZero

    init() {
        viewport.documentView = root
        viewport.hasVerticalScroller = true
        viewport.hasHorizontalScroller = true
        viewport.autohidesScrollers = true
        viewport.scrollerStyle = .overlay
        viewport.automaticallyAdjustsContentInsets = false
        viewport.contentInsets = NSEdgeInsetsZero
        viewport.drawsBackground = true
        viewport.backgroundColor = .white
    }

    /// A restart: every view goes.
    func reset() {
        canvases.reset()
        views.values.forEach { $0.forget() }
        root.subviews.forEach { $0.removeFromSuperview() }
        views.removeAll()
        NodeView.imagesLoaded.removeAll()
    }

    /// Size the document to its roots, never smaller than the viewport.
    func fitDocument() {
        var size = viewport.contentSize
        for r in root.subviews {
            size.width = max(size.width, r.frame.maxX)
            size.height = max(size.height, r.frame.maxY)
        }
        if root.frame.size != size { root.frame = NSRect(origin: .zero, size: size) }
    }
    var onPress: ((UInt32) -> Void)?
    var onChange: ((UInt32, String) -> Void)?
    var onIntrinsic: ((UInt32, CGSize?) -> Void)?
    /// A capability an action called (LLP 1005 §3), after its commit.
    var onCommand: ((String, [Any]) -> Void)?
    /// The events beyond press and change (LLP 1005 §3).
    var onHover: ((UInt32, Bool) -> Void)?
    var onFocus: ((UInt32) -> Void)?
    var onBlur: ((UInt32) -> Void)?
    var onKey: ((UInt32, String) -> Void)?
    var onSubmit: ((UInt32) -> Void)?
    var onLoad: ((UInt32) -> Void)?
    var onMessage: ((UInt32, String) -> Void)?
    /// The node the pointer is over, of those with a hover handler: it hears
    /// the leave when the pointer moves onto another (the agent's `hover`).
    weak var hovered: NodeView?

    func press(_ id: UInt32) {
        onPress?(id)
        // An invoker's press also drops its menu (LLP 1021 D3).
        menus.pressed(id)
    }
    func change(_ id: UInt32, _ value: String) { onChange?(id, value) }
    /// An event a view reports: sent only while the presenter still has the
    /// view (the platform fires editing-ended as a destroyed field leaves the
    /// window; the browser fires no blur on removal, so neither does this
    /// host), and never while a batch is being applied — it waits for the
    /// batch to finish, then goes if its view survived it.
    private var applying = false
    private var waiting: [(UInt32, () -> Void)] = []
    private func send(_ id: UInt32, _ f: @escaping () -> Void) {
        guard views[id] != nil else { return }
        if applying { waiting.append((id, f)) } else { f() }
    }
    func hover(_ view: NodeView, _ over: Bool) {
        guard views[view.id] === view else { return }
        if over {
            if let h = hovered, h !== view { send(h.id) { [self] in onHover?(h.id, false) } }
            hovered = view
            send(view.id) { [self] in onHover?(view.id, true) }
        } else {
            if hovered === view { hovered = nil }
            send(view.id) { [self] in onHover?(view.id, false) }
        }
    }
    func focus(_ id: UInt32) { send(id) { [self] in onFocus?(id) } }
    func blur(_ id: UInt32) { send(id) { [self] in onBlur?(id) } }
    func key(_ id: UInt32, _ name: String) { send(id) { [self] in onKey?(id, name) } }
    func submit(_ id: UInt32) { send(id) { [self] in onSubmit?(id) } }
    func load(_ id: UInt32) { send(id) { [self] in onLoad?(id) } }
    func message(_ id: UInt32, _ value: String) { send(id) { [self] in onMessage?(id, value) } }
    func intrinsic(_ id: UInt32, _ size: CGSize?) { onIntrinsic?(id, size) }

    func apply(_ batch: Batch) {
        if let e = batch.error { FileHandle.standardError.write(Data("exact: \(e)\n".utf8)) }
        let outermost = !applying
        applying = true
        defer {
            if outermost {
                applying = false
                let q = waiting
                waiting = []
                for (id, f) in q where views[id] != nil { f() }
            }
        }
        for op in batch.ops {
            guard let kind = op["op"] as? String else { continue }
            let id = UInt32(op["id"] as? Int ?? 0)
            if kind == "children" { touched(id, children: true) } else if kind != "roots" && kind != "create" { touched(id) }
            switch kind {
            case "create":
                let v = NodeView(id: id, kind: op["kind"] as? String ?? "view", presenter: self)
                v.handlers = Set(op["handlers"] as? [String] ?? [])
                v.applyStyle(op["style"] as? [String: Any] ?? [:])
                v.applyProps(set: op["props"] as? [String: String] ?? [:], clear: [])
                views[id] = v
            case "props":
                views[id]?.applyProps(set: op["set"] as? [String: String] ?? [:], clear: op["clear"] as? [String] ?? [])
            case "style":
                views[id]?.applyStyle(op["style"] as? [String: Any] ?? [:])
            case "children":
                guard let parent = views[id] else { continue }
                let want = (op["ids"] as? [Int] ?? []).compactMap { views[UInt32($0)] }
                let container = parent.container
                for child in container.subviews where !(want as [NSView]).contains(child) && child is NodeView { child.removeFromSuperview() }
                for (i, child) in want.enumerated() {
                    if child.superview !== container { container.addSubview(child) }
                    if container.subviews.firstIndex(of: child) != i {
                        child.removeFromSuperview()
                        container.addSubview(child, positioned: .above, relativeTo: i > 0 ? want[i - 1] : nil)
                    }
                }
            case "surface":
                if let v = views[id] { canvases.surface(view: v, name: op["name"] as? String ?? "", values: op["values"] as? [Any] ?? []) }
            case "command":
                onCommand?(op["name"] as? String ?? "", op["args"] as? [Any] ?? [])
            case "destroy":
                canvases.destroy(view: id)
                views[id]?.forget()
                // Out of the map before out of the window: the editing-ended
                // notification removal fires finds no view to send for.
                let gone = views.removeValue(forKey: id)
                gone?.removeFromSuperview()
            case "roots":
                root.subviews.forEach { $0.removeFromSuperview() }
                for r in (op["ids"] as? [Int] ?? []).compactMap({ views[UInt32($0)] }) { root.addSubview(r) }
            case "frame":
                guard let v = views[id] else { continue }
                v.frame = NSRect(x: op["x"] as? Double ?? 0, y: op["y"] as? Double ?? 0, width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
                v.scroll?.frame = v.bounds
                v.field?.frame = v.fieldBox()
                v.metal?.frame = v.bounds
                v.overlay?.frame = v.bounds
                v.web?.frame = v.bounds
                v.applyTransform()
            case "content":
                views[id]?.scroll?.documentView?.frame = NSRect(x: 0, y: 0, width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
            case "present":
                guard let v = views[id] else { continue }
                let x = CGFloat(op["x"] as? Double ?? 0)
                switch op["property"] as? String {
                case "translate": v.translate = CGPoint(x: x, y: CGFloat(op["y"] as? Double ?? 0)); v.applyTransform()
                case "scale": v.scale = x; v.applyTransform()
                case "rotate": v.rotate = x; v.applyTransform()
                case "opacity": v.alphaValue = x
                default: break
                }
            default: break
            }
        }
        fitDocument()
        // The page's canvas colour is the first root's background — what
        // shows beyond a document shorter than the viewport, as a browser
        // paints the root element's background over the whole canvas.
        let color = (root.subviews.first as? NodeView)?.color("background_color", .white) ?? .white
        if viewport.backgroundColor != color { viewport.backgroundColor = color }
        let first = root.subviews.first as? NodeView
        let fit = first?.props["viewportFit"]
        if fit != viewportFit { viewportFit = fit; onViewportFit?() }
        canvases.captureIfNeeded()
        menus.sync()
        syncKeyViewLoop()
    }

    /// The view that takes Tab for this node: an input's field, else itself.
    private func keyView(of v: NodeView) -> NSView { v.field ?? v }

    /// Sequential focus after a batch: tree order, then `tabIndex` > 0, as
    /// HTML. `autorecalculatesKeyViewLoop` stays false so nothing is focused
    /// at launch (LLP 1014); Tab from the viewport still reaches the first
    /// tabbable. Hidden popover rows stay out (their container is hidden).
    func syncKeyViewLoop() {
        var listed: [NodeView] = []
        func walk(_ v: NodeView) {
            if v.props["inert"] == "true" || v.isHidden { return }
            if Self.tabbable(v) { listed.append(v) }
            for child in v.container.subviews.compactMap({ $0 as? NodeView }) { walk(child) }
        }
        for r in root.subviews.compactMap({ $0 as? NodeView }) { walk(r) }
        let tabbable = listed.enumerated().sorted { a, b in
            let ia = Self.tabIndex(a.element), ib = Self.tabIndex(b.element)
            let pa = ia > 0 ? ia : Int.max, pb = ib > 0 ? ib : Int.max
            if pa != pb { return pa < pb }
            return a.offset < b.offset
        }.map(\.element)
        if tabbable.isEmpty {
            viewport.nextKeyView = nil
            return
        }
        for (i, v) in tabbable.enumerated() {
            keyView(of: v).nextKeyView = keyView(of: tabbable[(i + 1) % tabbable.count])
        }
        viewport.nextKeyView = keyView(of: tabbable[0])
    }

    private static func tabIndex(_ v: NodeView) -> Int { Int(v.props["tabIndex"] ?? "0") ?? 0 }

    private static func tabbable(_ v: NodeView) -> Bool {
        if v.props["disabled"] == "true" { return false }
        let index = tabIndex(v)
        if index < 0 { return false }
        if v.field != nil { return true }
        if v.kind == "button" || v.kind == "toggle" || v.handlers.contains("press") { return true }
        if v.acceptsFirstResponder { return true }
        return index > 0
    }

    /// An op touched a node (LLP 1014 D4 a): every canvas it is painted
    /// through captures again at the end of the batch — the canvas above
    /// it, and itself for its own `children` op.
    func touched(_ id: UInt32, children: Bool = false) {
        guard let start = views[id] else { return }
        if children, start.overlay != nil { start.needsCapture = true }
        if let c = start.canvasAbove { c.needsCapture = true }
    }
}

/// A view's subtree as pixels (LLP 1014 D3).
enum Capture {
    /// A capture is drawing: its draws are not repaints (D4 b).
    nonisolated(unsafe) static var capturing = false
    /// Guest pictures for this turn; the remote platform views are hidden
    /// while their owning nodes draw these (@ref LLP 1020 D4/D6).
    nonisolated(unsafe) static var web: [UInt32: ExactWebImage] = [:]

    /// The subtree painted at `scale`: premultiplied RGBA, rows top-down,
    /// `pixelsWide * 4` bytes per row, transparent where nothing painted.
    static func bitmap(of view: NSView, scale: CGFloat) -> NSBitmapImageRep? {
        let w = Int((view.bounds.width * scale).rounded()), h = Int((view.bounds.height * scale).rounded())
        guard w > 0, h > 0,
              let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: w * 4, bitsPerPixel: 32)
        else { return nil }
        rep.size = view.bounds.size
        if let p = rep.bitmapData { memset(p, 0, h * w * 4) }
        // A subtree painted through its canvas composites at alpha 0; paint
        // it opaque into the bitmap regardless.
        let alpha = view.alphaValue
        view.alphaValue = 1
        // A canvas nested under this one that is painted through its own
        // surface: its picture comes by readback (its draw), not from its
        // overlay's views, which cacheDisplay would paint regardless of their
        // alpha — so those are hidden for the duration.
        var hidden: [NSView] = []
        func hide(_ v: NSView) {
            for s in v.subviews {
                if let n = s as? NodeView, let o = n.overlay, o.alphaValue == 0, !o.isHidden { o.isHidden = true; hidden.append(o); continue }
                hide(s)
            }
        }
        hide(view)
        capturing = true
        view.cacheDisplay(in: view.bounds, to: rep)
        capturing = false
        for o in hidden { o.isHidden = false }
        view.alphaValue = alpha
        return rep
    }
}

extension NSRect {
    /// The rect inside the given edges (never negative in size).
    func insetBy(left: CGFloat, top: CGFloat, right: CGFloat, bottom: CGFloat) -> NSRect {
        NSRect(x: minX + left, y: minY + top, width: max(0, width - left - right), height: max(0, height - top - bottom))
    }
}
