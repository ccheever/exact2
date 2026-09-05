// One UIView per kernel node (the UIKit presenter's node — the AppKit
// presenter's shape on UIKit, LLP 1008 §9): nothing flips (UIKit's origin
// is the top-left already); a scroll container is a UIScrollView; a press
// is a touch down and up inside the bounds, and a touch on a node without
// a handler goes up the responder chain as a DOM click bubbles; transforms
// and opacity come from presentation values, about the center. Everything
// a node reaches beyond itself — the text engine, the canvases, the web
// views — it reaches through its presenter's session (LLP 1031 D1).
#if os(iOS)
import ImageIO
import UIKit

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
    let firstDraw: () -> Void
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
    var disabled: Bool { props["disabled"] == "true" }
    /// Images loaded since launch (smoke reporting).
    nonisolated(unsafe) static var imagesLoaded: [(String, CGSize)] = []
    /// The session's text engine (LLP 1031 D12: the catalog is the session's).
    var text: TextEngine? { presenter?.session?.text }
    var canvases: Canvases? { presenter?.session?.canvases }

    /// A node with focus, blur, or key handlers takes the focus (an input's
    /// field does by itself): the web's rule that only a focusable element
    /// hears these. Keys come from a hardware keyboard (`pressesBegan`).
    override var canBecomeFirstResponder: Bool { !disabled && field == nil && !handlers.isDisjoint(with: ["focus", "blur", "key"]) }
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
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        guard !disabled, handlers.contains("key"), let key = presses.first?.key else { return super.pressesBegan(presses, with: event) }
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
        guard !disabled else { return false }
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
                if let c = self.canvasAbove { c.needsCapture = true; self.canvases?.scheduleCapture() }
            }
        }
    }

    /// The view is gone: no load in flight may report for it.
    func forget() {
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
            addSubview(f)
            field = f
        }
        if kind == "iframe", let w = presenter.session?.webviews.create(owner: self) {
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
        canvases?.scheduleCapture()
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
        // UIKit's default rejects a view when alpha is near zero. CSS opacity
        // changes painting, not hit participation, so walk the ordinary
        // subtree ourselves without consulting alpha.
        func ordinary() -> UIView? {
            guard !isHidden, isUserInteractionEnabled, bounds.contains(point) else { return nil }
            for child in subviews.reversed() {
                if let hit = child.hitTest(convert(point, to: child), with: event) { return hit }
            }
            return self
        }
        guard let overlay else { return ordinary() }
        let placed = overlay.subviews.compactMap { $0 as? NodeView }.filter { $0.placement != nil }
        guard !placed.isEmpty else { return ordinary() }
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
        return TextEngine.color(c)
    }
    func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
        if let n = style[key] as? Double { return CGFloat(n) }
        return fallback
    }

    /// The input's content box: padding and border sit on the node, the
    /// field is the text inside — CSS's rule, so a placeholder lines up
    /// with a native one.
    func fieldBox() -> CGRect {
        let uniform = number("border_width")
        return bounds.insetBy(
            left: number("border_width_left", uniform) + number("padding_left"),
            top: number("border_width_top", uniform) + number("padding_top"),
            right: number("border_width_right", uniform) + number("padding_right"),
            bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
    }

    func applyPlaceholder(_ f: UITextField) {
        let text = props["placeholder"] ?? ""
        let font = f.font ?? UIFont.systemFont(ofSize: 17)
        if text.isEmpty {
            f.attributedPlaceholder = nil
            f.placeholder = nil
            return
        }
        // Not `placeholderText`: that tracks the window's appearance, so a
        // white field in a dark app (the night) paints a light placeholder
        // and it vanishes. Mute this field's text color — the web's
        // `input::placeholder`.
        let ink = (f.textColor ?? UIColor(red: 0, green: 0, blue: 0, alpha: 1)).withAlphaComponent(0.30)
        f.attributedPlaceholder = NSAttributedString(string: text, attributes: [
            .font: font,
            .foregroundColor: ink,
        ])
    }

    func applyProps(set: [String: String], clear: [String]) {
        for k in clear { props.removeValue(forKey: k) }
        for (k, v) in set { props[k] = v }
        if let f = field {
            if let v = props["value"], f.text != v { f.text = v }
            applyPlaceholder(f)
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
            f.isEnabled = !disabled
        }
        if disabled { accessibilityTraits.insert(.notEnabled) } else { accessibilityTraits.remove(.notEnabled) }
        accessibilityIdentifier = props["testId"]
        accessibilityLabel = props["accessibilityLabel"]
        if kind == "image", let src = props["imageSource"], src != imageSource { loadImage(src) }
        if kind == "image", props["imageSource"] == nil, imageSource != nil { imageSource = nil; image = nil; presenter?.intrinsic(id, nil) }
        if kind == "iframe" { presenter?.session?.webviews.update(self) }
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
        if let f = field, let t = text {
            f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic")
            f.textColor = color("text_color", .black)
            applyPlaceholder(f)
            f.frame = fieldBox()
        }
        layer.zPosition = number("z_index")
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
        if let s = presenter?.session, s.firstLayoutMs == nil { s.firstLayoutMs = ExactEnv.wall() }
        super.layoutSubviews()
        if field != nil { field?.frame = fieldBox() }
    }

    override func draw(_ rect: CGRect) {
        repaintThrough()
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        if Capture.capturing, kind == "canvas", let picture = canvases?.picture(of: self) {
            // A canvas nested under a canvas painted through its surface: its
            // picture into the ancestor's capture (LLP 1014); its own Metal
            // layer is not seen there.
            UIImage(cgImage: picture).draw(in: bounds)
        }
        // The first pixel is on its way: the GPU module may load now (LLP
        // 1009 D4), on the next turn (LLP 1014's readback fixture found a
        // batch's own attempt too early).
        if presenter?.views[id] === self { firstDraw() }
        let radius = number("border_radius", number("border_radius_top_left"))
        let path = UIBezierPath(roundedRect: bounds, cornerRadius: radius)
        let bg = color("background_color", .clear)
        if bg.cgColor.alpha > 0 {
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
            let stroke = UIBezierPath(roundedRect: bounds.insetBy(dx: inset, dy: inset), cornerRadius: max(0, radius - inset))
            stroke.lineWidth = top
            stroke.lineJoinStyle = .round
            borderColor.setStroke()
            stroke.stroke()
        } else {
            let sides: [(String, CGRect)] = [
                ("border_width_top", CGRect(x: 0, y: 0, width: bounds.width, height: top)),
                ("border_width_bottom", CGRect(x: 0, y: bounds.height - bottom, width: bounds.width, height: bottom)),
                ("border_width_left", CGRect(x: 0, y: 0, width: left, height: bounds.height)),
                ("border_width_right", CGRect(x: bounds.width - right, y: 0, width: right, height: bounds.height)),
            ]
            for (key, r) in sides where number(key, uniform) > 0 {
                ctx.setFillColor(color(key.replacingOccurrences(of: "width", with: "color"), borderColor).cgColor)
                ctx.fill(r)
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
            ctx.saveGState()
            path.addClip()
            UIBezierPath(rect: content).addClip()
            img.draw(in: CGRect(origin: origin, size: size))
            ctx.restoreGState()
        }
        if isParagraph {
            // The same paragraph the kernel measured at this width, painted.
            let spec = paragraphSpec()
            if let t = self.text { TextEngine.draw(t.paragraph(spec, width: bounds.width), spec: spec, in: bounds, context: ctx) }
        }
        if Capture.capturing, let picture = Capture.web[id] {
            // A capture that populated an arm snapshot draws that one WebKit
            // source at the node's hierarchy position (@ref LLP 1020 D4).
            picture.draw(in: bounds)
        }
    }


    // Press: a touch down and up inside the bounds. A node without a
    // handler passes the touch up the responder chain (UIView's default),
    // so a touch on a button's text reaches the button, as a DOM click
    // bubbles. A pan cancels it (the scroll view's `canCancelContentTouches`):
    // scroll always wins.
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard !disabled else { pressed = false; return }
        if handlers.contains("press") { pressed = true } else { super.touchesBegan(touches, with: event) }
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        if !pressed { super.touchesMoved(touches, with: event) }
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard !disabled else { pressed = false; return }
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
            if let n = cur as? NodeView, n.disabled { return nil }
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
        if !disabled, handlers.contains("change") { presenter?.change(id, field?.text ?? "") }
    }
    func textFieldDidBeginEditing(_ textField: UITextField) {
        presenter?.editing = self
        // The keyboard is already up (another field had it): it will not
        // move, so this field is revealed here, as a browser scrolls a
        // newly focused field into view.
        if let p = presenter, p.keyboardInset > 0 { if ExactEnv.agentMode { p.reveal(self) } else { UIView.animate(withDuration: 0.25) { p.reveal(self) } } }
        if handlers.contains("focus") { presenter?.focus(id) }
    }
    func textFieldDidEndEditing(_ textField: UITextField) { if presenter?.editing === self { presenter?.editing = nil }; if handlers.contains("blur") { presenter?.blur(id) } }
}
#endif
