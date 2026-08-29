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
    /// An image node's picture, once loaded (decoded off the main thread),
    /// the source it came from, and which load is current: a completion
    /// from an older load, or for a view that was destroyed, is dropped.
    var image: NSImage?
    var imageSource: String?
    var loadGeneration = 0
    var pressed = false
    /// Images loaded since launch (smoke reporting).
    nonisolated(unsafe) static var imagesLoaded: [(String, CGSize)] = []

    /// Where an image source resolves, as a page resolves `src`: an `http(s)`
    /// URL as is; a relative path under the asset root (`EXACT_ASSETS`, else
    /// the current directory) and never outside it; anything else (`file:`,
    /// `..` escaping the root) does not load.
    static func resolveSource(_ source: String) -> URL? {
        if let u = URL(string: source), let scheme = u.scheme {
            return scheme == "http" || scheme == "https" ? u : nil
        }
        let base = ProcessInfo.processInfo.environment["EXACT_ASSETS"] ?? FileManager.default.currentDirectoryPath
        let root = URL(fileURLWithPath: base).standardizedFileURL.path
        let url = URL(fileURLWithPath: base).appendingPathComponent(source).standardizedFileURL
        return url.path == root || url.path.hasPrefix(root.hasSuffix("/") ? root : root + "/") ? url : nil
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
                self.needsDisplay = true
                if let (cg, size) = loaded {
                    self.image = NSImage(cgImage: cg, size: size)
                    NodeView.imagesLoaded.append((source, size))
                    presenter.intrinsic(id, size)
                } else {
                    self.image = nil
                    FileHandle.standardError.write(Data("exact: image \(source) did not load\n".utf8))
                    presenter.intrinsic(id, nil)
                }
            }
        }
    }

    /// The view is gone: no load in flight may report for it.
    func forget() {
        loadGeneration += 1
        imageSource = nil
        image = nil
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
            let f = NSTextField(frame: .zero)
            f.isBordered = false
            f.drawsBackground = false
            f.focusRingType = .none
            f.delegate = self
            f.autoresizingMask = [.width, .height]
            addSubview(f)
            field = f
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

    func color(_ key: String, _ fallback: NSColor) -> NSColor {
        guard let c = style[key] as? [Double], c.count == 4 else { return fallback }
        return NSColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
    }
    func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
        if let n = style[key] as? Double { return CGFloat(n) }
        return fallback
    }

    func applyProps(set: [String: String], clear: [String]) {
        for k in clear { props.removeValue(forKey: k) }
        for (k, v) in set { props[k] = v }
        if let f = field {
            if let v = props["value"], f.stringValue != v { f.stringValue = v }
            f.placeholderString = props["placeholder"]
        }
        setAccessibilityIdentifier(props["testId"])
        setAccessibilityLabel(props["accessibilityLabel"])
        if kind == "image", let src = props["imageSource"], src != imageSource { loadImage(src) }
        if kind == "image", props["imageSource"] == nil, imageSource != nil { imageSource = nil; image = nil; presenter?.intrinsic(id, nil) }
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
            f.font = Text.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), italic: false)
            f.textColor = color("text_color", .black)
        }
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
    }

    override func draw(_ rect: NSRect) {
        repaintThrough()
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
        let sides: [(String, NSRect)] = [
            ("border_width_top", NSRect(x: 0, y: 0, width: bounds.width, height: number("border_width_top", uniform))),
            ("border_width_bottom", NSRect(x: 0, y: bounds.height - number("border_width_bottom", uniform), width: bounds.width, height: number("border_width_bottom", uniform))),
            ("border_width_left", NSRect(x: 0, y: 0, width: number("border_width_left", uniform), height: bounds.height)),
            ("border_width_right", NSRect(x: bounds.width - number("border_width_right", uniform), y: 0, width: number("border_width_right", uniform), height: bounds.height)),
        ]
        for (key, r) in sides where number(key, uniform) > 0 {
            color(key.replacingOccurrences(of: "width", with: "color"), borderColor).setFill()
            r.fill()
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
            Text.draw(Text.paragraph(spec, width: bounds.width), spec: spec, in: bounds)
        }
    }

    /// The paragraph spec from this node's rows, with CSS's defaults for the
    /// rows it does not set (the kernel's defaults are CSS's).
    func textSpec(_ text: String) -> Spec {
        let align: Int
        switch style["text_align"] as? String { case "center": align = 1; case "right": align = 2; case "justify": align = 3; default: align = 0 }
        let c = (style["text_color"] as? [Double]) ?? [0, 0, 0, 255]
        return Spec(
            runs: [Run(text: text, size: number("font_size", 16), weight: Int(number("font_weight", 400)), italic: (style["font_style"] as? String) == "italic", lineHeight: number("line_height"), letterSpacing: number("letter_spacing"))],
            align: align, lineClamp: Int(number("line_clamp")), color: c)
    }

    /// A click counts even when it is the one that activates the window —
    /// the web's rule (a click on an unfocused page still clicks). AppKit's
    /// default swallows it, which made a `tap` sent before the window became
    /// key vanish (found driving the app by hand over stdin).
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    // Press: down and up inside the bounds.
    override func mouseDown(with event: NSEvent) {
        if handlers.contains("press") { pressed = true } else { super.mouseDown(with: event) }
    }
    override func mouseUp(with event: NSEvent) {
        guard pressed else { return super.mouseUp(with: event) }
        pressed = false
        if bounds.contains(convert(event.locationInWindow, from: nil)) { presenter?.press(id) }
    }
    func controlTextDidChange(_ obj: Notification) {
        if handlers.contains("change") { presenter?.change(id, field?.stringValue ?? "") }
    }
}

final class Presenter {
    /// The document: the roots live here, content-sized like a page.
    let root = FlippedView(frame: .zero)
    /// The viewport over it: the window's content view, scrolling like a browser's.
    let viewport = NSScrollView(frame: .zero)
    var views: [UInt32: NodeView] = [:]

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

    func press(_ id: UInt32) { onPress?(id) }
    func change(_ id: UInt32, _ value: String) { onChange?(id, value) }
    func intrinsic(_ id: UInt32, _ size: CGSize?) { onIntrinsic?(id, size) }

    func apply(_ batch: Batch) {
        if let e = batch.error { FileHandle.standardError.write(Data("exact: \(e)\n".utf8)) }
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
            case "destroy":
                canvases.destroy(view: id)
                views[id]?.forget()
                views[id]?.removeFromSuperview()
                views.removeValue(forKey: id)
            case "roots":
                root.subviews.forEach { $0.removeFromSuperview() }
                for r in (op["ids"] as? [Int] ?? []).compactMap({ views[UInt32($0)] }) { root.addSubview(r) }
            case "frame":
                guard let v = views[id] else { continue }
                v.frame = NSRect(x: op["x"] as? Double ?? 0, y: op["y"] as? Double ?? 0, width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
                v.scroll?.frame = v.bounds
                v.field?.frame = v.bounds
                v.metal?.frame = v.bounds
                v.overlay?.frame = v.bounds
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
        canvases.captureIfNeeded()
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
        capturing = true
        view.cacheDisplay(in: view.bounds, to: rep)
        capturing = false
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
