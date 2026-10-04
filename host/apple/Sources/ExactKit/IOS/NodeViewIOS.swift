// One UIView per kernel node (the UIKit presenter's node — the AppKit
// presenter's shape on UIKit, LLP 1008 §9): nothing flips (UIKit's origin
// is the top-left already); a scroll container is a UIScrollView; a press
// is a touch down and up inside the bounds, and a touch on a node without
// a handler goes up the responder chain as a DOM click bubbles; transforms
// and opacity come from presentation values, about the center. Everything
// a node reaches beyond itself — the text engine, the canvases, the web
// views — it reaches through its presenter's session (LLP 1031 D1).
#if os(iOS) || os(tvOS)
import ImageIO
import UIKit

final class NodeView: UIView, UITextViewDelegate, UITextFieldDelegate, UIScrollViewDelegate, UIGestureRecognizerDelegate {
    /// The kernel's id; a parked view takes a new row's and a new incarnation (`NodePool`, LLP 1068 §4.9).
    var id: UInt32
    var incarnation = NodePool.issue()
    let firstDraw: () -> Void
    let kind: String
    var inlineText: [InlineText] = []
    /// The rarely set fields (`NodeExtrasIOS.swift`); nil until one is set.
    var extras: NodeExtras?
    override class var layerClass: AnyClass { NodeLayer.self }
    /// The box is `draw(_:)`'s to paint: Core Animation cannot say it
    /// (`applyBoxLayer`).
    var boxDrawn = false
    /// A uniform border under the children, where they can reach it.
    var boxBorder: CALayer?
    var textRasterKey: TextRasterKey? { didSet { textRasterWhole = textRasterKey.map { $0.clip == nil } ?? false } }
    /// The key is set and paints the whole paragraph (not a band of it).
    private(set) var textRasterWhole = false
    var textRaster: CGImage?
    var textRasterLayer: CALayer?
    var textRasterFrame = CGRect.zero
    var textRasterScale: CGFloat = 1
    var textRasterReady = false
    var textRasterFailed = false
    var cachedTextSpec: Spec?
    var textLayoutValid = false
    var flowShapes: [TextFlowShape] = []
    var cachedTextLayout: (width: CGFloat, paragraph: Paragraph)?
    var liveText: String?
    var props: [String: String] = [:] { didSet { presenter?.propsChanged(self) } }
    var style: NodeStyle = [:]
    var handlers: Set<String> = [] {
        didSet {
            updateContextGestures()
            updateSwipeGesture()
            updateLayoutPan()
            updateMaterial()
            updateRefresh()
            if handlers.contains("scroll") { needScroll() }
            #if !os(tvOS)
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
            #endif
            video?.update() // the media events the player reports
        }
    }
    func allowsTouchPan(_ velocity: CGPoint) -> Bool {
        let action = style["touch_action"]?.string ?? "auto"
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
            #if !os(tvOS)
            gesture.maximumNumberOfTouches = 1
            #endif
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
        if gesture === layoutPanRecognizer, let pan = gesture as? UIPanGestureRecognizer {
            guard SwipeInput.allows(self) else { return false }
            let action = style["touch_action"]?.string ?? "auto"
            // An explicit native pan axis belongs to the scroller. Keep the
            // existing unqualified pan behavior for auto and manipulation.
            if action == "auto" || action == "manipulation" { return true }
            let velocity = pan.velocity(in: self)
            let direction = velocity == .zero ? pan.translation(in: self) : velocity
            return direction == .zero || !allowsTouchPan(direction)
        }
        if let reorder = reorderShouldBegin(gesture) { return reorder }
        if let transform = transformShouldBegin(gesture) { return transform }
        if gesture === heightRecognizer, let pan = gesture as? UIPanGestureRecognizer {
            let velocity = pan.velocity(in: window), translation = pan.translation(in: window)
            return SwipeInput.allows(self) && HeightDragDirection.accepts(
                velocityX: Double(velocity.x), velocityY: Double(velocity.y),
                translationX: Double(translation.x), translationY: Double(translation.y))
                && presenter?.heightBindings[id]?.target != nil
        }
        if gesture === swipeRecognizer, let pan = gesture as? UIPanGestureRecognizer {
            let velocity = pan.velocity(in: window)
            let start = pan.location(in: window).x - pan.translation(in: window).x
            return !disabled && start >= Gesture.edge && SwipeRecognition.accepts(x: Double(velocity.x), y: Double(velocity.y), presentedX: Double(translate.x)) && !allowsTouchPan(velocity)
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
            let armed = hold.mapping.value(delta) >= Gesture.knee
            #if os(tvOS)
            swipeArmed = armed
            #else
            if armed != swipeArmed { swipeFeedback.selectionChanged(); swipeArmed = armed }
            #endif
        case .ended, .cancelled, .failed:
            let hold = swipeHold; swipeHold = nil; swipeArmed = false
            hold?.finish(displacement: translation - swipeOrigin,
                fingerVelocity: Double(gesture.velocity(in: window).x), cancel: gesture.state != .ended)
        default: break
        }
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        if CanvasInput.owns(touch.view) { return false }
        if stopsAtPress(gestureRecognizer), pressBoundary(touch, presses: gestureRecognizer !== layoutPanRecognizer) { return false } // LLP 1057.001 rule 3
        // A nested editor owns its selection gestures, including read-only
        // text. A containing bubble's reply/Tapback recognizers must yield.
        var hit = touch.view
        while let current = hit, current !== self {
            if current is UITextView || current is UITextField { return false }
            if (gestureRecognizer === heightRecognizer || gestureRecognizer === transformRecognizer || gestureRecognizer === transformContact?.pinch), current is UIScrollView { return false }
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
            // The web's order (LLP 1057.001 §1 rule 5): the second tap still presses.
            g.numberOfTapsRequired = 2; g.delaysTouchesEnded = false; g.cancelsTouchesInView = false
            addGestureRecognizer(g)
            doubleRecognizer = g
        }
        if !handlers.contains("dblclick"), let g = doubleRecognizer {
            removeGestureRecognizer(g)
            doubleRecognizer = nil
        }
        syncPointerRecognizer()
    }
    @objc func openContext(_ gesture: UILongPressGestureRecognizer) {
        guard gesture.state == .began, !disabled else { return }
        presenter?.contextmenu(id)
    }
    @objc func doubleClicked(_ gesture: UITapGestureRecognizer) {
        guard gesture.state == .ended, !disabled else { return } // then after this touch's own press
        DispatchQueue.main.async { [weak self, token = incarnation] in if let self, self.incarnation == token, !self.disabled, self.presenter?.views[self.id] === self { self.presenter?.dblclick(self.id) } }
    }
    var translate = CGPoint.zero
    var scale: CGFloat = 1
    var rotate: CGFloat = 0
    var contextTransform = CGAffineTransform.identity {
        didSet {
            if contextTransform.isIdentity { presenter?.contextNodes.remove(id) }
            else { presenter?.contextNodes.insert(id) }
        }
    }
    weak var presenter: Presenter?
    var scroll: ScrollView? {
        didSet {
            if scroll == nil { presenter?.scrollers.remove(id) }
            else { presenter?.scrollers.insert(id) }
        }
    }
    /// A scroll container's content extent (the `content` op), before the
    /// axes that do not scroll are held to the box.
    var content = CGSize.zero
    var placementHidden: Bool {
        get { extras?.placementHidden ?? false }
        set {
            let oldValue = placementHidden
            if newValue || extras != nil { more.placementHidden = newValue }
            if placementHidden && !oldValue { more.hiddenBeforePlacement = isHidden }
            if placementHidden { isHidden = true }
            else if oldValue { isHidden = extras?.hiddenBeforePlacement ?? false }
            accessibilityElementsHidden = hidesAccessibility
        }
    }
    /// Off the accessibility tree: projected away, inert, or `aria-hidden`.
    private var hidesAccessibility: Bool {
        placementHidden || props["inert"] == "true" || props["accessibilityElementsHidden"] == "true"
    }
    /// An image node's picture, once loaded (decoded off the main thread),
    /// the source it came from, and which load is current: a completion
    /// from an older load, or for a view that was destroyed, is dropped.
    var image: UIImage?
    var raster: NativeRasterLease? { didSet { if raster == nil, let l = imageLayer { l.removeFromSuperlayer(); imageLayer = nil } } }
    /// An image's pixels as a sublayer's contents (`applyImageLayer`).
    var imageLayer: CALayer?
    var imageSource: String?
    var loadGeneration = 0
    var pressed = false { didSet { if pressed != oldValue { pressChanged() } } }
    var press = PressFeedback() // LLP 1061 D2: the feedback `pressed` drives
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
    /// UIKit's focus search finds what UIKit can focus (`FocusSearch`).
    override func didAddSubview(_ subview: UIView) { super.didAddSubview(subview); FocusSearch.joined(subview) }
    override var canBecomeFirstResponder: Bool { !disabled && !inert && field == nil && textArea == nil && (kind == "button" || isNativeButton || canvases?.wantsInput(id) == true || !handlers.isDisjoint(with: ["focus", "blur", "key"])) }
    override func becomeFirstResponder() -> Bool {
        guard !disabled, !inert else { return false }
        let ok = super.becomeFirstResponder()
        if ok { presenter?.collections.pinsChanged() }
        if ok, handlers.contains("focus") { presenter?.focus(id) }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { showFocusRing(false) }
        if ok { presenter?.collections.pinsChanged() }
        if ok { inputCanvas?.canvasInput?.blur() }
        if ok, handlers.contains("blur") { presenter?.blur(id) }
        return ok
    }
    /// A hardware keyboard's Tab and Shift-Tab move the focus through the
    /// sequential order, as macOS's key-view loop does (`Presenter.moveFocus`);
    /// Enter and Space then press (`pressesBegan`). UIKit gives text inputs a
    /// Tab of their own, so these take priority; the web's Tab leaves a
    /// textarea too. A field's or textarea's chain reaches its node's.
    override var keyCommands: [UIKeyCommand]? { (super.keyCommands ?? []) + NodeView.tabCommands }
    static let tabCommands: [UIKeyCommand] = [(UIKeyModifierFlags(), #selector(NodeView.focusNextNode)), (.shift, #selector(NodeView.focusPreviousNode))].map {
        let command = UIKeyCommand(input: "\t", modifierFlags: $0.0, action: $0.1)
        command.wantsPriorityOverSystemBehavior = true
        return command
    }
    @objc func focusNextNode() { presenter?.moveFocus(backward: false) }
    @objc func focusPreviousNode() { presenter?.moveFocus(backward: true) }
    /// The ring a keyboard-focused control shows, as the web's `:focus-visible`
    /// and AppKit's focus ring do: drawn when Tab moved the focus here, never
    /// for a touch, and inside the box so no clip hides it.
    func showFocusRing(_ shown: Bool) {
        guard shown else { focusRing?.removeFromSuperlayer(); focusRing = nil; return }
        let ring = focusRing ?? CAShapeLayer()
        #if os(tvOS)
        // Across a room the ring stands clear of the content: outside the box, padded and rounded.
        ring.path = UIBezierPath(roundedRect: bounds.insetBy(dx: -10, dy: -5), cornerRadius: 12).cgPath
        #else
        ring.path = roundedPath(in: bounds.insetBy(dx: 1.5, dy: 1.5), inset: 1.5).cgPath
        #endif
        ring.fillColor = nil
        #if os(tvOS)
        // tvOS tints white; the ring must read across a room.
        ring.strokeColor = UIColor.systemBlue.cgColor
        ring.lineWidth = 4
        #else
        ring.strokeColor = tintColor.cgColor
        ring.lineWidth = 3
        #endif
        if ring.superlayer !== layer { layer.addSublayer(ring) }
        focusRing = ring
    }
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        #if os(tvOS)
        if remoteSelect(presses, down: true) { return }
        #endif
        let presses=pressedControls(presses,down:true)
        if presses.isEmpty {return}
        if inputCanvas?.canvasInput?.presses(presses, down: true, source: self) == true { return }
        // The focus's `key` handlers and its ancestors' (KeyEvents.swift); an
        // ancestor UIKit passes the presses up to dispatches none again.
        let name = presses.first?.key.map(NodeView.keyName)
        if !disabled, isFirstResponder, let name, presenter?.keyDown(at: self, name) == true { return }
        if !disabled, handlers.contains("press"), let name, ["Enter", " "].contains(name) { presenter?.press(id); return }
        super.pressesBegan(presses, with: event)
    }
    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        #if os(tvOS)
        if remoteSelect(presses, down: false) { return }
        #endif
        let presses=pressedControls(presses,down:false)
        if presses.isEmpty {return}
        if inputCanvas?.canvasInput?.presses(presses, down: false, source: self) != true { super.pressesEnded(presses, with: event) }
    }
    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let presses=pressedControls(presses,down:false)
        if presses.isEmpty {return}
        if inputCanvas?.canvasInput?.presses(presses, down: false, source: self) != true { super.pressesCancelled(presses, with: event) }
    }
    /// A pointer over the node (an iPad's trackpad or mouse; a phone has
    /// none): `hover` in and out.
    override var accessibilityElements: [Any]? {
        get { textAccessibilityChildren() ?? super.accessibilityElements }
        set { super.accessibilityElements = newValue }
    }
    /// A text field's Enter as a key (its characters are its `input`, the
    /// Enter commits its `change`); the editing goes on, as on the web.
    func textFieldShouldBeginEditing(_ textField: UITextField) -> Bool { !disabled && !inert }
    func textViewShouldBeginEditing(_ textView: UITextView) -> Bool { !disabled && !inert }

    func textField(_ textField: UITextField, shouldChangeCharactersIn range: NSRange, replacementString string: String) -> Bool {
        guard !disabled, !inert, props["editable"] != "false" else { return false }
        if props["emojiPicker"] == "true" {
            if EmojiSelection.accepts(string) { presenter?.typed(id, string, input: handlers.contains("input")) }
            return false
        }
        return true
    }

    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        guard !disabled else { return false }
        // The software keyboard's Return is a key the handlers hear first (a
        // hardware one's they heard in `pressesBegan`); prevented, it neither
        // commits nor submits.
        if (textField as? TextField)?.heard != "Enter", presenter?.keyDown(at: self, "Enter") == true { return false }
        presenter?.commitEdit(id, textField.text ?? "", change: handlers.contains("change"))
        // Enter in an input with a `submit` handler is the web's implicit submission.
        if handlers.contains("submit") { presenter?.submit(id) }
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
        if previousSource?.hasPrefix("symbol:") == true { presenter?.queueIntrinsicSize(self, generation: loadGeneration, nil) }
        guard let session = presenter?.session else { return }
        if !session.rasters.load(self, source: source, resolver: session.app.resolver) {
            imageSource = previousSource; loadGeneration = previousGeneration
        }
    }

    /// The lease is this load's: shown, its natural size answered for the
    /// loader to report (`RasterLoader.reconcile`, one report per turn).
    func acceptRaster(_ lease: NativeRasterLease, generation: Int) -> CGSize? {
        guard loadGeneration == generation, let presenter, presenter.views[id] === self else { return nil }
        raster = lease; AnimatedRasters.shared.attach(self)
        self.setNeedsDisplay()
        if let c = canvasAbove { c.needsCapture = true; canvases?.scheduleCapture() }
        return lease.image.naturalSize
    }

    // A symbol's box is Exact's; UIKit renders its glyph, including pixel alignment.
    func clearSymbol() {
        symbolView?.removeFromSuperview(); symbolView = nil; symbolKey = nil; symbolFound = false
    }
    func updateSymbol() {
        guard kind == "image", let source = imageSource, source.hasPrefix("symbol:") else { return }
        isAccessibilityElement = false
        let name = props["symbolName"] ?? "", points = number("font_size", 16)
        let weights: [UIImage.SymbolWeight] = [.ultraLight, .thin, .light, .regular, .medium, .semibold, .bold, .heavy, .black]
        let index = min(8, max(0, Int((number("font_weight", 400) / 100).rounded()) - 1))
        let key = "\(source):\(name):\(points):\(index):\(symbolLookKey)"
        if symbolKey != key {
            symbolKey = key; loadGeneration += 1
            let generation = loadGeneration
            image = name.isEmpty ? nil : symbolImage(name, symbolConfiguration(UIImage.SymbolConfiguration(pointSize: points > 0 ? points : 1, weight: weights[index])))
            symbolFound = image != nil; if points <= 0 { image = nil }
            if name.isEmpty, !source.hasPrefix("symbol:sf/"), symbolRefusal != source { symbolRefusal = source; presenter?.session?.log("image \(source) refused: unknown symbol role") }
            if !name.isEmpty || source.hasPrefix("symbol:sf/") { symbolRefusal = nil }
            let leaf = symbolView ?? UIImageView()
            if symbolView == nil { symbolView = leaf; addSubview(leaf) }
            showSymbol(image, on: leaf); leaf.isAccessibilityElement = false; leaf.isUserInteractionEnabled = false
            presenter?.queueIntrinsicSize(self, generation: generation, (image?.size ?? (points > 0 ? CGSize(width: points, height: points) : nil)))
        }
        symbolView?.tintColor = color("tint_color", .black)
        if let leaf = symbolView { applySymbolEffect(leaf) }
        layoutSymbol()
    }
    func layoutSymbol() {
        guard let leaf = symbolView else { return }
        let uniform = number("border_width")
        let content = bounds.insetBy(left: number("border_width_left", uniform) + number("padding_left"), top: number("border_width_top", uniform) + number("padding_top"), right: number("border_width_right", uniform) + number("padding_right"), bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
        leaf.frame = content; leaf.clipsToBounds = true
        switch style["object_fit"]?.string ?? "fill" {
        case "contain": leaf.contentMode = .scaleAspectFit
        case "cover": leaf.contentMode = .scaleAspectFill
        case "none": leaf.contentMode = .center
        case "scale-down":
            let size = image?.size ?? .zero
            leaf.contentMode = size.width <= content.width && size.height <= content.height ? .center : .scaleAspectFit
        default: leaf.contentMode = .scaleToFill
        }
        // A square box clips nothing the content box does not.
        if cornerRadii(in: bounds).allSatisfy({ $0 == 0 }) { if leaf.layer.mask != nil { leaf.layer.mask = nil }; return }
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
        presenter?.forgetParagraph(self)
        cancelSurfaceControls()
        invalidateText()
        cachedTextLayout = nil
        dropTextRaster()
        loadGeneration += 1
        presenter?.session?.rasters.cancel(id)
        raster = nil
        imageSource = nil
        clearSymbol()
        image = nil
        video?.invalidate(); video = nil
        destroyEmbedded()
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
            node.presenter?.requestTextPublication()
            if let presenter = node.presenter, node.superview === presenter.root { presenter.paintCanvas() }
        }
        // The box's background is the layer's (`applyBoxLayer`), never
        // UIView's: UIKit would reapply its own on a trait change.
        isOpaque = false
        // A frame change repaints at the new width instead of stretching stale pixels.
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
        presenter.leaves.embed(self) // a video's player, an iframe's web view, a module's box (LLP 1068 §5.1)
    }
    required init?(coder: NSCoder) { nil }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
        if window != nil { presenter?.flushPendingFocus() }
    }

    /// Glass content participates in UIKit's interactive effect. Other
    /// materials remain background siblings of the authored children. A
    /// glass group is innermost (`GlassGroup.swift`).
    var container: UIView { glassGroupView?.contentView ?? baseContainer }

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

    /// The end the reader was following moved during their interaction:
    /// settle there when it ends, as a browser's scroll anchoring does.
    func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        if !decelerate { followEndIfOwed() }
    }
    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) { followEndIfOwed() }
    func scrollViewDidEndScrollingAnimation(_ scrollView: UIScrollView) {
        followingEndAnimated = false
        // Whether it ended at the running animation's target, clamped to
        // the content as it is now (`CollectionHost.animationEnded`).
        let atTarget = presenter?.collections.animationTargets[id].map { t -> Bool in
            let o = scrollView.contentOffset, i = scrollView.adjustedContentInset
            let x = min(max(t.x, -i.left), max(-i.left, scrollView.contentSize.width + i.right - scrollView.bounds.width))
            let y = min(max(t.y, -i.top), max(-i.top, scrollView.contentSize.height + i.bottom - scrollView.bounds.height))
            return abs(o.x - x) + abs(o.y - y) <= 1
        }
        presenter?.collections.animationEnded(id, atTarget: atTarget)
    }
    private func followEndIfOwed() {
        guard followsEndAfterInteraction, let sv = scroll else { return }
        followsEndAfterInteraction = false
        let maximum = max(-sv.adjustedContentInset.top, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height)
        if sv.contentOffset.y >= maximum - 80 { sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: maximum), animated: true) }
        anchoredScrollTop = maximum
    }

    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        followingEndAnimated = false
        presenter?.collections.animationEnded(id, dragging: true)
        presenter?.collections.userIntent(id, travel: true)
        retainedScrollTop = nil
    }

    /// A scroll under a canvas repaints it (LLP 1014 D4 c).
    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        let post = Presenter.signposts.beginInterval("scrolled")
        defer { Presenter.signposts.endInterval("scrolled", post) }
        presenter?.stickies.scrolled(id); presenter?.collections.changed(id, user: true)
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
        presenter?.reaimFixedGradients()
        presenter?.scrollPump.scrolled(self)
        repaintThrough()
        // User scrolling is already a coherent position. Deliver before the
        // frame paints so authored scroll-linked geometry cannot lag a frame.
        // Layout-generated offsets still coalesce after the batch completes.
        if presenter?.applying == false && !dispatchingScrollEvent &&
            (scrollView.isTracking || scrollView.isDecelerating) {
            sendScrollEvent()
        } else { queueScrollEvent() }
    }
    private func sendScrollEvent() {
        guard handlers.contains("scroll"), let point = scroll?.contentOffset,
              point != lastScrollEvent, presenter?.views[id] === self,
              hasScrollLayoutBox else { return }
        lastScrollEvent = point
        dispatchingScrollEvent = true
        defer { dispatchingScrollEvent = false }
        presenter?.scroll(id, Double(point.x), Double(point.y + (scroll.map(scrollTopInset) ?? 0)))
    }
    private func queueScrollEvent() {
        guard handlers.contains("scroll"), !scrollEventQueued else { return }
        scrollEventQueued = true
        DispatchQueue.main.async { [weak self, token = incarnation] in
            guard let self else { return }
            self.scrollEventQueued = false
            if self.incarnation == token { self.sendScrollEvent() }
        }
    }

    /// The direct child of a canvas this node is under, when that child is
    /// placed by the surface: the node whose `placement` maps this subtree.
    var placedAncestor: NodeView? {
        var v: UIView? = self
        while let n = v {
            if let node = n as? NodeView, (node.placement != nil || node.placementHidden) { return node }
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
        // The canvas reached the same way (a placement above it included),
        // then the child's own points, then down to this node.
        let inChild = NodeView.map(inv, canvas.local(windowPoint))
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
        // UIKit's conversion already carries a box in space through its
        // plane (LLP 1077 D8); a hidden back face is the host's to refuse.
        if placedAncestor?.placementHidden == true || hidesBack() { return nil }
        if let clipPath, !clipPath.contains(point, using: clipRule) { return nil }
        if props["swipeIndicator"] == "true" { return nil }
        if isSurfaceControl, !inert, !isHidden, isUserInteractionEnabled, bounds.contains(point) { return self }
        // A touch landing on a native swipe row: its cell mounts now, before
        // UIKit gathers the touch's recognizers, so the cell's swipe sees it.
        if event?.type == .touches, props["swipeContent"] != nil, !isHidden, isUserInteractionEnabled, bounds.contains(point) {
            presenter?.swipeActions.touch(self)
        }
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
            if outsideX && (style["overflow_x"]?.string ?? "visible") != "visible" { return nil }
            if outsideY && (style["overflow_y"]?.string ?? "visible") != "visible" { return nil }
            for child in NodeView.hitOrder(subviews) {
                if child === (glassSlot ?? materialView), Materials.glass(materialKind) || blurHostsChildren, let contentView = materialView?.contentView {
                    // The effect's UIKit bounds check must not hide authored
                    // children in CSS visible overflow. They remain descendants
                    // of the effect, so its recognizers still see their touches.
                    for content in NodeView.hitOrder(contentView.subviews) where content is NodeView || content is GlassGroupView {
                        if let hit = content.hitTest(convert(point, to: content), with: event) { return hit }
                    }
                }
                if let hit = child.hitTest(convert(point, to: child), with: event) { return hit }
            }
            // CSS `pointer-events: none` (inherited): the box is never the
            // target, so a touch goes to what is under it — a header's blur
            // over a list must not stop the list scrolling. A descendant
            // that sets `auto` again is still a target.
            if style["pointer_events"]?.string == "none" { return nil }
            guard bounds.contains(point) else { return nil }
            // Interactive glass (a pressable glass box) answers a finger
            // itself, swelling and lighting under it, only for touches that
            // reach its effect view. Its content view is the target; the
            // touch climbs the responder chain to this node, whose press is
            // unchanged.
            if event?.type == .touches, materialInteractive, Materials.glass(materialKind),
               let contentView = materialView?.contentView {
                return contentView
            }
            return self
        }
        guard let overlay else { return ordinary() }
        let placed = overlay.subviews.compactMap { $0 as? NodeView }.filter { $0.placement != nil || $0.placementHidden }
        guard !placed.isEmpty else { return ordinary() }
        guard !isHidden, isUserInteractionEnabled, bounds.contains(point) else { return nil }
        // Ordinary HUD paints above the captured children, so it hits first.
        let inOverlay = overlay.convert(point, from: self)
        for child in overlay.subviews.reversed() where (child as? NodeView)?.placement == nil && (child as? NodeView)?.placementHidden != true {
            if let hit = child.hitTest(child.convert(inOverlay, from: overlay), with: event) { return hit }
        }
        // Nearest first: what is seen on top is what a tap reaches.
        for child in placed.reversed().sorted(by: { ($0.placement?[9] ?? 0) > ($1.placement?[9] ?? 0) }) {
            guard let h = child.placement, let inv = NodeView.invert(h) else { continue }
            let p = NodeView.map(inv, point)
            guard child.bounds.contains(p) else { continue }
            if let hit = child.hitTest(p, with: event) { return hit }
        }
        return self
    }


    /// The box on screen, through the placement of the placed child this
    /// node is (or is under), for assistive technology — the same box the
    /// agent's `layout` reports.
    override var accessibilityFrame: CGRect {
        get {
            if placedAncestor?.placementHidden == true { return .zero }
            guard placedAncestor?.placement != nil, let window else { return super.accessibilityFrame }
            return UIAccessibility.convertToScreenCoordinates(drawnRect(bounds, in: window), in: window)
        }
        set { super.accessibilityFrame = newValue }
    }

    // Resolve the same light-dark() wire value as the shared paragraph builder.
    // @ref LLP 1034 D1/D2
    var drawsDark: Bool { traitCollection.userInterfaceStyle == .dark }
    func channels(_ key: String, dark: Bool? = nil) -> [Double]? {
        style[key]?.channels(dark: dark ?? drawsDark)
    }
    func color(_ key: String, _ fallback: UIColor) -> UIColor {
        guard let c = channels(key) else { return fallback }
        return TextEngine.color(c)
    }
    /// CSS's used `z-index` (LLP 1074 T1): the row applies to a positioned
    /// box and to a flex or grid item; a static box elsewhere paints in order.
    var usedZIndex: CGFloat {
        let position = style["position_type"]?.string
        if position == "relative" || position == "absolute" || position == "sticky" { return number("z_index") }
        var parent = superview
        while let view = parent, !(view is NodeView) { parent = view.superview }
        let display = (parent as? NodeView)?.style["display"]?.string
        return display == "flex" || display == "grid" ? number("z_index") : 0
    }
    func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
        if let n = style[key]?.number { return CGFloat(n) }
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
    private var hasScrollLayoutBox: Bool {
        var ancestor: UIView? = self
        while let current = ancestor {
            if let node = current as? NodeView, node.style["display"]?.string == "none" { return false }
            ancestor = current.superview
        }
        return true
    }

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
        // An animated follow of the end (below) is still the end, mid-flight.
        followedScroll = (retainedScrollTop ?? sv.contentOffset.y,
                          (retainedScrollTop == nil && sv.contentOffset.y >= maximum - 1) || followingEndAnimated)
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
        let y = Self.followedTop(current: sv.contentOffset.y, minimum: minimum, maximum: maximum, end: prior.end, top: top,
                                 moving: sv.isTracking || sv.isDragging || sv.isDecelerating)
        let inactive = window == nil || presenter?.navigation.isInactiveRoute(containing: self) == true
        retainedScrollTop = !prior.end && top > maximum && (inactive || retainedScrollTop != nil) ? top : nil
        // While the reader's finger is down or the fling is running, an
        // absolute write would cut the pan, the deceleration or the rubber
        // band (a batch every 250 ms yanked a bottom overscroll back to the
        // end, mid-drag). Follow the end once the interaction is over, and
        // keep a surviving row in place by moving the offset by its shift
        // only, without clamping, as UIKit's own contentOffsetAdjustment does.
        if sv.isTracking || sv.isDecelerating {
            if prior.end { followsEndAfterInteraction = true }
            else if top != prior.top { sv.contentOffset.y += top - prior.top }
            anchoredScrollTop = sv.contentOffset.y
            return
        }
        if sv.contentOffset.y != y {
            // `scroll-behavior: smooth`: an end that moved down (a message
            // appended) is followed with UIKit's scroll animation, as a
            // smooth `scrollTop` write is; a shrink or a jump up lands at once.
            let animate = prior.end && y > sv.contentOffset.y && style["scroll_behavior"]?.string == "smooth" && !ExactEnv.agentFreezes && window != nil
            followingEndAnimated = animate
            sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: y), animated: animate)
        }
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
        // A collapsing title's scroller (LLP 1075.003 Stage 3): CSS counts
        // from the bar's bottom, and its end is where the title rests
        // collapsed (UIKit moves the offset by what the title gives up).
        let inset = scrollTopInset(sv), slack = scrollOrigin > 0 ? max(0, i.top - scrollCollapsed) : 0
        let y = pendingScrollTop.map { CGFloat($0) - inset == sv.contentOffset.y ? sv.contentOffset.y : min(max(CGFloat($0) - inset, -i.top), max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height - slack)) } ?? sv.contentOffset.y
        let x = pendingScrollLeft.map { CGFloat($0) == sv.contentOffset.x ? sv.contentOffset.x : min(max(CGFloat($0), -i.left), max(-i.left, sv.contentSize.width + i.right - sv.bounds.width)) } ?? sv.contentOffset.x
        let target = CGPoint(x: x, y: y)
        // `scroll-behavior: smooth` (CSS) animates a prop write, never a
        // reader's own scroll. Under the agent's frozen clock it lands at once
        // (as a modal presents, LLP 1035.003 D5), so `layout` reads the target.
        let smooth = style["scroll_behavior"]?.string == "smooth" && !ExactEnv.agentFreezes
        if sv.contentOffset != target { sv.setContentOffset(target, animated: smooth) }
    }
    var materialView: UIVisualEffectView? {
        get { extras?.materialView }
        set {
            if newValue != nil || extras != nil { more.materialView = newValue }
            if materialView == nil { presenter?.materialNodes.remove(id) }
            else { presenter?.materialNodes.insert(id) }
        }
    }
    func updateMaterial() {
        defer { syncGlassSlot(); syncGlassGroup(); settleVibrancy() }
        let kind = materialRequest
        let supported = kind != nil
        let interactive = Materials.glass(kind) && handlers.contains("press") && !disabled
        if materialKind != (supported ? kind : nil) {
            let children = container.subviews.compactMap { $0 as? NodeView }
            materialView?.removeFromSuperview()
            materialView = nil
            materialKind = nil
            if supported {
                let effect = kind == "backdrop" ? BackdropEffectView() : UIVisualEffectView()
                effect.isUserInteractionEnabled = Materials.glass(kind)
                effect.frame = bounds
                effect.autoresizingMask = [.flexibleWidth, .flexibleHeight]
                (glassSlot?.contentView ?? self).insertSubview(effect, at: 0)
                materialView = effect
                materialKind = kind
            }
            for (index, child) in children.enumerated() { container.insertSubview(child, at: index) }
            presenter?.flats.containerChanged(id)
        }
        guard let materialView else { rehomeMaterialChildren(); return }
        if materialView.effect == nil || materialInteractive != interactive || backdropStale {
            materialView.effect = backdropEffect() ?? materialEffect(kind ?? "ultra-thin", interactive: interactive)
            materialInteractive = interactive
        }
        applyMaterialRadius()
        rehomeMaterialChildren()
    }
    func applyMaterialRadius() {
        guard let materialView else { return }
        let radius = BorderPaint.clip(materialView.layer, in: bounds, radii: cornerSizes(in: bounds))
        if #available(iOS 26.0, tvOS 26.0, *) {
            materialView.cornerConfiguration = .corners(radius: .fixed(Double(radius)))
        } else {
            materialView.layer.cornerRadius = radius
            materialView.clipsToBounds = true
        }
        if style["mask_image"] != nil { applyBoxMask() }
    }
    var pendingScrollLeft: Double? {
        get { extras?.pendingScrollLeft }
        set { if newValue != nil || extras != nil { more.pendingScrollLeft = newValue }; presenter?.pendingScrolls.insert(id) }
    }
    var pendingScrollTop: Double? {
        get { extras?.pendingScrollTop }
        set { if newValue != nil || extras != nil { more.pendingScrollTop = newValue }; presenter?.pendingScrolls.insert(id) }
    }
    func applyProps(set: [String: String], clear: [String]) {
        if clear.contains("action") { cancelSurfaceControls() }
        if clear.contains("scrollLeft") { pendingScrollLeft = nil }
        if let raw = set["scrollLeft"], let left = Double(raw), left.isFinite { pendingScrollLeft = left }
        if clear.contains("scrollTop") { pendingScrollTop = nil }
        if let raw = set["scrollTop"], let top = Double(raw), top.isFinite { pendingScrollTop = top }
        // One assignment: `props` tells the presenter of each.
        var next = props
        for k in clear { next.removeValue(forKey: k) }
        for (k, v) in set { next[k] = v }
        props = next
        swipeOwner = props["swipeContent"] != nil
        if set["symbolEffectValue"] != nil { updateSymbol() }
        if (pendingScrollLeft ?? 0) != 0 || (pendingScrollTop ?? 0) != 0 { needScroll() }
        if set["inert"] != nil || clear.contains("inert") {
            let ownInert = props["inert"] == "true"
            if ownInert { endEditing(true) }
            isUserInteractionEnabled = !ownInert
        }
        accessibilityElementsHidden = hidesAccessibility
        updateKeyboardDismissal()
        updateMaterial()
        applyTextArea()
        if let f = field {
            f.tintColor = props["emojiPicker"] == "true" ? .clear : nil
            if (set["emojiPicker"] != nil || clear.contains("emojiPicker")), f.isFirstResponder { f.reloadInputViews() }
            if let v = props["value"] { writeValue(v, into: f) }
            applyPlaceholder(f); applyFieldName(f)
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
        updateTextAccessibility()
        if kind == "button" {
            isAccessibilityElement = true
            accessibilityTraits.insert(.button)
            if props["accessibilitySelected"] == "true" { accessibilityTraits.insert(.selected) } else { accessibilityTraits.remove(.selected) }
            setAccessibilityToggle(pressedState)
            if #available(iOS 18, tvOS 18, *) {
                accessibilityExpandedStatus = props["accessibilityExpanded"].map { $0 == "true" ? .expanded : .collapsed } ?? .unsupported
            }
        }
        if kind == "image", let src = props["imageSource"], src != imageSource { loadImage(src) }
        if kind == "image", props["imageSource"] == nil, imageSource != nil {
            loadGeneration += 1
            presenter?.session?.rasters.cancel(id)
            raster = nil
            imageSource = nil
            clearSymbol()
            image = nil
            presenter?.intrinsic(id, nil)
        }
        updateEmbedded()
        video?.update()
        setNeedsDisplay()
    }

    /// CSS `filter` (LLP 1055.000 D14): the box shows through a filtered
    /// picture (`BoxFilter`), drawn again after each batch.
    private(set) var boxFilter: BoxFilter?
    var hasBoxFilter: Bool { boxFilter != nil }
    func applyFilter() {
        // A node with no `filter` and none before makes no `BoxFilter` (three
        // layers) to learn so: every styled node passes through here.
        guard boxFilter != nil || style["filter"] != nil else { return }
        let f = boxFilter ?? BoxFilter()
        if f.set(style["filter"]) {
            boxFilter = f
            layer.mask = f.hide
            presenter?.boxFilters.add(self) { [weak self] in self?.renderFilter() }
            renderFilter()
        } else if let f = boxFilter {
            f.remove()
            boxFilter = nil
            presenter?.boxFilters.remove(self)
            applyBoxMask()
        }
    }

    func renderFilter() {
        guard let f = boxFilter else { return }
        guard superview != nil else { f.remove(); return }
        f.render(layer, clip: resolvedClipMask(), scale: window?.screen.scale ?? traitCollection.displayScale)
    }

    override func didMoveToSuperview() {
        super.didMoveToSuperview()
        // A static flex or grid item's z-index depends on its parent, which a
        // view styled before it was mounted did not have.
        if superview != nil, layer.zPosition != usedZIndex { layer.zPosition = usedZIndex }
        if superview == nil { boxFilter?.remove() } else if boxFilter != nil { renderFilter() }
        // A box styled before it joined its parent learns its material now.
        if superview != nil { syncVibrancy() }
    }

    func updateKeyboardDismissal() {
        switch props["keyboardDismissMode"] {
        case "interactive": scroll?.keyboardDismissMode = .interactive
        case "on-drag": scroll?.keyboardDismissMode = .onDrag
        default: scroll?.keyboardDismissMode = .none
        }
    }
    func applyStyle(_ s: NodeStyle) {
        defer { video?.update() }
        let origin = style["transform_origin"]
        let old = style
        style = s
        updateSymbol()
        (clipPath, clipRule) = (ClipPath.path(s["clip_path"]), ClipPath.rule(s["clip_path"]))
        applyBoxMask()
        applyFilter()
        updateMaterial()
        syncVibrancy()
        syncScroll()
        applyAffordances()
        styleTextArea()
        if let f = field, let t = text {
            f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic", numeric: Int(number("font_variant_numeric")))
            f.textColor = color("text_color", .black)
            applyPlaceholder(f)
            f.frame = contentBox()
        }
        layer.zPosition = usedZIndex
        if s["transform_origin"] != origin { applyTransform() }
        applySpace(changedFrom: old)
        setNeedsDisplay()
    }

    /// Scrolling and clipping come from the effective overflow the host
    /// wrote in (never from the node's kind): `scroll` on an axis makes a
    /// scroll container that scrolls that axis; `hidden` clips.
    func syncScroll() {
        let ox = style["overflow_x"]?.string ?? "visible", oy = style["overflow_y"]?.string ?? "visible"
        let scrolls = ox == "scroll" || oy == "scroll"
        if scrolls { syncClipBox(false) }
        if scrolls && scroll == nil && !scrollWaits {
            let sv = ScrollView(frame: bounds)
            sv.backgroundColor = .clear
            sv.contentInsetAdjustmentBehavior = .never
            sv.delegate = self
            sv.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            GlassGroups.moving(in: self) {
                for child in subviews where child is NodeView { child.removeFromSuperview(); sv.addSubview(child) }
                addSubview(sv)
            }
            scroll = sv
            scrollWritten = nil
            updateRefresh()
        }
        if !scrolls, let sv = scroll {
            // Neither axis scrolls any more: the children come back out.
            GlassGroups.moving(in: self) {
                for child in sv.subviews where child is NodeView { child.removeFromSuperview(); addSubview(child) }
                sv.removeFromSuperview()
            }
            scroll = nil
        }
        scroll?.scrollsX = ox == "scroll"
        scroll?.scrollsY = oy == "scroll"
        // UIKit's default indicator is already thin. CSS permits `thin`
        // to match `auto` on such platforms; `none` only hides the track.
        // Indicators and deceleration are the app's once a hook sets them
        // (LLP 1075.003 §3.5): written when what the style says changes.
        let snap = style["scroll_snap_type"]?.string == "x mandatory"
        let indicators = (style["scrollbar_width"]?.string ?? "auto") != "none"
        if let sv = scroll, scrollWritten != "\(snap)|\(ox)|\(oy)|\(indicators)" {
            scrollWritten = "\(snap)|\(ox)|\(oy)|\(indicators)"
            sv.decelerationRate = snap ? .fast : .normal
            sv.showsHorizontalScrollIndicator = ox == "scroll" && indicators
            sv.showsVerticalScrollIndicator = oy == "scroll" && indicators
        }
        updateKeyboardDismissal()
        fitScroll()
        // A waiting scroll clips as its scroll view would.
        let clips = ox == "hidden" || oy == "hidden" || scrollDormant
        // A paragraph paints its own text, which a box would not clip.
        syncClipBox(clips && kind != "text" && shadowColor != nil && scroll == nil && overlay == nil && materialKind != "glass")
        clipsToBounds = clips && clipBox == nil
        // A scroll's children, back out, go where a material holds them.
        if materialView != nil, scroll == nil { rehomeMaterialChildren() }
        syncGlassGroup()
        settleVibrancy()
    }

    /// A native swipe row's scroll container (`swipeContent`, LLP 1008 §9)
    /// waits for its UIScrollView. UIKit's swipe cell, not the scroll, takes
    /// the row's touches (`SwipeActionsHost`), and a closed row's content
    /// sits at the scroll's start, as it does at rest on the web: the
    /// children live in the node itself, clipped as the scroll would clip
    /// them. The scroll view comes when something needs it (`needScroll`):
    /// the swipe host refusing the row (the scroll is then the swipe), an
    /// authored scroll position or `scroll` handler, the agent's wheel, a
    /// reveal. A list builds each row without a scroll view, its
    /// recognizers, and its registration with the window.
    var swipeOwner = false { didSet { if oldValue && !swipeOwner { syncScroll() } } }
    private(set) var scrollNeeded = false
    var scrollWaits: Bool { swipeOwner && !scrollNeeded && !handlers.contains("scroll") }
    /// The node's overflow scrolls, and its scroll view is still waiting.
    var scrollDormant: Bool {
        scroll == nil && ((style["overflow_x"]?.string) == "scroll" || (style["overflow_y"]?.string) == "scroll")
    }

    func needScroll() {
        guard scrollDormant else { return }
        scrollNeeded = true
        syncScroll()
    }

    /// The scroll container's content size: the kernel's extent on an axis
    /// that scrolls, the box on one that does not (so UIKit cannot pan it
    /// there), never less than the box.
    func fitScroll() {
        guard let sv = scroll else { return }
        let size = CGSize(width: sv.scrollsX ? max(content.width, sv.bounds.width) : sv.bounds.width, height: sv.scrollsY ? max(content.height, sv.bounds.height) : sv.bounds.height)
        if sv.contentSize != size { sv.contentSize = size }
        // An orthogonal carousel's computed auto axis has no vertical travel.
        // Making that axis bounce traps Mac wheel input instead of letting the
        // enclosing page scroll. Keep elastic feedback for vertical content,
        // including short vertical lists that have no horizontal overflow.
        sv.alwaysBounceVertical = sv.scrollsY && (size.height > sv.bounds.height + 0.5 || size.width <= sv.bounds.width + 0.5)
    }

    override func layoutSubviews() {
        if focusRing != nil { showFocusRing(true) }
        if let s = presenter?.session, s.firstLayoutMs == nil { s.firstLayoutMs = ExactEnv.wall() }
        super.layoutSubviews()
        if materialView != nil { applyMaterialRadius() }
        syncEllipticalClip()
        if style["perspective"] != nil { applyPerspective() }
        if kind == "image" { presenter?.session?.rasters.resized(self); if raster != nil { applyImageLayer() } }
        presenter?.collections.changed(id)
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
        if field != nil { field?.frame = contentBox() }
        video?.layout()
        if kind == "native" { presenter?.session?.natives.laidOut(self) }
        layoutTextArea()
        layoutSymbol()
    }

    /// The reduced radii; the layer fast path additionally requires circles.
    func cornerSizes(in rect: CGRect, inset: CGFloat = 0) -> [CGSize] {
        BorderPaint.reduced(BorderPaint.radii(style, in: rect, inset: inset), in: rect)
    }
    func cornerRadii(in rect: CGRect, inset: CGFloat = 0) -> [CGFloat] {
        cornerSizes(in: rect, inset: inset).map { $0.width }
    }
    func roundedPath(in rect: CGRect, inset: CGFloat = 0) -> UIBezierPath {
        UIBezierPath(cgPath: BorderPaint.roundedRect(rect, cornerSizes(in: rect, inset: inset), shape: CornerShape(style["corner_shape"])))
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
        let path = roundedPath(in: bounds)
        let uniform = number("border_width")
        // A box Core Animation can say is the layer's (`applyBoxLayer`);
        // the background within its `background-clip` (LLP 1077 D6).
        paintBackground(ctx, border: path.cgPath, color: boxDrawn)
        if boxDrawn {
            // Sides that differ in colour or width, or a radius the layer
            // cannot say: each side in its colour, joined as the web joins
            // them (`BorderPaint`).
            let widths = ["top", "right", "bottom", "left"].map { number("border_width_" + $0, uniform) }
            let top = color("border_color_top", .clear)
            let colors = ["top", "right", "bottom", "left"].map { color("border_color_" + $0, top).cgColor }
            let radii = BorderPaint.radii(style, in: bounds)
            BorderPaint.paint(ctx, box: bounds, widths: widths, colors: colors, radii: radii, shape: CornerShape(style["corner_shape"]))
        }
        if kind == "image", symbolView == nil, let bitmap = raster?.image {
            // CSS object-fit over the content box (the frame inside border
            // and padding), clipped by the border box's radius: `fill`
            // stretches, `contain`/`cover` keep the ratio, `none` is the
            // natural size, `scale-down` the smaller of none and contain;
            // an unknown value is the initial `fill`.
            let fit = style["object_fit"]?.string ?? "fill"
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
            RasterGeometry.draw(ctx, AnimatedRasters.shared.frame(for: self) ?? bitmap.image, in: CGRect(origin: .zero, size: rect.size), tint: channels("tint_color").map { TextEngine.color($0).cgColor })
            ctx.restoreGState()
        }
        if isParagraph {
            if canRasterText {
                presenter?.textRasters.ensure(self, urgent: presenter?.textIsVisible(self) == true)
            } else if !textRasterFailed { dropTextRaster() }
            // Without a raster this bitmap is the paragraph's only paint, and
            // nothing redisplays a row the lead drew off-screen when it
            // scrolls in: draw it now, visible or not.
            if textRaster == nil && (!canRasterText || Capture.capturing || presenter?.textIsVisible(self) == true) {
                let post = Presenter.signposts.beginInterval("text-draw")
                defer { Presenter.signposts.endInterval("text-draw", post) }
                let spec = paragraphSpec()
                if let paragraph = paragraphLayout() {
                    paintBackgroundThroughText(ctx, paragraph: paragraph, spec: spec, in: contentBox())
                    TextEngine.draw(paragraph, spec: spec, in: contentBox(), context: ctx, dirty: rect)
                }
            }
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
        if ((isSurfaceControl || ownsSurfaceControl) ? inputCanvas?.canvasInput : canvasInput)?.touches(touches, phase: "down", source: self, event: event) == true { return }
        guard !disabled else { pressed = false; return }
        if let touch = touches.first, let target = presenter?.svg.target(id, at: local(touch.location(in: nil))) {
            svgPressed = target; return
        }
        if let touch = touches.first, let run = inlineActivationTarget(at: local(touch.location(in: nil))) {
            inlinePressed = run.id; return
        }
        if handlers.contains("press") { pressed = true } else { super.touchesBegan(touches, with: event) }
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        inlinePressed = nil
        if ((isSurfaceControl || ownsSurfaceControl) ? inputCanvas?.canvasInput : canvasInput)?.touches(touches, phase: "move", source: self, event: event) == true { return }
        if pressed { pressFollows(inside: touches.first.map(pressInside) ?? false) } else { super.touchesMoved(touches, with: event) }
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if ((isSurfaceControl || ownsSurfaceControl) ? inputCanvas?.canvasInput : canvasInput)?.touches(touches, phase: "up", source: self, event: event) == true { finishPointerPress(); return }
        guard !disabled else { pressed = false; inlinePressed = nil; svgPressed = nil; return }
        if let target = svgPressed {
            svgPressed = nil
            if let touch = touches.first, presenter?.svg.target(id, at: local(touch.location(in: nil))) == target { presenter?.press(target) }
            return
        }
        if let run = inlinePressed {
            inlinePressed = nil
            if let touch = touches.first, inlineActivationTarget(at: local(touch.location(in: nil)))?.id == run { _ = activateInline(run) }
            return
        }
        // A press under `retainFocus` leaves the editor its focus, as macOS's
        // mouseDown does: every pressable can take the focus now.
        if canBecomeFirstResponder, !isFirstResponder, presenter?.contextRetainsFocus(self) != true { _ = becomeFirstResponder() }
        guard pressed else { return super.touchesEnded(touches, with: event) }
        pressed = false
        // A pressed node that did not take the focus: the field being edited
        // loses it, as a click on a button blurs a page's input.
        let inside = touches.first.map(pressInside) ?? false
        if !isFirstResponder && presenter?.contextRetainsFocus(self) != true { presenter?.viewport.endEditing(true) }
        if inside, presenter?.views[id] === self { presenter?.press(id); finishPointerPress() }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        inlinePressed = nil; svgPressed = nil
        if ((isSurfaceControl || ownsSurfaceControl) ? inputCanvas?.canvasInput : canvasInput)?.touches(touches, phase: "cancel", source: self, event: event) == true { return }
        if pressed { pressed = false } else { super.touchesCancelled(touches, with: event) }
    }

    /// A press delivered by the rule a touch gets: to this node when it has
    /// a handler, else to the nearest ancestor with one, if the point (in
    /// the window) is inside that node's box. The agent's `tap` and
    /// VoiceOver's activation come here — UIKit offers no public touch
    /// synthesis.
    @discardableResult
    func activate(at windowPoint: CGPoint) -> NodeView? {
        // An SVG element under the point takes it (LLP 1055.000 D17).
        if kind == "svg", let element = presenter?.svg.target(id, at: local(windowPoint)) { presenter?.press(element); return self }
        guard let target = activationTarget(at: windowPoint) else { return nil }
        if target.isSurfaceControl { return target.control("down") && target.control("up") ? target : nil }
        target.presenter?.press(target.id)
        return target
    }
    /// Resolve before focus changes: a keyboard resize can move the control.
    func activationTarget(at windowPoint: CGPoint) -> NodeView? {
        guard !inert else { return nil }
        var v: UIView? = self
        while let cur = v {
            if let n = cur as? NodeView, n.disabled { return nil }
            if let n = cur as? NodeView, (n.handlers.contains("press") || n.isSurfaceControl) {
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

    /// As `writeValue(_:into:)` for a text view: composition defers the write,
    /// and the caret is carried through the changed middle.
    func writeValue(_ value: String, into f: UITextField) {
        if f.markedTextRange != nil { pendingValue = value; return }
        pendingValue = nil
        guard let edit = minimalTextEdit(from: f.text ?? "", to: value) else { return }
        var selection = NSRange(location: (f.text ?? "").utf16.count, length: 0)
        if let r = f.selectedTextRange {
            selection = NSRange(location: f.offset(from: f.beginningOfDocument, to: r.start), length: f.offset(from: r.start, to: r.end))
        }
        f.text = value
        let carried = carrySelection(selection, through: edit)
        if let start = f.position(from: f.beginningOfDocument, offset: carried.location), let end = f.position(from: start, offset: carried.length) {
            f.selectedTextRange = f.textRange(from: start, to: end)
        }
    }
    @objc func fieldChanged() {
        if props["emojiPicker"] == "true", let field {
            if field.markedTextRange == nil, let held = pendingValue { writeValue(held, into: field) }
            let value = field.text ?? ""
            field.text = ""
            if !disabled, EmojiSelection.accepts(value) { presenter?.typed(id, value, input: handlers.contains("input")) }
            return
        }
        // The textarea's order (`textViewDidChange`): the text the field now
        // holds is reported first, and only then does a value held while
        // composing apply. Writing it first reported the composing text —
        // 你好 committed as "nihao".
        if !disabled { presenter?.typed(id, field?.text ?? "", input: handlers.contains("input")) }
        if let f = field, f.markedTextRange == nil, let held = pendingValue { writeValue(held, into: f) }
    }
    func textFieldDidBeginEditing(_ textField: UITextField) {
        presenter?.collections.pinsChanged()
        presenter?.editing = self
        // The keyboard is already up (another field had it): it will not
        // move, so this field is revealed here, as a browser scrolls a
        // newly focused field into view.
        if let p = presenter, p.keyboardInset > 0 {
            if ExactEnv.agentFreezes {
                p.reveal(self)
            } else {
                UIView.animate(withDuration: 0.25) { p.reveal(self) }
            }
        }
        if handlers.contains("focus") { presenter?.focus(id) }
    }
    func textFieldDidEndEditing(_ textField: UITextField) {
        presenter?.collections.pinsChanged()
        if presenter?.editing === self { presenter?.editing = nil }
        presenter?.commitEdit(id, textField.text ?? "", change: handlers.contains("change"))
        if handlers.contains("blur") { presenter?.blur(id) }
    }
}
#endif
