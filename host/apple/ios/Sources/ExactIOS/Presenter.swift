// The presenter: one UIView per kernel node, in a document inside a scroll
// view, applying the host's batches — the AppKit presenter's shape
// (host/apple/macos/…/Presenter.swift) on UIKit. Nothing flips (UIKit's
// origin is the top-left already); a scroll container is a UIScrollView;
// a press is a touch down and up inside the bounds, and a touch on a node
// without a handler goes up the responder chain as a DOM click bubbles;
// transforms and opacity come from presentation values, about the center.
import ImageIO
import UIKit

/// Milliseconds from script start to the first node's first draw.
nonisolated(unsafe) var firstDrawMs: Double? = nil
/// Milliseconds from script start to the first node's first layout pass.
nonisolated(unsafe) var firstLayoutMs: Double? = nil

/// A plain container: the document, a canvas's overlay (LLP 1014). Hit-
/// testable at alpha 0 — a canvas's children painted through its surface
/// composite at alpha 0 and must still take a tap, which UIKit's default
/// hit-test refuses below 0.01 — and transparent to a hit on nothing, so
/// the touch reaches what holds it (the canvas, the viewport).
final class PlainView: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !isHidden, isUserInteractionEnabled, bounds.contains(point) else { return nil }
        for sub in subviews.reversed() {
            if let hit = sub.hitTest(convert(point, to: sub), with: event) { return hit }
        }
        return nil
    }
}

/// A scroll container — the viewport over the document, and a node whose
/// effective `overflow` scrolls. The platform pans it (LLP 1002 D4: scroll
/// always wins; UIKit does not chain a pan out of a nested scroll view at
/// its edge). Which axes it scrolls comes from the node's rows; a tap's
/// wheel (the agent's) applies the web's chaining rule itself
/// (`AgentIOS.swift`).
final class ScrollView: UIScrollView {
    var scrollsX = true
    var scrollsY = true
    /// A touch that no node took — nothing focusable, nothing pressable —
    /// ends the editing, as a tap on a page's blank ground blurs the field
    /// and sends the keyboard away (LLP 1008 §9).
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesEnded(touches, with: event)
        if let t = touches.first, bounds.contains(t.location(in: self)) { window?.endEditing(true) }
    }
}

final class NodeView: UIView, UITextFieldDelegate, UIScrollViewDelegate {
    let id: UInt32
    let kind: String
    var props: [String: String] = [:]
    var style: [String: Any] = [:]
    var handlers: Set<String> = [] {
        didSet {
            if handlers.contains("hover"), hoverRecognizer == nil {
                let g = UIHoverGestureRecognizer(target: self, action: #selector(hovering(_:)))
                addGestureRecognizer(g)
                hoverRecognizer = g
            }
        }
    }
    var hoverRecognizer: UIHoverGestureRecognizer?
    var translate = CGPoint.zero
    var scale: CGFloat = 1
    var rotate: CGFloat = 0
    weak var presenter: Presenter?
    var field: UITextField?
    var scroll: ScrollView?
    /// The platform view returned by the dlopened iframe arm (@ref LLP 1020 D3).
    var web: UIView?
    /// A scroll container's content extent (the `content` op), before the
    /// axes that do not scroll are held to the box.
    var content = CGSize.zero
    /// A canvas node's Metal layer (LLP 1009).
    var metal: MetalView?
    /// A canvas's children live here (LLP 1014): laid out by the kernel in
    /// the canvas's box, over the Metal layer; when the surface samples them
    /// they are painted into its children texture and this view composites
    /// at alpha 0. `needsCapture`: painted again at the next capture;
    /// `paintedThisTurn`: a draw on this turn is the capture's own.
    var overlay: PlainView?
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
    var image: UIImage?
    var imageSource: String?
    var loadGeneration = 0
    var pressed = false
    /// Images loaded since launch (smoke reporting).
    nonisolated(unsafe) static var imagesLoaded: [(String, CGSize)] = []

    /// A node with focus, blur, or key handlers takes the focus (an input's
    /// field does by itself): the web's rule that only a focusable element
    /// hears these. Keys come from a hardware keyboard (`pressesBegan`).
    override var canBecomeFirstResponder: Bool { field == nil && !handlers.isDisjoint(with: ["focus", "blur", "key"]) }
    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        if ok, handlers.contains("focus") { presenter?.focus(id) }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok, handlers.contains("blur") { presenter?.blur(id) }
        return ok
    }
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        guard handlers.contains("key"), let key = presses.first?.key else { return super.pressesBegan(presses, with: event) }
        presenter?.key(id, NodeView.keyName(key))
    }
    /// The web's key names for UIKit's.
    static func keyName(_ key: UIKey) -> String {
        switch key.keyCode {
        case .keyboardReturnOrEnter, .keypadEnter: return "Enter"
        case .keyboardEscape: return "Escape"
        case .keyboardTab: return "Tab"
        case .keyboardDeleteOrBackspace: return "Backspace"
        case .keyboardDeleteForward: return "Delete"
        case .keyboardUpArrow: return "ArrowUp"
        case .keyboardDownArrow: return "ArrowDown"
        case .keyboardLeftArrow: return "ArrowLeft"
        case .keyboardRightArrow: return "ArrowRight"
        default: return key.charactersIgnoringModifiers
        }
    }
    /// A pointer over the node (an iPad's trackpad or mouse; a phone has
    /// none): `hover` in and out.
    @objc func hovering(_ g: UIHoverGestureRecognizer) {
        switch g.state {
        case .began: presenter?.hover(self, true)
        case .ended, .cancelled, .failed: presenter?.hover(self, false)
        default: break
        }
    }
    /// A text field's Enter as a key (its characters are its `change`);
    /// the editing goes on, as on the web.
    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        // Enter in an input with a `submit` handler is the web's implicit
        // submission; a `key` handler hears it as Enter as well.
        if handlers.contains("submit") { presenter?.submit(id) }
        if handlers.contains("key") { presenter?.key(id, "Enter") }
        return false
    }

    /// Where an image source resolves, as a page resolves `src`: an `http(s)`
    /// URL as is; a relative path under the asset root (`EXACT_ASSETS`, else
    /// the app bundle, which carries the app's `assets/` — a phone reads no
    /// other machine's paths) and never outside it; anything else (`file:`,
    /// `..` escaping the root) does not load.
    static func resolveSource(_ source: String) -> URL? {
        if let u = URL(string: source), let scheme = u.scheme {
            return scheme == "http" || scheme == "https" ? u : nil
        }
        let base = ProcessInfo.processInfo.environment["EXACT_ASSETS"] ?? Bundle.main.bundlePath
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
                if let (cg, size) = loaded {
                    self.image = UIImage(cgImage: cg)
                    NodeView.imagesLoaded.append((source, size))
                    presenter.intrinsic(id, size)
                } else {
                    self.image = nil
                    FileHandle.standardError.write(Data("exact: image \(source) did not load\n".utf8))
                    presenter.intrinsic(id, nil)
                }
                // Only now, with the picture in hand. A picture arriving is a
                // repaint under a canvas (LLP 1014 D4 b) that `draw(_:)`
                // cannot report — the overlay is at alpha 0 — and a box of
                // fixed size gives the kernel no relayout to capture after.
                // Ask the canvas for this turn's capture directly.
                self.setNeedsDisplay()
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
        isOpaque = false
        backgroundColor = .clear
        // A frame change repaints at the new width instead of stretching
        // stale pixels.
        contentMode = .redraw
        if kind == "canvas" {
            let m = MetalView(frame: .zero)
            addSubview(m)
            metal = m
            let o = PlainView(frame: .zero)
            o.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            addSubview(o)
            overlay = o
        }
        if kind == "input" {
            let f = UITextField(frame: .zero)
            f.borderStyle = .none
            f.backgroundColor = .clear
            f.delegate = self
            f.addTarget(self, action: #selector(fieldChanged), for: .editingChanged)
            f.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            addSubview(f)
            field = f
        }
        if kind == "iframe", let w = webviews.create(owner: self) {
            w.frame = bounds
            w.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            addSubview(w)
            web = w
        }
    }
    required init?(coder: NSCoder) { nil }

    /// Where children go: the scroll container, the overlay, or this view.
    var container: UIView { scroll ?? overlay ?? self }

    /// The canvas this node is painted through, if any: the nearest canvas
    /// above whose overlay holds it.
    var canvasAbove: NodeView? {
        var v: UIView = self
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

    /// A scroll under a canvas repaints it (LLP 1014 D4 c).
    func scrollViewDidScroll(_ scrollView: UIScrollView) { repaintThrough() }

    /// The direct child of a canvas this node is under, when that child is
    /// placed by the surface: the node whose `placement` maps this subtree.
    var placedAncestor: NodeView? {
        var v: UIView? = self
        while let n = v {
            if let node = n as? NodeView, node.placement != nil { return node }
            if let s = n.superview as? PlainView, let c = s.superview as? NodeView, c.overlay === s { return nil }
            v = n.superview
        }
        return nil
    }

    /// A homography applied to a point (row major, projective).
    static func map(_ h: [Double], _ p: CGPoint) -> CGPoint {
        let w = h[6] * p.x + h[7] * p.y + h[8]
        guard abs(w) > 1e-9 else { return CGPoint(x: CGFloat.infinity, y: CGFloat.infinity) }
        return CGPoint(x: (h[0] * p.x + h[1] * p.y + h[2]) / w, y: (h[3] * p.x + h[4] * p.y + h[5]) / w)
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
    /// UIKit's own conversion.
    func local(_ windowPoint: CGPoint) -> CGPoint {
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
        UIAccessibility.post(notification: .layoutChanged, argument: nil)
    }

    /// Hit-testing through the surface's placements (LLP 1014 D5): a canvas
    /// whose children are placed maps the point through each child's
    /// inverse, topmost first — straight from the canvas to the child,
    /// skipping the box UIKit would test. A placed child is only where the
    /// surface put it, never at its kernel frame: the rest of the overlay
    /// (children the surface left in place) is tested in UIKit's order
    /// without them, and then the canvas itself is the hit. (`point` is in
    /// this view's own coordinates — UIKit's convention, not AppKit's.)
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard let overlay else { return super.hitTest(point, with: event) }
        let placed = overlay.subviews.compactMap { $0 as? NodeView }.filter { $0.placement != nil }
        guard !placed.isEmpty else { return super.hitTest(point, with: event) }
        guard !isHidden, isUserInteractionEnabled, bounds.contains(point) else { return nil }
        // Nearest first: what is seen on top is what a tap reaches.
        for child in placed.sorted(by: { ($0.placement?[9] ?? 0) > ($1.placement?[9] ?? 0) }) {
            guard let h = child.placement, let inv = NodeView.invert(h) else { continue }
            let p = NodeView.map(inv, point)
            guard child.bounds.contains(p) else { continue }
            if let hit = child.hitTest(p, with: event) { return hit }
        }
        let inOverlay = overlay.convert(point, from: self)
        for child in overlay.subviews.reversed() where (child as? NodeView)?.placement == nil {
            if let hit = child.hitTest(child.convert(inOverlay, from: overlay), with: event) { return hit }
        }
        return self
    }

    /// The box on screen, through the placement of the placed child this
    /// node is (or is under), for assistive technology — the same box the
    /// agent's `layout` reports.
    override var accessibilityFrame: CGRect {
        get {
            guard let placed = placedAncestor, let h = placed.placement, let overlay = placed.superview, let canvas = overlay.superview as? NodeView else { return super.accessibilityFrame }
            let corners = [CGPoint(x: 0, y: 0), CGPoint(x: bounds.width, y: 0), CGPoint(x: bounds.width, y: bounds.height), CGPoint(x: 0, y: bounds.height)].map { NodeView.map(h, placed.convert($0, from: self)) }
            let xs = corners.map { $0.x }, ys = corners.map { $0.y }
            let inCanvas = CGRect(x: xs.min()!, y: ys.min()!, width: xs.max()! - xs.min()!, height: ys.max()! - ys.min()!)
            return UIAccessibility.convertToScreenCoordinates(inCanvas, in: canvas)
        }
        set { super.accessibilityFrame = newValue }
    }

    func color(_ key: String, _ fallback: UIColor) -> UIColor {
        guard let c = style[key] as? [Double], c.count == 4 else { return fallback }
        return Text.color(c)
    }
    func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
        if let n = style[key] as? Double { return CGFloat(n) }
        return fallback
    }

    func applyProps(set: [String: String], clear: [String]) {
        for k in clear { props.removeValue(forKey: k) }
        for (k, v) in set { props[k] = v }
        if let f = field {
            if let v = props["value"], f.text != v { f.text = v }
            f.placeholder = props["placeholder"]
            // The web's `type` and `inputmode`, as UIKit spells them.
            let type = props["type"] ?? "text"
            f.isSecureTextEntry = type == "password"
            f.textContentType = type == "password" ? .password : type == "email" ? .emailAddress : nil
            f.autocapitalizationType = (type == "password" || type == "email" || type == "url") ? .none : .sentences
            f.autocorrectionType = (type == "password" || type == "email" || type == "url") ? .no : .default
            switch props["inputMode"] ?? type {
            case "email": f.keyboardType = .emailAddress
            case "numeric": f.keyboardType = .numberPad
            case "decimal", "number": f.keyboardType = .decimalPad
            case "tel": f.keyboardType = .phonePad
            case "url": f.keyboardType = .URL
            case "search": f.keyboardType = .webSearch
            default: f.keyboardType = .default
            }
            f.returnKeyType = handlers.contains("submit") ? .go : .default
        }
        accessibilityIdentifier = props["testId"]
        accessibilityLabel = props["accessibilityLabel"]
        if kind == "image", let src = props["imageSource"], src != imageSource { loadImage(src) }
        if kind == "image", props["imageSource"] == nil, imageSource != nil { imageSource = nil; image = nil; presenter?.intrinsic(id, nil) }
        if kind == "iframe" { webviews.update(self) }
        setNeedsDisplay()
    }

    func applyStyle(_ s: [String: Any]) {
        style = s
        // Scrolling and clipping come from the effective overflow the host
        // wrote in (never from the node's kind): `scroll` on an axis makes a
        // scroll container that scrolls that axis; `hidden` clips.
        let ox = s["overflow_x"] as? String ?? "visible", oy = s["overflow_y"] as? String ?? "visible"
        if (ox == "scroll" || oy == "scroll") && scroll == nil {
            let sv = ScrollView(frame: bounds)
            sv.backgroundColor = .clear
            sv.contentInsetAdjustmentBehavior = .never
            sv.delegate = self
            sv.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            for child in subviews where child is NodeView { child.removeFromSuperview(); sv.addSubview(child) }
            addSubview(sv)
            scroll = sv
        }
        if ox != "scroll" && oy != "scroll", let sv = scroll {
            // Neither axis scrolls any more: the children come back out.
            for child in sv.subviews where child is NodeView { child.removeFromSuperview(); addSubview(child) }
            sv.removeFromSuperview()
            scroll = nil
        }
        scroll?.scrollsX = ox == "scroll"
        scroll?.scrollsY = oy == "scroll"
        scroll?.showsHorizontalScrollIndicator = ox == "scroll"
        scroll?.showsVerticalScrollIndicator = oy == "scroll"
        fitScroll()
        clipsToBounds = ox == "hidden" || oy == "hidden"
        if let f = field {
            f.font = Text.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic")
            f.textColor = color("text_color", .black)
        }
        setNeedsDisplay()
    }

    /// The scroll container's content size: the kernel's extent on an axis
    /// that scrolls, the box on one that does not (so UIKit cannot pan it
    /// there), never less than the box.
    func fitScroll() {
        guard let sv = scroll else { return }
        let size = CGSize(width: sv.scrollsX ? max(content.width, sv.bounds.width) : sv.bounds.width, height: sv.scrollsY ? max(content.height, sv.bounds.height) : sv.bounds.height)
        if sv.contentSize != size { sv.contentSize = size }
    }

    func applyTransform() {
        // CSS's individual transforms: translate, then rotate, then scale,
        // about the center (UIKit's anchor).
        transform = CGAffineTransform(translationX: translate.x, y: translate.y).rotated(by: rotate * .pi / 180).scaledBy(x: scale, y: scale)
    }

    override func layoutSubviews() {
        if firstLayoutMs == nil { firstLayoutMs = wall() }
        super.layoutSubviews()
    }

    override func draw(_ rect: CGRect) {
        repaintThrough()
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        if Capture.capturing, kind == "canvas", let picture = canvases.picture(of: self) {
            // A canvas nested under a canvas painted through its surface: its
            // picture into the ancestor's capture (LLP 1014); its own Metal
            // layer is not seen there.
            UIImage(cgImage: picture).draw(in: bounds)
        }
        if firstDrawMs == nil {
            firstDrawMs = wall()
            // The first pixel is on its way: the GPU module may load now
            // (LLP 1009 D4), on the next turn (LLP 1014's readback fixture
            // found a batch's own attempt too early).
            DispatchQueue.main.async { canvases.loadIfNeeded(); frames.run(frames.motion || canvases.wantsFrames) }
        }
        let radius = number("border_radius", number("border_radius_top_left"))
        let path = UIBezierPath(roundedRect: bounds, cornerRadius: radius)
        let bg = color("background_color", .clear)
        if bg.cgColor.alpha > 0 {
            bg.setFill()
            path.fill()
        }
        let borderColor = color("border_color", .clear)
        let uniform = number("border_width")
        let sides: [(String, CGRect)] = [
            ("border_width_top", CGRect(x: 0, y: 0, width: bounds.width, height: number("border_width_top", uniform))),
            ("border_width_bottom", CGRect(x: 0, y: bounds.height - number("border_width_bottom", uniform), width: bounds.width, height: number("border_width_bottom", uniform))),
            ("border_width_left", CGRect(x: 0, y: 0, width: number("border_width_left", uniform), height: bounds.height)),
            ("border_width_right", CGRect(x: bounds.width - number("border_width_right", uniform), y: 0, width: number("border_width_right", uniform), height: bounds.height)),
        ]
        for (key, r) in sides where number(key, uniform) > 0 {
            ctx.setFillColor(color(key.replacingOccurrences(of: "width", with: "color"), borderColor).cgColor)
            ctx.fill(r)
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
            ctx.saveGState()
            path.addClip()
            UIBezierPath(rect: content).addClip()
            img.draw(in: CGRect(origin: origin, size: size))
            ctx.restoreGState()
        }
        if kind == "text", let text = props["text"] {
            // The same paragraph the kernel measured at this width, painted.
            let spec = textSpec(text)
            Text.draw(Text.paragraph(spec, width: bounds.width), spec: spec, in: bounds, context: ctx)
        }
        if Capture.capturing, let picture = Capture.web[id] {
            // A capture that populated an arm snapshot draws that one WebKit
            // source at the node's hierarchy position (@ref LLP 1020 D4).
            picture.draw(in: bounds)
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

    // Press: a touch down and up inside the bounds. A node without a
    // handler passes the touch up the responder chain (UIView's default),
    // so a touch on a button's text reaches the button, as a DOM click
    // bubbles. A pan cancels it (the scroll view's `canCancelContentTouches`):
    // scroll always wins.
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        if handlers.contains("press") { pressed = true } else { super.touchesBegan(touches, with: event) }
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        if !pressed { super.touchesMoved(touches, with: event) }
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if canBecomeFirstResponder, !isFirstResponder { _ = becomeFirstResponder() }
        guard pressed else { return super.touchesEnded(touches, with: event) }
        pressed = false
        // A pressed node that did not take the focus: the field being edited
        // loses it, as a click on a button blurs a page's input.
        if !isFirstResponder { window?.endEditing(true) }
        if let t = touches.first, bounds.contains(local(t.location(in: nil))) { presenter?.press(id) }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        if pressed { pressed = false } else { super.touchesCancelled(touches, with: event) }
    }

    /// A press delivered by the rule a touch gets: to this node when it has
    /// a handler, else to the nearest ancestor with one, if the point (in
    /// the window) is inside that node's box. The agent's `tap` and
    /// VoiceOver's activation come here — UIKit offers no public touch
    /// synthesis.
    @discardableResult
    func activate(at windowPoint: CGPoint) -> NodeView? {
        var v: UIView? = self
        while let cur = v {
            if let n = cur as? NodeView, n.handlers.contains("press") {
                guard n.bounds.contains(n.local(windowPoint)) else { return nil }
                n.presenter?.press(n.id)
                return n
            }
            v = cur.superview
        }
        return nil
    }
    override func accessibilityActivate() -> Bool {
        activate(at: convert(CGPoint(x: bounds.midX, y: bounds.midY), to: nil)) != nil
    }

    @objc func fieldChanged() {
        if handlers.contains("change") { presenter?.change(id, field?.text ?? "") }
    }
    func textFieldDidBeginEditing(_ textField: UITextField) {
        presenter?.editing = self
        // The keyboard is already up (another field had it): it will not
        // move, so this field is revealed here, as a browser scrolls a
        // newly focused field into view.
        if let p = presenter, p.keyboardInset > 0 { if agentMode { p.reveal(self) } else { UIView.animate(withDuration: 0.25) { p.reveal(self) } } }
        if handlers.contains("focus") { presenter?.focus(id) }
    }
    func textFieldDidEndEditing(_ textField: UITextField) { if presenter?.editing === self { presenter?.editing = nil }; if handlers.contains("blur") { presenter?.blur(id) } }
}

final class Presenter {
    /// The document: the roots live here, content-sized like a page.
    let root = PlainView(frame: .zero)
    /// The viewport over it: the window's content, scrolling like a browser's.
    let viewport = ScrollView(frame: .zero)
    var views: [UInt32: NodeView] = [:]
    /// The input being edited, if any (UIKit exposes no first responder):
    /// what a canvas painted through its surface captures every frame for
    /// (LLP 1014 D4 d), and what the keyboard reveals.
    weak var editing: NodeView?
    /// The first root's `viewportFit` prop (`"cover"` or nothing), as of the
    /// last batch; `onViewportFit` fires when it changes.
    private(set) var viewportFit: String?
    var onViewportFit: (() -> Void)?
    /// The first root's `interactiveWidget` prop: `resizes-content` ends the
    /// layout viewport at the keyboard's top (the controller lays out again,
    /// inside the keyboard's animation, so every frame that moves moves with
    /// it); anything else is the default, `resizes-visual` — the inset below.
    private(set) var interactiveWidget: String?
    /// The keyboard's top edge in the window while one is shown, else nil.
    private(set) var keyboardTop: CGFloat?
    /// The controller's: frame the viewport again (`Controller.fit`).
    var onKeyboardResize: (() -> Void)?
    /// The safe-area insets the kernel was given (LLP 1008 §9): the
    /// screen's under `viewport-fit=cover`, zero when the viewport is the
    /// safe area itself. Reported to the agent as `env`.
    var insets = UIEdgeInsets.zero
    /// The keyboard's inset on the viewport: the points of the screen's
    /// viewport a software keyboard covers. By default the web's visual
    /// viewport — the layout viewport does not change; the viewport insets
    /// its content by this and reveals the field being edited, in the
    /// keyboard's own animation. Under `resizes-content` the controller
    /// sets it, measured against the viewport it would frame without a
    /// keyboard.
    var keyboardInset: CGFloat = 0

    init() {
        viewport.addSubview(root)
        viewport.contentInsetAdjustmentBehavior = .never
        viewport.backgroundColor = .white
    }

    func observeKeyboard() {
        let c = NotificationCenter.default
        c.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
        c.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillHideNotification, object: nil)
    }

    /// The keyboard is about to move: inset the viewport by what it will
    /// cover and reveal the field, inside an animation with the keyboard's
    /// own duration and curve — Core Animation runs both in the same
    /// transaction, so the content moves in lockstep with the keyboard,
    /// never a frame behind it.
    @objc func keyboardChanged(_ n: Notification) {
        guard let info = n.userInfo, let window = viewport.window, let parent = viewport.superview else { return }
        let end = (info[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect) ?? .zero
        let hiding = n.name == UIResponder.keyboardWillHideNotification
        // The keyboard's frame is the screen's; the viewport's, the window's.
        let keyboard = window.convert(end, from: window.screen.coordinateSpace)
        let shown = !hiding && keyboard.minY < window.bounds.maxY
        let top: CGFloat? = shown ? keyboard.minY : nil
        let duration = info[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double ?? 0.25
        let curve = info[UIResponder.keyboardAnimationCurveUserInfoKey] as? UInt ?? 7
        // A focus moving from one field to another comes as a burst of
        // notifications with no duration, over a few turns — the height
        // jittering between the two keyboards (335, 308, 335 on the
        // simulator) — and laying out for each flashed the page. Those wait
        // 80 ms for the last of them, which usually changes nothing. An
        // animated change (the show, the hide) is applied at once, in the
        // keyboard's own transaction, so it stays in step with the keyboard.
        keyboardDebounce?.cancel()
        keyboardDebounce = nil
        if duration > 0 {
            applyKeyboard(top: top, duration: duration, curve: curve)
        } else {
            let work = DispatchWorkItem { [weak self] in
                self?.keyboardDebounce = nil
                self?.applyKeyboard(top: top, duration: 0, curve: curve)
            }
            keyboardDebounce = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.08, execute: work)
        }
    }
    /// The last no-duration keyboard change, waiting to be applied.
    private var keyboardDebounce: DispatchWorkItem?

    /// The keyboard's top edge (nil: hidden) takes effect: the viewport is
    /// inset by the overlap (`resizes-visual`, the default) or laid out to
    /// end there (`resizes-content`, the controller's `fit`), the field
    /// being edited revealed after — inside an animation with the keyboard's
    /// own duration and curve, so the frames the batch sets are Core
    /// Animation moves in the keyboard's transaction, never a frame behind.
    func applyKeyboard(top: CGFloat?, duration: Double, curve: UInt) {
        guard let window = viewport.window, let parent = viewport.superview else { return }
        keyboardTop = top
        let change = {
            if self.interactiveWidget == "resizes-content" {
                self.onKeyboardResize?()
            } else {
                let frame = parent.convert(self.viewport.frame, to: window)
                let overlap = top.map { min(max(0, frame.maxY - max($0, frame.minY)), frame.height) } ?? 0
                self.setKeyboardInset(overlap)
            }
            self.reveal(self.editing ?? self.views.values.first { $0.field?.isFirstResponder == true })
        }
        // Under the agent (LLP 1012) the change applies at once, as the
        // agent's wheel scrolls at once: its world is settled between calls,
        // and UIKit hit-tests a scroll view at its presentation offset while
        // the keyboard's spring is still settling — a tap there would miss.
        if agentMode || duration <= 0 { change(); return }
        UIView.animate(withDuration: duration, delay: 0, options: [UIView.AnimationOptions(rawValue: curve << 16), .beginFromCurrentState], animations: change)
    }

    func setKeyboardInset(_ h: CGFloat) {
        keyboardInset = h
        var inset = viewport.contentInset
        inset.bottom = h
        viewport.contentInset = inset
        var indicators = viewport.verticalScrollIndicatorInsets
        indicators.bottom = h
        viewport.verticalScrollIndicatorInsets = indicators
    }

    /// Scroll a node into the part of the viewport the keyboard leaves —
    /// the browser's rule for a focused field — through every scroll
    /// container above it, each moving only as far as it must, never past
    /// its edges. Animated by whatever animation block this runs inside.
    func reveal(_ node: NodeView?) {
        guard let node, node.window != nil else { return }
        var v: UIView? = node.superview
        while let cur = v {
            if let sv = cur as? ScrollView {
                // The node's box in the container's content space, with a
                // little air; the visible part of that space.
                let r = node.convert(node.bounds, to: sv).insetBy(dx: 0, dy: -8)
                let visible = sv.bounds.inset(by: sv.adjustedContentInset)
                var o = sv.contentOffset
                if r.maxY > visible.maxY { o.y += r.maxY - visible.maxY } else if r.minY < visible.minY { o.y -= visible.minY - r.minY }
                if r.maxX > visible.maxX { o.x += r.maxX - visible.maxX } else if r.minX < visible.minX { o.x -= visible.minX - r.minX }
                let i = sv.adjustedContentInset
                o.y = min(max(o.y, -i.top), max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height))
                o.x = min(max(o.x, -i.left), max(-i.left, sv.contentSize.width + i.right - sv.bounds.width))
                if o != sv.contentOffset { sv.contentOffset = o }
            }
            v = cur.superview
        }
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
        var size = viewport.bounds.size
        for r in root.subviews {
            size.width = max(size.width, r.frame.maxX)
            size.height = max(size.height, r.frame.maxY)
        }
        if root.frame.size != size { root.frame = CGRect(origin: .zero, size: size) }
        if viewport.contentSize != size { viewport.contentSize = size }
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

    func press(_ id: UInt32) { onPress?(id) }
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
                for child in container.subviews where !(want as [UIView]).contains(child) && child is NodeView { child.removeFromSuperview() }
                // In order, below anything else in the container (a scroll
                // view's indicators): inserting a subview at an index moves
                // it when it is already there.
                for (i, child) in want.enumerated() { container.insertSubview(child, at: i) }
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
                // A frame is set untransformed (UIKit's `frame` is undefined
                // under a transform); the presentation goes back on after.
                v.transform = .identity
                v.frame = CGRect(x: op["x"] as? Double ?? 0, y: op["y"] as? Double ?? 0, width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
                v.scroll?.frame = v.bounds
                v.field?.frame = v.bounds
                v.metal?.frame = v.bounds
                v.overlay?.frame = v.bounds
                v.web?.frame = v.bounds
                v.fitScroll()
                v.applyTransform()
            case "content":
                guard let v = views[id] else { continue }
                v.content = CGSize(width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
                v.fitScroll()
            case "present":
                guard let v = views[id] else { continue }
                let x = CGFloat(op["x"] as? Double ?? 0)
                switch op["property"] as? String {
                case "translate": v.translate = CGPoint(x: x, y: CGFloat(op["y"] as? Double ?? 0)); v.applyTransform()
                case "scale": v.scale = x; v.applyTransform()
                case "rotate": v.rotate = x; v.applyTransform()
                case "opacity": v.alpha = x
                default: break
                }
            default: break
            }
        }
        fitDocument()
        paintCanvas()
        let first = root.subviews.first as? NodeView
        interactiveWidget = first?.props["interactiveWidget"]
        let fit = first?.props["viewportFit"]
        if fit != viewportFit { viewportFit = fit; onViewportFit?() }
        canvases.captureIfNeeded()
    }

    /// The page's canvas colour — behind the document and into the safe
    /// areas the layout keeps out of — is the first root's background, as
    /// Safari paints the root element's background under the status bar
    /// and the home indicator; white when the root sets none.
    var onCanvasColor: ((UIColor) -> Void)?
    func paintCanvas() {
        let color = (root.subviews.first as? NodeView)?.color("background_color", .white) ?? .white
        if viewport.backgroundColor != color { viewport.backgroundColor = color; onCanvasColor?(color) }
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

/// Pixels a view's subtree was painted into: premultiplied RGBA, rows
/// top-down, `width * 4` bytes per row, owned by the context.
struct Bitmap {
    let context: CGContext
    let width: Int
    let height: Int
    var bytes: UnsafeMutableRawPointer? { context.data }
    var bytesPerRow: Int { context.bytesPerRow }
    /// Empty pixels for the module to fill (a readback).
    static func blank(width: Int, height: Int) -> Bitmap? {
        guard width > 0, height > 0, let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)
        else { return nil }
        return Bitmap(context: ctx, width: width, height: height)
    }
    var image: UIImage? { context.makeImage().map { UIImage(cgImage: $0) } }
}

/// A view's subtree as pixels (LLP 1014 D3).
enum Capture {
    /// A capture is drawing: its draws are not repaints (D4 b).
    nonisolated(unsafe) static var capturing = false
    /// Guest pictures when this turn chooses arm snapshots; their remote
    /// platform views are hidden while the nodes draw these (LLP 1020 D4/D6).
    nonisolated(unsafe) static var web: [UInt32: ExactWebImage] = [:]
    /// EXACT_CAPTURE=cpu: the Core Graphics capture even where Metal is
    /// present — the measure's baseline, and the fixture's oracle.
    static let cpu = ProcessInfo.processInfo.environment["EXACT_CAPTURE"] == "cpu"
    /// The subtree painted at `scale`: premultiplied RGBA, rows top-down,
    /// transparent where nothing painted.
    static func bitmap(of view: UIView, scale: CGFloat) -> Bitmap? {
        let w = Int((view.bounds.width * scale).rounded()), h = Int((view.bounds.height * scale).rounded())
        guard let bitmap = Bitmap.blank(width: w, height: h) else { return nil }
        // The GPU, where there is one (`Shadow`); Core Graphics otherwise.
        if !cpu, let shadow = Shadow.shared {
            capturing = true
            let ok = shadow.render(view, scale: scale, into: bitmap)
            capturing = false
            if ok {
                // EXACT_CAPTURE_DUMP=<dir>: this render and the CPU one of the
                // same frame, as PNGs, to compare the two by eye.
                if let dir = ProcessInfo.processInfo.environment["EXACT_CAPTURE_DUMP"], dumped < 4 {
                    dumped += 1
                    try? bitmap.image?.pngData()?.write(to: URL(fileURLWithPath: dir).appendingPathComponent("gpu-\(dumped).png"))
                    if let cpuBitmap = Bitmap.blank(width: w, height: h) { draw(view, scale: scale, into: cpuBitmap); try? cpuBitmap.image?.pngData()?.write(to: URL(fileURLWithPath: dir).appendingPathComponent("cpu-\(dumped).png")) }
                }
                return bitmap
            }
        }
        draw(view, scale: scale, into: bitmap)
        return bitmap
    }
    nonisolated(unsafe) static var dumped = 0

    /// The CPU capture: Core Graphics rasterizes the subtree into `bitmap`.
    static func draw(_ view: UIView, scale: CGFloat, into bitmap: Bitmap) {
        let h = bitmap.height
        let ctx = bitmap.context
        // UIKit's geometry — y down from the top — into a context whose y
        // is up from the bottom: the first row in memory is then the top.
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: scale, y: -scale)
        // A subtree painted through its canvas composites at alpha 0; paint
        // it opaque into the bitmap regardless.
        let alpha = view.alpha
        view.alpha = 1
        // A canvas nested under this one that is painted through its own
        // surface: its picture comes by readback (its draw), not from its
        // overlay's views, which the render would paint regardless of their
        // alpha — so those are hidden for the duration.
        var hidden: [UIView] = []
        func hide(_ v: UIView) {
            for s in v.subviews {
                // A nested canvas paints its readback in `draw` (LLP 1014):
                // drop the layer's cached picture so the render calls `draw`
                // instead of copying what it drew last time (its placements
                // are read there too, so the cards of a deck in the sky move).
                if let n = s as? NodeView, n.kind == "canvas" { n.layer.contents = nil; n.setNeedsDisplay() }
                if let n = s as? NodeView, let o = n.overlay, o.alpha == 0, !o.isHidden { o.isHidden = true; hidden.append(o); continue }
                hide(s)
            }
        }
        hide(view)
        capturing = true
        UIGraphicsPushContext(ctx)
        view.layer.render(in: ctx)
        UIGraphicsPopContext()
        capturing = false
        for o in hidden { o.isHidden = false }
        view.alpha = alpha
    }
}

extension CGRect {
    /// The rect inside the given edges (never negative in size).
    func insetBy(left: CGFloat, top: CGFloat, right: CGFloat, bottom: CGFloat) -> CGRect {
        CGRect(x: minX + left, y: minY + top, width: max(0, width - left - right), height: max(0, height - top - bottom))
    }
}
