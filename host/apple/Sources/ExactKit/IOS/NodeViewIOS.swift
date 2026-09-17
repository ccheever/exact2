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
    override func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
        if gesture === panGestureRecognizer {
            let velocity = panGestureRecognizer.velocity(in: self)
            let location = panGestureRecognizer.location(in: self)
            let translation = panGestureRecognizer.translation(in: self)
            var view = hitTest(CGPoint(x: location.x - translation.x, y: location.y - translation.y), with: nil)
            // CSS intersects touch-action from the hit element through the
            // scroll container. It governs initial direction, not reversal.
            while let current = view {
                if let node = current as? NodeView, !node.allowsTouchPan(velocity) { return false }
                if current === self { break }
                view = current.superview
            }
            if let owner = superview as? NodeView, !owner.allowsTouchPan(velocity) { return false }
        }
        return super.gestureRecognizerShouldBegin(gesture)
    }
    /// A touch that no node took — nothing focusable, nothing pressable —
    /// ends the editing, as a tap on a page's blank ground blurs the field
    /// and sends the keyboard away (LLP 1008 §9).
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        // An inert button can still retain focus, for example between the
        // two taps of a double-tap recognizer. Its unhandled touch is not
        // blank ground. Check before forwarding to enclosing scroll views.
        var target = touches.first?.view
        while let view = target {
            if let node = view as? NodeView {
                if node.presenter?.contextRetainsFocus(node) == true { return }
                break
            }
            target = view.superview
        }
        super.touchesEnded(touches, with: event)
        // A blur is the session's (LLP 1035.001 D5): its viewport's, never the
        // window's — found through the nearest node above a nested scroller;
        // the viewport itself has none above it and is its own.
        if let t = touches.first, bounds.contains(t.location(in: self)) {
            var viewport: UIView = self
            var above = superview
            while let current = above {
                if let node = current as? NodeView, let owned = node.presenter?.viewport { viewport = owned; break }
                above = current.superview
            }
            viewport.endEditing(true)
        }
    }
}

final class NodeView: UIView, UITextViewDelegate, UITextFieldDelegate, UIScrollViewDelegate, UIGestureRecognizerDelegate {
    let id: UInt32
    let firstDraw: () -> Void
    let kind: String
    weak var textParent: NodeView?
    var textChildren: [NodeView] = []
    var cachedTextSpec: Spec?
    var cachedTextLayout: (width: CGFloat, paragraph: Paragraph)?
    var props: [String: String] = [:]
    var style: [String: Any] = [:]
    var clipPath: CGPath?
    var handlers: Set<String> = [] {
        didSet {
            updateContextGestures()
            updateSwipeGesture()
            updateMaterial()
            if handlers.contains("hover"), hoverRecognizer == nil {
                let g = UIHoverGestureRecognizer(target: self, action: #selector(hovering(_:)))
                // Hover observes pointer movement; it must never hold or cancel
                // finger events while a node is removed by a URL replacement.
                g.delaysTouchesBegan = false
                g.delaysTouchesEnded = false
                g.cancelsTouchesInView = false
                addGestureRecognizer(g)
                hoverRecognizer = g
            }
        }
    }
    var swipeRecognizer: UIPanGestureRecognizer?
    var swipeArmed = false
    var swipeHold: SwipeHold?
    var heightRecognizer: UIPanGestureRecognizer?
    var heightHold: HeightDragHold?
    var heightOrigin = 0.0
    var transformRecognizer: UIPanGestureRecognizer?
    var transformHold: TransformDragHold?
    var transformOrigin = CGPoint.zero
    var swipeOrigin = 0.0
    lazy var swipeFeedback = UISelectionFeedbackGenerator()
    func allowsTouchPan(_ velocity: CGPoint) -> Bool {
        let action = style["touch_action"] as? String ?? "auto"
        if action == "auto" || action == "manipulation" { return true }
        let values = action.split(separator: " ")
        if abs(velocity.x) > abs(velocity.y) {
            return values.contains("pan-x") || values.contains(velocity.x < 0 ? "pan-right" : "pan-left")
        }
        return values.contains("pan-y") || values.contains(velocity.y < 0 ? "pan-down" : "pan-up")
    }
    func updateSwipeGesture() {
        if handlers.contains("swiperight"), swipeRecognizer == nil {
            let gesture = UIPanGestureRecognizer(target: self, action: #selector(swiping(_:)))
            gesture.maximumNumberOfTouches = 1
            gesture.delegate = self
            addGestureRecognizer(gesture)
            swipeRecognizer = gesture
        } else if !handlers.contains("swiperight"), let gesture = swipeRecognizer {
            let prior = swipeHold; swipeHold = nil
            DispatchQueue.main.async { prior?.cancel() }
            removeGestureRecognizer(gesture)
            swipeRecognizer = nil
        }
    }
    override func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
        if gesture === transformRecognizer {
            return SwipeInput.allows(self) && presenter?.transformBindings[id]?.target != nil
        }
        if gesture === heightRecognizer, let pan = gesture as? UIPanGestureRecognizer {
            let velocity = pan.velocity(in: window)
            return SwipeInput.allows(self) && abs(velocity.y) > abs(velocity.x)
                && presenter?.heightBindings[id]?.target != nil
        }
        if gesture === swipeRecognizer, let pan = gesture as? UIPanGestureRecognizer {
            let velocity = pan.velocity(in: window)
            let start = pan.location(in: window).x - pan.translation(in: window).x
            return !disabled && start >= 20 && SwipeRecognition.accepts(x: Double(velocity.x), y: Double(velocity.y), presentedX: Double(translate.x)) && !allowsTouchPan(velocity)
        }
        return super.gestureRecognizerShouldBegin(gesture)
    }
    @objc func swiping(_ gesture: UIPanGestureRecognizer) {
        let translation = Double(gesture.translation(in: window).x)
        switch gesture.state {
        case .began:
            swipeHold?.cancel()
            swipeOrigin = translation
            swipeHold = SwipeHold(self)
        case .changed:
            guard let hold = swipeHold else { return }
            let delta = translation - swipeOrigin
            guard hold.move(delta) else { hold.cancel(); swipeHold = nil; return }
            let armed = hold.mapping.value(delta) >= 64
            if armed != swipeArmed { swipeFeedback.selectionChanged(); swipeArmed = armed }
        case .ended, .cancelled, .failed:
            let hold = swipeHold; swipeHold = nil; swipeArmed = false
            hold?.finish(displacement: translation - swipeOrigin,
                fingerVelocity: Double(gesture.velocity(in: window).x), cancel: gesture.state != .ended)
        default: break
        }
    }
    var contextRecognizer: UILongPressGestureRecognizer?
    var doubleRecognizer: UITapGestureRecognizer?
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        // A nested editor owns its selection gestures, including read-only
        // text. A containing bubble's reply/Tapback recognizers must yield.
        var hit = touch.view
        while let current = hit, current !== self {
            if current is UITextView || current is UITextField { return false }
            if (gestureRecognizer === heightRecognizer || gestureRecognizer === transformRecognizer), current is UIScrollView { return false }
            hit = current.superview
        }
        return true
    }
    func updateContextGestures() {
        if handlers.contains("contextmenu"), contextRecognizer == nil {
            let g = UILongPressGestureRecognizer(target: self, action: #selector(openContext(_:)))
            g.delegate = self
            // Recognition and scroll arbitration remain UIKit's.
            g.delaysTouchesEnded = false
            addGestureRecognizer(g)
            contextRecognizer = g
        }
        if !handlers.contains("contextmenu"), let g = contextRecognizer {
            removeGestureRecognizer(g)
            contextRecognizer = nil
        }
        if handlers.contains("dblclick"), doubleRecognizer == nil {
            let g = UITapGestureRecognizer(target: self, action: #selector(doubleClicked(_:)))
            g.delegate = self
            g.numberOfTapsRequired = 2
            g.delaysTouchesEnded = false
            addGestureRecognizer(g)
            doubleRecognizer = g
        }
        if !handlers.contains("dblclick"), let g = doubleRecognizer {
            removeGestureRecognizer(g)
            doubleRecognizer = nil
        }
    }
    @objc func openContext(_ gesture: UILongPressGestureRecognizer) {
        guard gesture.state == .began, !disabled else { return }
        presenter?.contextmenu(id)
    }
    @objc func doubleClicked(_ gesture: UITapGestureRecognizer) {
        guard gesture.state == .ended, !disabled else { return }
        presenter?.dblclick(id)
    }
    var hoverRecognizer: UIHoverGestureRecognizer?
    var translate = CGPoint.zero
    var scale: CGFloat = 1
    var rotate: CGFloat = 0
    var contextTransform = CGAffineTransform.identity
    weak var presenter: Presenter?
    var textArea: UITextView?
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
    var symbolView: UIImageView?
    var symbolKey: String?
    var symbolRefusal: String?
    var image: UIImage?
    var raster: NativeRasterLease?
    var imageSource: String?
    var loadGeneration = 0
    /// The native swipe cell supplies the row surface while this view is mounted in it.
    var nativeSwipeBody = false
    var pressed = false
    var disabled: Bool { props["disabled"] == "true" }
    /// HTML inertness covers the subtree, including direct agent activation.
    var inert: Bool {
        var ancestor: UIView? = self
        while let view = ancestor {
            if (view as? NodeView)?.props["inert"] == "true" { return true }
            ancestor = view.superview
        }
        return false
    }
    /// Images loaded since launch (smoke reporting).
    /// The session's text engine (LLP 1031 D12: the catalog is the session's).
    var text: TextEngine? { presenter?.session?.text }
    var canvases: Canvases? { presenter?.session?.canvases }

    /// A node with focus, blur, or key handlers takes the focus (an input's
    /// field does by itself): the web's rule that only a focusable element
    /// hears these. Keys come from a hardware keyboard (`pressesBegan`).
    override var canBecomeFirstResponder: Bool { !disabled && !inert && field == nil && textArea == nil && !handlers.isDisjoint(with: ["focus", "blur", "key"]) }
    override func becomeFirstResponder() -> Bool {
        guard !disabled, !inert else { return false }
        let ok = super.becomeFirstResponder()
        if ok { presenter?.collections.pinsChanged() }
        if ok, handlers.contains("focus") { presenter?.focus(id) }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { presenter?.collections.pinsChanged() }
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
    func textFieldShouldBeginEditing(_ textField: UITextField) -> Bool { !disabled && !inert }
    func textViewShouldBeginEditing(_ textView: UITextView) -> Bool { !disabled && !inert }

    func textField(_ textField: UITextField, shouldChangeCharactersIn range: NSRange, replacementString string: String) -> Bool {
        guard !disabled, !inert, props["editable"] != "false" else { return false }
        if props["emojiPicker"] == "true" {
            if EmojiSelection.accepts(string), handlers.contains("change") { presenter?.change(id, string) }
            return false
        }
        return true
    }

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

    /// One bounded, session-owned pipeline. Replacement keeps the old raster
    /// and original intrinsic geometry until a matching new backing is accepted.
    func loadImage(_ source: String) {
        let previousSource = imageSource, previousGeneration = loadGeneration
        imageSource = source
        loadGeneration += 1
        if source.hasPrefix("symbol:") { presenter?.session?.rasters.cancel(id); raster = nil; updateSymbol(); return }
        clearSymbol(); image = nil
        guard let session = presenter?.session else { return }
        if !session.rasters.load(self, source: source, resolver: session.app.resolver) {
            imageSource = previousSource; loadGeneration = previousGeneration
        }
    }

    func acceptRaster(_ lease: NativeRasterLease, generation: Int) {
        guard loadGeneration == generation, let presenter, presenter.views[id] === self else { return }
        raster = lease
        presenter.intrinsic(id, lease.image.naturalSize)
        self.setNeedsDisplay()
        if let c = canvasAbove { c.needsCapture = true; canvases?.scheduleCapture() }
    }

    // A symbol's box is Exact's; UIKit renders its glyph, including pixel alignment.
    func clearSymbol() {
        symbolView?.removeFromSuperview(); symbolView = nil; symbolKey = nil
    }
    func updateSymbol() {
        guard kind == "image", let source = imageSource, source.hasPrefix("symbol:") else { return }
        isAccessibilityElement = false
        let name = props["symbolName"] ?? "", points = number("font_size", 16)
        let weights: [UIImage.SymbolWeight] = [.ultraLight, .thin, .light, .regular, .medium, .semibold, .bold, .heavy, .black]
        let index = min(8, max(0, Int((number("font_weight", 400) / 100).rounded()) - 1))
        let key = "\(source):\(name):\(points):\(index)"
        if symbolKey != key {
            symbolKey = key; loadGeneration += 1
            let generation = loadGeneration
            image = name.isEmpty || points <= 0 ? nil : UIImage(systemName: name, withConfiguration: UIImage.SymbolConfiguration(pointSize: points, weight: weights[index]))
            if name.isEmpty, symbolRefusal != source { symbolRefusal = source; presenter?.session?.log("image \(source) refused: unknown symbol role") }
            if !name.isEmpty { symbolRefusal = nil }
            let leaf = symbolView ?? UIImageView()
            if symbolView == nil { symbolView = leaf; addSubview(leaf) }
            leaf.image = image; leaf.isAccessibilityElement = false; leaf.isUserInteractionEnabled = false
            let size = image?.size
            DispatchQueue.main.async { [weak self] in
                guard let self, self.loadGeneration == generation, let presenter = self.presenter,
                      presenter.views[self.id] === self else { return }
                presenter.intrinsic(self.id, size)
            }
        }
        symbolView?.tintColor = color("tint_color", .black)
        layoutSymbol()
    }
    func layoutSymbol() {
        guard let leaf = symbolView else { return }
        let uniform = number("border_width")
        let content = bounds.insetBy(left: number("border_width_left", uniform) + number("padding_left"), top: number("border_width_top", uniform) + number("padding_top"), right: number("border_width_right", uniform) + number("padding_right"), bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
        leaf.frame = content; leaf.clipsToBounds = true
        switch style["object_fit"] as? String ?? "fill" {
        case "contain": leaf.contentMode = .scaleAspectFit
        case "cover": leaf.contentMode = .scaleAspectFill
        case "none": leaf.contentMode = .center
        case "scale-down":
            let size = image?.size ?? .zero
            leaf.contentMode = size.width <= content.width && size.height <= content.height ? .center : .scaleAspectFit
        default: leaf.contentMode = .scaleToFill
        }
        let path = roundedPath(in: bounds).cgPath
        var transform = CGAffineTransform(translationX: -content.minX, y: -content.minY)
        let mask = CAShapeLayer(); mask.path = path.copy(using: &transform); leaf.layer.mask = mask
    }

    /// The view is gone: no load in flight may report for it.
    func forget() {
        let previousTransform = transformHold; transformHold = nil
        DispatchQueue.main.async { previousTransform?.cancel() }
        let previousHeight = heightHold; heightHold = nil
        DispatchQueue.main.async { previousHeight?.cancel() }
        let prior = swipeHold; swipeHold = nil
        DispatchQueue.main.async { prior?.cancel() }
        textParent?.textChildren.removeAll { $0 === self }
        textParent = nil
        textChildren.removeAll()
        invalidateText()
        loadGeneration += 1
        presenter?.session?.rasters.cancel(id)
        raster = nil
        imageSource = nil
        clearSymbol()
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
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (node: NodeView, _: UITraitCollection) in
            node.paragraphOwner.invalidateText()
            node.paragraphOwner.setNeedsDisplay()
            node.applyStyle(node.style)
            if let presenter = node.presenter, node.superview === presenter.root { presenter.paintCanvas() }
        }
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
        if kind == "textarea" { makeTextArea() }
        if kind == "input" {
            let f = TextField(frame: .zero)
            f.owner = self
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

    override func didMoveToWindow() {
        super.didMoveToWindow()
        presenter?.transformGeometry.changed()
        if window != nil { presenter?.flushPendingFocus() }
    }

    /// Glass content participates in UIKit's interactive effect. Other
    /// materials remain background siblings of the authored children.
    var container: UIView { scroll ?? overlay ?? (materialKind == "glass" ? materialView?.contentView : nil) ?? self }

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

    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        presenter?.collections.userIntent(id)
        retainedScrollTop = nil
    }

    /// A scroll under a canvas repaints it (LLP 1014 D4 c).
    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        presenter?.collections.changed(id, user: true)
        presenter?.transformGeometry.changed()
        repaintThrough()
        // User scrolling is already a coherent position. Deliver before the
        // frame paints so authored scroll-linked geometry cannot lag a frame.
        // Layout-generated offsets still coalesce after the batch completes.
        if presenter?.applying == false && !dispatchingScrollEvent &&
            (scrollView.isTracking || scrollView.isDecelerating) {
            sendScrollEvent()
        } else { queueScrollEvent() }
    }
    private var scrollEventQueued = false
    private var lastScrollEvent = CGPoint.zero
    private var dispatchingScrollEvent = false
    private func sendScrollEvent() {
        guard handlers.contains("scroll"), let point = scroll?.contentOffset,
              point != lastScrollEvent, presenter?.views[id] === self,
              hasScrollLayoutBox else { return }
        lastScrollEvent = point
        dispatchingScrollEvent = true
        defer { dispatchingScrollEvent = false }
        presenter?.scroll(id, Double(point.x), Double(point.y))
    }
    private func queueScrollEvent() {
        guard handlers.contains("scroll"), !scrollEventQueued else { return }
        scrollEventQueued = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.scrollEventQueued = false
            self.sendScrollEvent()
        }
    }

    /// CSS's admitted `x mandatory` / `start` scroll snap. UIKit supplies
    /// the projected resting offset and owns the resulting deceleration.
    func scrollViewWillEndDragging(_ scrollView: UIScrollView, withVelocity velocity: CGPoint, targetContentOffset: UnsafeMutablePointer<CGPoint>) {
        guard (style["scroll_snap_type"] as? String) == "x mandatory" else { return }
        let maximum = max(0, scrollView.contentSize.width - scrollView.bounds.width)
        var positions: [CGFloat] = []
        func visit(_ view: UIView) {
            for case let node as NodeView in view.subviews where !node.isHidden {
                if (node.style["scroll_snap_align"] as? String) == "start" {
                    let rect = node.convert(node.bounds, to: scrollView)
                    // A snap area wider than the viewport can be explored
                    // freely while it covers the viewport (CSS Snap §5.2.2).
                    let start = min(maximum, max(0, rect.minX))
                    let end = min(maximum, max(start, rect.maxX - scrollView.bounds.width))
                    positions.append(min(end, max(start, targetContentOffset.pointee.x)))
                }
                // A nested scroll container captures its own snap areas.
                if node.scroll == nil && (node.style["scroll_snap_type"] as? String ?? "none") == "none" { visit(node.container) }
            }
        }
        visit(scrollView)
        if let nearest = positions.min(by: { abs($0 - targetContentOffset.pointee.x) < abs($1 - targetContentOffset.pointee.x) }) {
            targetContentOffset.pointee.x = nearest
        }
    }

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
        if let clipPath, !clipPath.contains(point) { return nil }
        if props["swipeIndicator"] == "true" { return nil }
        // UIKit's default rejects a view when alpha is near zero. CSS opacity
        // changes painting, not hit participation, so walk the ordinary
        // subtree ourselves without consulting alpha.
        func ordinary() -> UIView? {
            guard !isHidden, isUserInteractionEnabled else { return nil }
            // CSS visible overflow remains hit-testable. A row in a horizontal
            // scroll can paint children beyond its own width; rejecting the
            // parent box first made those visible choices impossible to tap.
            let outsideX = point.x < bounds.minX || point.x > bounds.maxX
            let outsideY = point.y < bounds.minY || point.y > bounds.maxY
            if outsideX && (style["overflow_x"] as? String ?? "visible") != "visible" { return nil }
            if outsideY && (style["overflow_y"] as? String ?? "visible") != "visible" { return nil }
            for child in subviews.reversed() {
                if child === materialView, materialKind == "glass", let contentView = materialView?.contentView {
                    // The effect's UIKit bounds check must not hide authored
                    // children in CSS visible overflow. They remain descendants
                    // of the effect, so its recognizers still see their touches.
                    for content in contentView.subviews.reversed() where content is NodeView {
                        if let hit = content.hitTest(convert(point, to: content), with: event) { return hit }
                    }
                }
                if let hit = child.hitTest(convert(point, to: child), with: event) { return hit }
            }
            return bounds.contains(point) ? self : nil
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

    // Resolve the same light-dark() wire value as the shared paragraph builder.
    // @ref LLP 1034 D1/D2
    var drawsDark: Bool { traitCollection.userInterfaceStyle == .dark }
    func channels(_ key: String, dark: Bool? = nil) -> [Double]? {
        switch style[key] {
        case let c as [Double] where c.count == 4: return c
        case let pair as [[Double]] where pair.count == 2:
            let half = (dark ?? drawsDark) ? pair[1] : pair[0]
            return half.count == 4 ? half : nil
        default: return nil
        }
    }
    func color(_ key: String, _ fallback: UIColor) -> UIColor {
        guard let c = channels(key) else { return fallback }
        return TextEngine.color(c)
    }
    func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
        if let n = style[key] as? Double { return CGFloat(n) }
        return fallback
    }

    /// The input's content box: padding and border sit on the node, the
    /// field is the text inside — CSS's rule, so a placeholder lines up
    /// with a native one.
    func contentBox() -> CGRect {
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

    // display:none removes the CSS box, but retains its stored scroll position.
    // UIKit/AppKit collapse the native extent; keep that transient reset out of
    // scroll events and restore only when the box returns.
    private var beforeLayoutScroll: CGPoint?
    private var hiddenScroll: CGPoint?
    private var hasScrollLayoutBox: Bool {
        var ancestor: UIView? = self
        while let current = ancestor {
            if let node = current as? NodeView, node.style["display"] as? String == "none" { return false }
            ancestor = current.superview
        }
        return true
    }

    var followedScroll: (top: CGFloat, end: Bool)?
    var readingAnchors: [(node: NodeView, y: CGFloat)] = []
    weak var activeReadingAnchor: NodeView?
    var anchoredScrollTop: CGFloat?
    private var retainedScrollTop: CGFloat?
    func captureScrollPosition() {
        beforeLayoutScroll = scroll?.contentOffset
        followedScroll = nil
        readingAnchors.removeAll(keepingCapacity: true)
        guard props["scrollFollowEnd"] == "true", let sv = scroll else {
            activeReadingAnchor = nil; anchoredScrollTop = nil; retainedScrollTop = nil; return
        }
        let maximum = max(-sv.adjustedContentInset.top, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height)
        // A retained route can gain height when another route hides the keyboard.
        // That clamp is not the reader choosing the end. Keep its intended offset
        // until the returning viewport can fit it, or the reader scrolls again.
        if anchoredScrollTop != sv.contentOffset.y { retainedScrollTop = nil }
        followedScroll = (retainedScrollTop ?? sv.contentOffset.y,
                          retainedScrollTop == nil && sv.contentOffset.y >= maximum - 1)
        guard let followedScroll, !followedScroll.end, followedScroll.top > -sv.adjustedContentInset.top else { return }
        // A scroll by the reader invalidates the prior choice. Anchoring's
        // own adjustment does not: keep the same surviving row across batches.
        if anchoredScrollTop == sv.contentOffset.y, let node = activeReadingAnchor,
           presenter?.views[node.id] === node, node.isDescendant(of: sv), !node.isHidden {
            let rect = node.convert(node.bounds, to: sv)
            if rect.width > 0, rect.height > 0, rect.intersects(sv.bounds) { readingAnchors.append((node, rect.minY)) }
        }
        // Prefer the first fully visible box; descend into a partially visible
        // one. This keeps a message stable when rows above it change height.
        // Coordinates are in the scroll view's content space, not the window.
        func anchors(in view: UIView) {
            for case let node as NodeView in view.subviews where !node.isHidden {
                let rect = node.convert(node.bounds, to: sv)
                guard rect.width > 0, rect.height > 0, rect.intersects(sv.bounds) else { continue }
                if !sv.bounds.contains(rect) { anchors(in: node.container) }
                readingAnchors.append((node, rect.minY))
            }
        }
        // Keep later visible candidates too: a deleted anchor cannot hold the
        // reader's position, but the next surviving message still can.
        anchors(in: sv)
    }
    func restoreScrollPosition() {
        defer { followedScroll = nil; readingAnchors.removeAll(keepingCapacity: true) }
        guard props["scrollFollowEnd"] == "true", let sv = scroll else { return }
        let minimum = -sv.adjustedContentInset.top
        let maximum = max(minimum, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height)
        let prior = followedScroll ?? (top: maximum, end: true)
        var top = prior.top
        activeReadingAnchor = nil
        if !prior.end, let anchor = readingAnchors.first(where: {
            presenter?.views[$0.node.id] === $0.node && $0.node.isDescendant(of: sv) &&
                !$0.node.isHidden && $0.node.bounds.height > 0
        }) {
            activeReadingAnchor = anchor.node
            top += anchor.node.convert(anchor.node.bounds, to: sv).minY - anchor.y
        }
        let y = prior.end ? maximum : min(maximum, max(minimum, top))
        let inactive = window == nil || presenter?.navigation.isInactiveRoute(containing: self) == true
        retainedScrollTop = !prior.end && top > maximum && (inactive || retainedScrollTop != nil) ? top : nil
        if sv.contentOffset.y != y { sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: y), animated: false) }
        // UIKit quantizes the assigned offset. Compare its actual stored value
        // next time so that rounding cannot masquerade as a reader's scroll.
        anchoredScrollTop = sv.contentOffset.y
    }

    func applyPendingScroll() {
        defer { pendingScrollTop = nil; pendingScrollLeft = nil }
        guard let sv = scroll else { return }
        if pendingScrollTop != nil { retainedScrollTop = nil }
        guard hasScrollLayoutBox else {
            if hiddenScroll == nil { hiddenScroll = beforeLayoutScroll ?? .zero }
            return
        }
        let i = sv.adjustedContentInset
        if let saved = hiddenScroll {
            hiddenScroll = nil
            let target = CGPoint(
                x: min(max(saved.x, -i.left), max(-i.left, sv.contentSize.width + i.right - sv.bounds.width)),
                y: min(max(saved.y, -i.top), max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height)))
            if sv.contentOffset != target { sv.setContentOffset(target, animated: false) }
        }
        guard pendingScrollTop != nil || pendingScrollLeft != nil else { return }
        let y = pendingScrollTop.map { CGFloat($0) == sv.contentOffset.y ? sv.contentOffset.y : min(max(CGFloat($0), -i.top), max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height)) } ?? sv.contentOffset.y
        let x = pendingScrollLeft.map { CGFloat($0) == sv.contentOffset.x ? sv.contentOffset.x : min(max(CGFloat($0), -i.left), max(-i.left, sv.contentSize.width + i.right - sv.bounds.width)) } ?? sv.contentOffset.x
        let target = CGPoint(x: x, y: y)
        if sv.contentOffset != target { sv.setContentOffset(target, animated: false) }
    }
    var materialView: UIVisualEffectView?
    var materialKind: String?
    var materialInteractive = false
    func updateMaterial() {
        let kind = props["backgroundMaterial"]
        let supported = kind == "ultra-thin" || kind == "glass"
        let interactive = kind == "glass" && handlers.contains("press") && !disabled
        if materialKind != (supported ? kind : nil) {
            let children = container.subviews.compactMap { $0 as? NodeView }
            materialView?.removeFromSuperview()
            materialView = nil
            materialKind = nil
            if supported {
                let effect = UIVisualEffectView()
                effect.isUserInteractionEnabled = kind == "glass"
                effect.frame = bounds
                effect.autoresizingMask = [.flexibleWidth, .flexibleHeight]
                insertSubview(effect, at: 0)
                materialView = effect
                materialKind = kind
            }
            for (index, child) in children.enumerated() { container.insertSubview(child, at: index) }
        }
        guard let materialView else { return }
        if materialView.effect == nil || materialInteractive != interactive {
            let visual: UIVisualEffect
            if kind == "glass", #available(iOS 26.0, *) {
                let glass = UIGlassEffect(style: .regular)
                glass.isInteractive = interactive
                visual = glass
            } else {
                visual = UIBlurEffect(style: .systemUltraThinMaterial)
            }
            materialView.effect = visual
            materialInteractive = interactive
        }
        let radius = number("border_radius", number("border_radius_top_left"))
        if #available(iOS 26.0, *) {
            materialView.cornerConfiguration = .corners(radius: .fixed(Double(radius)))
        } else {
            materialView.layer.cornerRadius = radius
            materialView.clipsToBounds = true
        }
    }
    var pendingScrollLeft: Double?
    var pendingScrollTop: Double?
    func applyProps(set: [String: String], clear: [String]) {
        if clear.contains("scrollLeft") { pendingScrollLeft = nil }
        if let raw = set["scrollLeft"], let left = Double(raw), left.isFinite { pendingScrollLeft = left }
        if clear.contains("scrollTop") { pendingScrollTop = nil }
        if let raw = set["scrollTop"], let top = Double(raw), top.isFinite { pendingScrollTop = top }
        for k in clear { props.removeValue(forKey: k) }
        for (k, v) in set { props[k] = v }
        if set["inert"] != nil || clear.contains("inert") {
            let ownInert = props["inert"] == "true"
            if ownInert { endEditing(true) }
            isUserInteractionEnabled = !ownInert
            accessibilityElementsHidden = ownInert
        }
        updateKeyboardDismissal()
        updateMaterial()
        applyTextArea()
        if let f = field {
            f.tintColor = props["emojiPicker"] == "true" ? .clear : nil
            if (set["emojiPicker"] != nil || clear.contains("emojiPicker")), f.isFirstResponder { f.reloadInputViews() }
            if let v = props["value"], f.text != v { f.text = v }
            applyPlaceholder(f)
            // The web's `type` and `inputmode`, as UIKit spells them.
            let type = props["type"] ?? "text"
            f.isSecureTextEntry = type == "password"
            f.textContentType = type == "password" ? .password : type == "email" ? .emailAddress : nil
            let traitsChanged = f.autocapitalizationType != inputCapitalization || f.autocorrectionType != inputCorrection || f.spellCheckingType != inputSpellChecking
            f.autocapitalizationType = inputCapitalization
            f.autocorrectionType = inputCorrection
            f.spellCheckingType = inputSpellChecking
            if traitsChanged, f.isFirstResponder { f.reloadInputViews() }
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
        if kind == "button" {
            isAccessibilityElement = true
            accessibilityTraits.insert(.button)
            if props["accessibilitySelected"] == "true" { accessibilityTraits.insert(.selected) }
            else { accessibilityTraits.remove(.selected) }
        }
        if kind == "image", let src = props["imageSource"], src != imageSource { loadImage(src) }
        if kind == "image", props["imageSource"] == nil, imageSource != nil { loadGeneration += 1; presenter?.session?.rasters.cancel(id); raster = nil; imageSource = nil; clearSymbol(); image = nil; presenter?.intrinsic(id, nil) }
        if kind == "iframe" { presenter?.session?.webviews.update(self) }
        setNeedsDisplay()
    }

    func updateKeyboardDismissal() {
        switch props["keyboardDismissMode"] {
        case "interactive": scroll?.keyboardDismissMode = .interactive
        case "on-drag": scroll?.keyboardDismissMode = .onDrag
        default: scroll?.keyboardDismissMode = .none
        }
    }
    func applyStyle(_ s: [String: Any]) {
        style = s
        updateSymbol()
        clipPath = ClipPath.path(s["clip_path"])
        layer.mask = ClipPath.mask(clipPath)
        updateMaterial()
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
        scroll?.decelerationRate = (s["scroll_snap_type"] as? String) == "x mandatory" ? .fast : .normal
        scroll?.scrollsX = ox == "scroll"
        scroll?.scrollsY = oy == "scroll"
        // LLP 1008 §9: a vertical scroll container keeps elastic boundary feedback
        // even when its content fits (for example a short conversation inbox).
        scroll?.alwaysBounceVertical = oy == "scroll"
        // UIKit's default indicator is already thin. CSS permits `thin`
        // to match `auto` on such platforms; `none` only hides the track.
        let indicators = (s["scrollbar_width"] as? String ?? "auto") != "none"
        scroll?.showsHorizontalScrollIndicator = ox == "scroll" && indicators
        scroll?.showsVerticalScrollIndicator = oy == "scroll" && indicators
        updateKeyboardDismissal()
        fitScroll()
        clipsToBounds = ox == "hidden" || oy == "hidden"
        styleTextArea()
        if let f = field, let t = text {
            f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic")
            f.textColor = color("text_color", .black)
            applyPlaceholder(f)
            f.frame = contentBox()
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
        transform = CGAffineTransform(translationX: translate.x, y: translate.y).rotated(by: rotate * .pi / 180).scaledBy(x: scale, y: scale).concatenating(contextTransform)
    }

    override func layoutSubviews() {
        if let s = presenter?.session, s.firstLayoutMs == nil { s.firstLayoutMs = ExactEnv.wall() }
        super.layoutSubviews()
        if kind == "image" { presenter?.session?.rasters.resized(self) }
        presenter?.collections.changed(id)
        presenter?.transformGeometry.changed()
        if field != nil { field?.frame = contentBox() }
        layoutTextArea()
        layoutSymbol()
    }

    /// CSS reduces overlapping corner radii by one common factor.
    func roundedPath(in rect: CGRect, inset: CGFloat = 0) -> UIBezierPath {
        let names = ["top_left", "top_right", "bottom_right", "bottom_left"]
        var r = names.map { max(0, number("border_radius_" + $0) - inset) }
        let sums = [r[0] + r[1], r[3] + r[2], r[0] + r[3], r[1] + r[2]]
        let edges = [rect.width, rect.width, rect.height, rect.height]
        var factor: CGFloat = 1
        for i in 0..<4 where sums[i] > 0 { factor = min(factor, edges[i] / sums[i]) }
        r = r.map { $0 * factor }
        let p = UIBezierPath()
        p.move(to: CGPoint(x: rect.minX + r[0], y: rect.minY))
        p.addLine(to: CGPoint(x: rect.maxX - r[1], y: rect.minY))
        p.addArc(withCenter: CGPoint(x: rect.maxX-r[1], y: rect.minY+r[1]), radius: r[1], startAngle: -.pi/2, endAngle: 0, clockwise: true)
        p.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY-r[2]))
        p.addArc(withCenter: CGPoint(x: rect.maxX-r[2], y: rect.maxY-r[2]), radius: r[2], startAngle: 0, endAngle: .pi/2, clockwise: true)
        p.addLine(to: CGPoint(x: rect.minX+r[3], y: rect.maxY))
        p.addArc(withCenter: CGPoint(x: rect.minX+r[3], y: rect.maxY-r[3]), radius: r[3], startAngle: .pi/2, endAngle: .pi, clockwise: true)
        p.addLine(to: CGPoint(x: rect.minX, y: rect.minY+r[0]))
        p.addArc(withCenter: CGPoint(x: rect.minX+r[0], y: rect.minY+r[0]), radius: r[0], startAngle: .pi, endAngle: 3 * .pi/2, clockwise: true)
        p.close()
        return p
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
        let path = roundedPath(in: bounds)
        let bg = nativeSwipeBody ? UIColor.clear : color("background_color", .clear)
        if bg.cgColor.alpha > 0 {
            bg.setFill()
            path.fill()
        }
        let borderColor = color("border_color_top", .clear)
        let uniform = number("border_width")
        let top = number("border_width_top", uniform), right = number("border_width_right", uniform)
        let bottom = number("border_width_bottom", uniform), left = number("border_width_left", uniform)
        // A uniform border on a rounded box follows the curve (the web's
        // rule). Four edge rects would square the corners and show as nubs.
        if radius > 0, top > 0, top == right, right == bottom, bottom == left {
            let inset = top / 2
            let stroke = roundedPath(in: bounds.insetBy(dx: inset, dy: inset), inset: inset)
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
        if kind == "image", symbolView == nil, let bitmap = raster?.image {
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
            let rect = RasterGeometry.rect(natural: bitmap.naturalSize, content: content, fit: fit)
            ctx.saveGState()
            path.addClip()
            UIBezierPath(rect: content).addClip()
            ctx.translateBy(x: rect.minX, y: rect.maxY)
            ctx.scaleBy(x: 1, y: -1)
            ctx.draw(bitmap.image, in: CGRect(origin: .zero, size: rect.size))
            ctx.restoreGState()
        }
        if isParagraph {
            // The same paragraph the kernel measured at this width, painted.
            let spec = paragraphSpec()
            if let paragraph = paragraphLayout() { TextEngine.draw(paragraph, spec: spec, in: contentBox(), context: ctx, dirty: rect) }
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
        let inside = touches.first.map { bounds.contains(local($0.location(in: nil))) } ?? false
        if !isFirstResponder && presenter?.contextRetainsFocus(self) != true { presenter?.viewport.endEditing(true) }
        if inside, presenter?.views[id] === self { presenter?.press(id) }
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
        guard let target = activationTarget(at: windowPoint) else { return nil }
        target.presenter?.press(target.id)
        return target
    }
    /// Resolve before focus changes: a keyboard resize can move the control.
    func activationTarget(at windowPoint: CGPoint) -> NodeView? {
        guard !inert else { return nil }
        var v: UIView? = self
        while let cur = v {
            if let n = cur as? NodeView, n.disabled { return nil }
            if let n = cur as? NodeView, n.handlers.contains("press") {
                guard n.bounds.contains(n.local(windowPoint)) else { return nil }
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
        if props["emojiPicker"] == "true", let field {
            let value = field.text ?? ""
            field.text = ""
            if !disabled, handlers.contains("change"), EmojiSelection.accepts(value) { presenter?.change(id, value) }
            return
        }
        if !disabled, handlers.contains("change") { presenter?.change(id, field?.text ?? "") }
    }
    func textFieldDidBeginEditing(_ textField: UITextField) {
        presenter?.collections.pinsChanged()
        presenter?.editing = self
        // The keyboard is already up (another field had it): it will not
        // move, so this field is revealed here, as a browser scrolls a
        // newly focused field into view.
        if let p = presenter, p.keyboardInset > 0 { if ExactEnv.agentFreezes { p.reveal(self) } else { UIView.animate(withDuration: 0.25) { p.reveal(self) } } }
        if handlers.contains("focus") { presenter?.focus(id) }
    }
    func textFieldDidEndEditing(_ textField: UITextField) { presenter?.collections.pinsChanged(); if presenter?.editing === self { presenter?.editing = nil }; if handlers.contains("blur") { presenter?.blur(id) } }
}
#endif
