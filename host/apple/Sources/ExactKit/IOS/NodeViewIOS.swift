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

package final class NodeView: UIView, UITextViewDelegate, UITextFieldDelegate, UIScrollViewDelegate, UIGestureRecognizerDelegate {
    /// The kernel's id; a parked view takes a new row's and a new incarnation (`NodePool`, LLP 1068 §4.9).
    package var id: UInt32
    var incarnation = NodePool.issue()
    // Twice the kernel rank; transients never overwrite the authored answer.
    var rank: Int64 = 0
    var paintLifted = false
    var paintGhost = false
    var paintGhosts = 0
    weak var paintParent: PaintView?
    weak var ghostParent: NodeView?
    let firstDraw: () -> Void
    package let kind: String
    var inlineText: [InlineText] = []
    /// The rarely set fields (`NodeExtrasIOS.swift`); nil until one is set.
    var extras: NodeExtras?
    package override class var layerClass: AnyClass { NodeLayer.self }
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
    var columnRecord: ColumnRecord?  // LLP 1093 D7: fragments or columns
    var cachedTextLayout: (width: CGFloat, paragraph: Paragraph)?
    var liveText: String?
    package var props: [String: String] = [:] { didSet { presenter?.propsChanged(self) } }
    package var style: NodeStyle = [:]
    /// What the host's own writers hid (a covered route, a tab a native control
    /// stands in for, a placement); a `display: none` box is hidden besides, as
    /// CSS removes it with its subtree, or its texts paint at its 0×0 frame's
    /// origin (recipes F19). Reading says whether it is hidden, for either reason.
    private var hostHidden = false
    /// Whether the host hid this view, whatever CSS's `display` says: what a
    /// projection or a placement saves and restores, since `isHidden` also
    /// reads `display: none` (review B1: restoring that wrote CSS's bit into
    /// the host's and kept the view hidden once it was displayed).
    package var hiddenByHost: Bool { hostHidden }
    package override var isHidden: Bool {
        get { super.isHidden }
        set {
            // Writing back "hidden" that only CSS's `display: none` made is not
            // the host's word (a save of `isHidden` restored; review B1): a
            // projection that means it hides again on its next pass.
            let css = style["display"]?.string == "none"
            if !(newValue && css && !hostHidden && super.isHidden) { hostHidden = newValue }
            super.isHidden = hostHidden || css
        }
    }
    package var handlers: Set<String> = [] {
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
    package func allowsTouchPan(_ velocity: CGPoint) -> Bool {
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
    package override func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
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
        if let drag = DragLink.installed?.shouldBegin(self, gesture) { return drag }
        if gesture === heightRecognizer, let pan = gesture as? UIPanGestureRecognizer {
            let velocity = pan.velocity(in: window), translation = pan.translation(in: window)
            return SwipeInput.allows(self) && HeightDragDirection.accepts(
                velocityX: Double(velocity.x), velocityY: Double(velocity.y),
                translationX: Double(translation.x), translationY: Double(translation.y))
                && presenter?.heightBindings[id]?.target != nil
        }
        if gesture === swipeRecognizer, let pan = gesture as? UIPanGestureRecognizer {
            let velocity = pan.velocity(in: window)
            // Where the finger landed: the translation leaves out the travel
            // before recognition, so a swipe from x = 2 read as starting past
            // the edge and a back swipe over a message became a reply.
            let start = swipeDownX ?? pan.location(in: window).x - pan.translation(in: window).x
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
    package func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        if CanvasInputs.owns(touch.view) { return false }
        if gestureRecognizer === swipeRecognizer, gestureRecognizer.numberOfTouches == 0 { swipeDownX = touch.location(in: window).x }
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
        // Where the press is, as a touch's `PointerEvent` (studio diary R22).
        let point = gesture.location(in: self), box = contentBox()
        let client = presenter?.client(gesture.location(in: nil)) ?? .zero
        let sample = PointerSample(x: Double(point.x - box.minX), y: Double(point.y - box.minY), buttons: 1, pressure: 0.5, type: "touch", id: 2,
                                   clientX: Double(client.x), clientY: Double(client.y))
        presenter?.contextmenu(id, line: sample.line); presenter?.menus.agentContext(self) // then, under the agent, its popover (LLP 1021 §5.1)
    }
    @objc func doubleClicked(_ gesture: UITapGestureRecognizer) {
        guard gesture.state == .ended, !disabled else { return } // then after this touch's own press
        DispatchQueue.main.async { [weak self, token = incarnation] in if let self, self.incarnation == token, !self.disabled, self.presenter?.views[self.id] === self { self.presenter?.dblclick(self.id) } }
    }
    var translatePx = CGPoint.zero, translatePercent = CGPoint.zero // `translate`: its lengths, and its percentages of the box (chess diary #4)
    package var scale: CGFloat = 1
    package var rotate: CGFloat = 0
    package var contextTransform = CGAffineTransform.identity {
        didSet {
            if contextTransform.isIdentity { presenter?.contextNodes.remove(id) }
            else { presenter?.contextNodes.insert(id) }
        }
    }
    weak package var presenter: Presenter?
    /// The scroll view a capability module sees (LLP 1047.001 D4).
    package var scrollView: UIScrollView? { scroll }
    package var scrollsVertically: Bool { scroll?.scrollsY ?? true }
    package var scroll: ScrollView? {
        didSet {
            if scroll == nil { presenter?.scrollers.remove(id) }
            else { presenter?.scrollers.insert(id) }
        }
    }
    /// A scroll container's content extent (the `content` op), before the
    /// axes that do not scroll are held to the box.
    var content = CGSize.zero
    package var placementHidden: Bool {
        get { extras?.placementHidden ?? false }
        set {
            let oldValue = placementHidden
            if newValue || extras != nil { more.placementHidden = newValue }
            if placementHidden && !oldValue { more.hiddenBeforePlacement = hiddenByHost }
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
    /// While it flies as a shared element (LLP 1013.000 D4): where its image is drawn.
    var flightLook: FlightLook?
    var imageSource: String?
    var loadGeneration = 0
    package var pressed = false { didSet { if pressed != oldValue { pressChanged() } } }
    /// The focus came from a touch's press (PointerIOS, NativeButtonsIOS),
    /// focus WebKit never gives a button on a tap: a button holding it does
    /// not keep a later autofocus off (Accessibility). Tab and `focus(id)`
    /// clear it (PresenterIOS); UIKit handing the focus back when the view
    /// moves (into a presented sheet) keeps it.
    package var focusedByTouch = false
    /// The focus a touch's press takes (PointerIOS, NativeButtonsIOS, the
    /// agent's tap), marked before `becomeFirstResponder` runs: its `focus`
    /// handler can mount an autofocus field, whose pass must see the mark.
    @discardableResult
    func takeTouchFocus() -> Bool {
        focusedByTouch = true
        if becomeFirstResponder() { return true }
        focusedByTouch = false
        return false
    }
    var press = PressFeedback() // LLP 1061 D2: the feedback `pressed` drives
    package var disabled: Bool { props["disabled"] == "true" }
    /// HTML inertness covers the subtree, including direct agent activation.
    package var inert: Bool {
        var ancestor: UIView? = self
        while let view = ancestor {
            if (view as? NodeView)?.props["inert"] == "true" { return true }
            ancestor = view.superview
        }
        return false
    }
    /// Images loaded since launch (smoke reporting).
    /// The session's text engine (LLP 1031 D12: the catalog is the session's).
    package var text: TextEngine? { presenter?.session?.text }
    package var canvases: Canvases? { presenter?.session?.canvases }

    /// A node with focus, blur, or key handlers takes the focus (an input's
    /// field does by itself): the web's rule that only a focusable element
    /// hears these. Keys come from a hardware keyboard (`pressesBegan`).
    /// UIKit's focus search finds what UIKit can focus (`FocusSearch`).
    package override func didAddSubview(_ subview: UIView) { super.didAddSubview(subview); FocusSearch.joined(subview) }
    /// A hidden element is not exposed. Children stay in the tree: this is
    /// not `accessibilityElementsHidden`, which would hide them too.
    package override var isAccessibilityElement: Bool {
        get { accessibilityExposed && super.isAccessibilityElement }
        set { super.isAccessibilityElement = newValue }
    }
    package override var canBecomeFirstResponder: Bool { !formDisabled && !inert && !cssVisibilityHidden && !isHidden && field == nil && textArea == nil && (kind == "button" || isNativeButton || isRadio || explicitTabIndex != nil || canvases?.wantsInput(id) == true || !handlers.isDisjoint(with: Self.focusEvents)) }
    package override func becomeFirstResponder() -> Bool {
        guard !formDisabled, !inert, !cssVisibilityHidden else { return false }
        let ok = super.becomeFirstResponder()
        if ok { presenter?.collections.pinsChanged() }
        if ok, handlers.contains("focus"), presenter?.menus.focus.quiet != true { presenter?.focus(id) } // a menu's set-aside focus returns quietly (MenuFocusIOS)
        return ok
    }
    package override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { showFocusRing(false) }
        if ok { presenter?.collections.pinsChanged() }
        if ok && !isSurfaceControl { inputCanvas?.canvasInput?.blur() }
        if ok, handlers.contains("blur"), presenter?.menus.focus.quiet != true { presenter?.blur(id) }
        return ok
    }
    /// A hardware keyboard's Tab and Shift-Tab move the focus through the
    /// sequential order, as macOS's key-view loop does (`Presenter.moveFocus`);
    /// Enter and Space then press (`pressesBegan`). UIKit gives text inputs a
    /// Tab of their own, so these take priority; the web's Tab leaves a
    /// textarea too. A field's or textarea's chain reaches its node's.
    package override var keyCommands: [UIKeyCommand]? { (super.keyCommands ?? []) + NodeView.tabCommands }
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
        guard shown, !isNativeTextControl else { focusRing?.removeFromSuperlayer(); focusRing = nil; return }
        let ring = focusRing ?? CAShapeLayer()
        #if os(tvOS)
        // Across a room the ring stands clear of the content: outside the box, padded and rounded.
        ring.path = UIBezierPath(roundedRect: clipsToBounds ? bounds.insetBy(dx: 2, dy: 2) : bounds.insetBy(dx: -10, dy: -5), cornerRadius: 12).cgPath
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
    package override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        #if os(tvOS)
        if remoteSelect(presses, down: true) { return }
        #endif
        let presses=pressedControls(presses,down:true)
        if presses.isEmpty {return}
        // The focus's `key` handlers and its ancestors' (KeyEvents.swift); an
        // ancestor UIKit passes the presses up to dispatches none again. In a
        // world's canvas they hear a key before the world, and one that
        // prevents it keeps it, as macOS's `routeKey` and the web order them
        // (the platformer's diary, R8).
        let name = presses.first?.key.map(NodeView.keyName)
        let held = presses.first?.key.map { KeyCodes.held($0.modifierFlags) } ?? ""
        if !formDisabled, isFirstResponder, let key = presses.first?.key, let name, hardwareKey(key, down: true) || presenter?.controls.radioKey(self, name, held: held) == true { return }
        if inputCanvas?.canvasInput?.presses(presses, down: true, source: self) == true { return }
        if !disabled, handlers.contains("press") || defaultLink != nil, let name, name == "Enter" || (name == " " && props["href"] == nil && UIDevice.current.userInterfaceIdiom != .tv) { presenter?.press(id); return }
        super.pressesBegan(presses, with: event)
    }
    package override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        #if os(tvOS)
        if remoteSelect(presses, down: false) { return }
        #endif
        let presses=pressedControls(presses,down:false)
        if presses.isEmpty {return}
        // The release's `keyup` handlers (#140), as `pressesBegan` the down's.
        if !formDisabled, isFirstResponder, let key = presses.first?.key { _ = hardwareKey(key, down: false) }
        if inputCanvas?.canvasInput?.presses(presses, down: false, source: self) != true { super.pressesEnded(presses, with: event) }
    }
    package override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let presses=pressedControls(presses,down:false)
        if presses.isEmpty {return}
        if inputCanvas?.canvasInput?.presses(presses, down: false, source: self) != true { super.pressesCancelled(presses, with: event) }
    }
    /// A pointer over the node (an iPad's trackpad or mouse; a phone has
    /// none): `hover` in and out.
    package override var accessibilityElements: [Any]? {
        get { textAccessibilityChildren() ?? super.accessibilityElements }
        set { super.accessibilityElements = newValue }
    }
    /// A text field's Enter as a key (its characters are its `input`, the
    /// Enter commits its `change`); the editing goes on, as on the web.
    package func textFieldShouldBeginEditing(_ textField: UITextField) -> Bool { !disabled && !inert }
    package func textViewShouldBeginEditing(_ textView: UITextView) -> Bool { !disabled && !inert }

    package func textField(_ textField: UITextField, shouldChangeCharactersIn range: NSRange, replacementString string: String) -> Bool {
        guard !disabled, !inert, props["editable"] != "false" else { return false }
        if props["emojiPicker"] == "true" {
            if EmojiSelection.accepts(string) { presenter?.typed(id, string, input: handlers.contains("input")) }
            return false
        }
        return TextInputLimit.allows(textField.text ?? "", range: range, replacement: string, props: props)
    }

    package func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        guard !disabled else { return false }
        // The software keyboard's Return is a key the handlers hear first (a
        // hardware one's they heard in `pressesBegan`); prevented, it neither
        // commits nor submits.
        if (textField as? TextField)?.heard != "Enter", presenter?.keyDown(at: self, "Enter") == true { return false }
        presenter?.commitEdit(id, textField.text ?? "", change: handlers.contains("change"))
        // Enter in an input with a `submit` handler is the web's implicit submission.
        if handlers.contains("submit") { presenter?.submit(id) }
        // The key does what its label says (LLP 1115 wave 1): Next moves to
        // the next field (or, at the last, puts the keyboard away); Done,
        // Go, Search and Send put the keyboard away.
        switch props["enterKeyHint"] {
        case "next": if presenter?.moveFocus(backward: false, fields: true) != true { textField.resignFirstResponder() }
        case "done", "go", "search", "send": textField.resignFirstResponder()
        default: break
        }
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
        raster = lease; AnimatedRasters.shared.attach(self); applyImageLayer()
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
        let name = props["symbolName"] ?? "", points = number("font_size", PageFacts.defaultRootFontSize)
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
        symbolView?.tintColor = symbolTint // `nil` inherits UIKit's live tint
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
        registerForTraitChanges(SystemColor.traits) { (node: NodeView, _: UITraitCollection) in // platform colours follow contrast and level too (LLP 1095 D5)
            // A control's accent, a tablist's tint and a grouped list's
            // switches resolve per appearance in their projections.
            if node.kind == "control" || node.kind == "list" || node.props["accessibilityRole"] == "tablist" {
                node.presenter?.requestProjectionSync()
            }
            node.paragraphOwner.invalidateText()
            node.paragraphOwner.setNeedsDisplay()
            node.applyStyle(node.style)
            // Its paint motion resolves in its own appearance (LLP 1062 D4),
            // a `color-scheme` above it included (LLP 1034 §8).
            node.presenter?.session?.noteAppearance(node)
            // An `svg`'s paints are resolved into its scene's layers.
            node.presenter?.svg.reappear(node.id, dark: node.drawsDark, clock: node.presenter?.session?.clock)
            node.presenter?.requestTextPublication()
            if let presenter = node.presenter, node.superview === presenter.root { presenter.paintCanvas() }
        }
        // The box's background is the layer's (`applyBoxLayer`), never
        // UIView's: UIKit would reapply its own on a trait change.
        isOpaque = false
        // A frame change repaints at the new width instead of stretching stale pixels.
        contentMode = .redraw
        if kind == "canvas", let m = SurfacesLink.installed?.makeMetalView() {
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

    package override func didMoveToWindow() {
        super.didMoveToWindow()
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
        if window != nil { presenter?.flushPendingFocus() }
    }

    /// Glass content participates in UIKit's interactive effect. Other
    /// materials remain background siblings of the authored children. A
    /// glass group is innermost (`GlassGroup.swift`).
    package var container: UIView { glassGroupView?.contentView ?? baseContainer }

    /// The canvas this node is painted through, if any: the nearest canvas
    /// above whose overlay holds it.
    package var canvasAbove: NodeView? {
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
    package func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        if !decelerate { followEndIfOwed() }
    }
    package func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) { followEndIfOwed() }
    package func scrollViewDidEndScrollingAnimation(_ scrollView: UIScrollView) {
        followingEndAnimated = false
        // A smooth correction's driver ends its own motion (`OffsetDriver`);
        // a UIKit animation's end is not its end.
        if let c = presenter?.collections, c.offsetDrivers[id] != nil || c.startOwed.contains(id) { return }
        presenter?.collections.animationEnded(id, atTarget: endedAtTarget(scrollView))
    }
    private func followEndIfOwed() {
        guard followsEndAfterInteraction, let sv = scroll else { return }
        followsEndAfterInteraction = false
        let maximum = max(-sv.adjustedContentInset.top, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height)
        if sv.contentOffset.y >= maximum - 80 { sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: maximum), animated: true) }
        anchoredScrollTop = maximum
    }

    package func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        followingEndAnimated = false
        presenter?.collections.animationEnded(id, dragging: true)
        presenter?.collections.userIntent(id, travel: true)
        retainedScrollTop = nil
    }

    /// A scroll under a canvas repaints it (LLP 1014 D4 c).
    package func scrollViewDidScroll(_ scrollView: UIScrollView) {
        let post = Presenter.signposts.beginInterval("scrolled")
        defer { Presenter.signposts.endInterval("scrolled", post) }
        presenter?.onScrolled?(id, Double(scrollView.contentOffset.x), Double(scrollView.contentOffset.y))
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
        guard handlers.contains("scroll"), let sv = scroll, case let point = sv.contentOffset,
              point != lastScrollEvent, presenter?.views[id] === self,
              hasScrollLayoutBox else { return }
        lastScrollEvent = point
        dispatchingScrollEvent = true
        defer { dispatchingScrollEvent = false }
        // CSS's extents (`ScrollEvent`): the port inside the insets, and the
        // port plus the range UIKit clamps a settled offset to, so at the
        // end `scrollHeight - scrollTop - clientHeight` is 0 (chat F4).
        let inset = sv.adjustedContentInset, top = scrollTopInset(sv)
        let port = CGSize(width: max(0, sv.bounds.width - inset.left - inset.right),
                          height: max(0, sv.bounds.height - inset.top - inset.bottom))
        let range = CGSize(width: max(0, sv.contentSize.width + inset.right - sv.bounds.width),
                           height: max(0, sv.contentSize.height + inset.bottom - sv.bounds.height + top))
        presenter?.scroll(id, [point.x, point.y + top, port.width + range.width, port.height + range.height,
                               port.width, port.height].map(Double.init))
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
    package func local(_ windowPoint: CGPoint) -> CGPoint {
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
    package func placementChanged() {
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
    package override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        // UIKit's conversion already carries a box in space through its
        // plane (LLP 1077 D8); a hidden back face is the host's to refuse.
        if placedAncestor?.placementHidden == true || hidesBack() { return nil }
        if let clipPath, !clipPath.contains(point, using: clipRule) { return nil }
        if !fragmentHit(point) { return nil }
        if props["swipeIndicator"] == "true" { return nil }
        if isSurfaceControl, !inert, !isHidden, !cssVisibilityHidden, isUserInteractionEnabled, bounds.contains(point) { return self }
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
            let passesThrough = style["pointer_events"]?.string == "none" || cssVisibilityHidden && !inlineRunShows(at: point)
            if let hit = NodeView.hitChildren(in: self, at: point, with: event, visit: { child in
                if child === (self.glassSlot ?? self.materialView), Materials.glass(self.materialKind) || self.blurHostsChildren, let contentView = self.materialView?.contentView {
                    // The effect's UIKit bounds check must not hide authored
                    // children in CSS visible overflow. They remain descendants
                    // of the effect, so its recognizers still see their touches.
                    if let hit = NodeView.hitChildren(in: contentView, at: self.convert(point, to: contentView), with: event) { return hit }
                }
                // Under `pointer-events: none` this box's own platform views
                // (a native module's, paint F9) are not targets either.
                if passesThrough, !(child is NodeView) { return nil }
                return child.hitTest(self.convert(point, to: child), with: event)
            }) { return hit }
            // CSS `pointer-events: none` (inherited): the box is never the
            // target, so a touch goes to what is under it — a header's blur
            // over a list must not stop the list scrolling. A descendant
            // that sets `auto` again is still a target.
            if passesThrough { return nil }
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
        for child in NodeView.hitOrder(overlay.subviews) where (child as? NodeView)?.placement == nil && (child as? NodeView)?.placementHidden != true {
            if let hit = child.hitTest(child.convert(inOverlay, from: overlay), with: event) { return hit }
        }
        // Nearest first: what is seen on top is what a tap reaches.
        for child in placed.reversed().sorted(by: { ($0.placement?[9] ?? 0) > ($1.placement?[9] ?? 0) }) {
            guard let h = child.placement, let inv = NodeView.invert(h) else { continue }
            let p = NodeView.map(inv, point)
            guard child.bounds.contains(p) else { continue }
            if let hit = child.hitTest(p, with: event) { return hit }
        }
        // Missed by every child: the canvas itself, unless it lets the
        // touch through (`pointer-events: none`, as `ordinary` says).
        return style["pointer_events"]?.string == "none" || cssVisibilityHidden ? nil : self
    }


    /// The box on screen, through the placement of the placed child this
    /// node is (or is under), for assistive technology — the same box the
    /// agent's `layout` reports.
    package override var accessibilityFrame: CGRect {
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
    package func channels(_ key: String, dark: Bool? = nil) -> [Double]? {
        style[key].flatMap { $0.channels(dark: dark ?? drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: ownTint(for: $0)) }
    }
    func textChannels(_ key: String, dark: Bool? = nil) -> [Double]? { style[key].flatMap { $0.textChannels(dark: dark ?? drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: ownTint(for: $0)) } }
    package func color(_ key: String, _ fallback: UIColor) -> UIColor { cgColor(key).map { UIColor(cgColor: $0) } ?? fallback }
    func cgColor(_ key: String, dark: Bool? = nil) -> CGColor? { style[key].flatMap { $0.cgColor(dark: dark ?? drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: ownTint(for: $0)) } }
    package func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
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
        // D3: UIKit's semantic placeholder colour on a native field. The
        // explicitly bare editor preserves its existing authored ink rule.
        let ink = isNativeTextControl ? UIColor.placeholderText : (f.textColor ?? SystemColor.canvasText).withAlphaComponent(0.30)
        f.attributedPlaceholder = NSAttributedString(string: text, attributes: [
            .font: font,
            .foregroundColor: ink,
        ])
    }

    // display:none removes the CSS box, but retains its stored scroll position.
    // UIKit/AppKit collapse the native extent; keep that transient reset out of
    // scroll events and restore only when the box returns.
    var hasScrollLayoutBox: Bool {
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
        scrollAnchor = nil
        guard props["scrollFollowEnd"] == "true", let sv = scroll else {
            activeReadingAnchor = nil; anchoredScrollTop = nil; retainedScrollTop = nil
            if let sv = scroll { captureScrollAnchor(sv) }
            return
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
        defer { followedScroll = nil; readingAnchors.removeAll(keepingCapacity: true); scrollAnchor = nil }
        guard props["scrollFollowEnd"] == "true", let sv = scroll else {
            if let sv = scroll { restoreScrollAnchor(sv) }
            return
        }
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
        // An authored position takes over from a smooth correction.
        if (pendingScrollTop != nil || pendingScrollLeft != nil), presenter?.collections.animating.contains(id) == true {
            presenter?.collections.stopAnimation(id)
        }
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
    var nativeFieldContent: CGRect?
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
            // `autocomplete` names the field to AutoFill over what `type` implies (LLP 1102 §3.6).
            let content = Autofill.contentType(props["autocomplete"], fallback: type == "password" ? .password : type == "email" ? .emailAddress : nil)
            let traitsChanged = f.autocapitalizationType != inputCapitalization || f.autocorrectionType != inputCorrection || f.spellCheckingType != inputSpellChecking || f.smartQuotesType != inputSmartQuotes || f.smartDashesType != inputSmartDashes || f.textContentType != content
            f.textContentType = content
            f.autocapitalizationType = inputCapitalization
            f.autocorrectionType = inputCorrection
            f.spellCheckingType = inputSpellChecking
            f.smartQuotesType = inputSmartQuotes
            f.smartDashesType = inputSmartDashes
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
            // HTML's `enterkeyhint` labels the return key (feed F25); UIKit
            // has no `previous`, which keeps the default.
            switch props["enterKeyHint"] {
            case "done": f.returnKeyType = .done
            case "go": f.returnKeyType = .go
            case "next": f.returnKeyType = .next
            case "search": f.returnKeyType = .search
            case "send": f.returnKeyType = .send
            case "enter", "previous": f.returnKeyType = .default
            default: f.returnKeyType = handlers.contains("submit") ? .go : .default
            }
            f.isEnabled = !disabled
            if isNativeTextControl { styleNativeField() }
        }
        if disabled { accessibilityTraits.insert(.notEnabled) } else { accessibilityTraits.remove(.notEnabled) }
        accessibilityIdentifier = props["testId"]
        accessibilityLabel = props["accessibilityLabel"]
        updateTextAccessibility()
        if actsAsButton {
            isAccessibilityElement = true
            accessibilityTraits.insert(kind == "view" && props["accessibilityRole"] == "link" ? .link : .button)
            if props["accessibilitySelected"] == "true" { accessibilityTraits.insert(.selected) } else { accessibilityTraits.remove(.selected) }
            setAccessibilityToggle(pressedState)
            if let checked = checkedRole { setAccessibilityChecked(checked.role, checked.checked) }
            if #available(iOS 18, tvOS 18, *) {
                accessibilityExpandedStatus = props["accessibilityExpanded"].map { $0 == "true" ? .expanded : .collapsed } ?? .unsupported
            }
        } else if props["accessibilityRole"] == "img" {
            // `role="img"` (an svg's) is one labelled image, as on the web (habits F16).
            isAccessibilityElement = authoredLabel != nil
            if authoredLabel != nil { accessibilityTraits.insert(.image) } else { accessibilityTraits.remove(.image) }
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
        setPaintPosition(paintZPosition)
        f.render(layer, clip: resolvedClipMask(), scale: window?.screen.scale ?? traitCollection.displayScale, dark: drawsDark)
    }

    package override func didMoveToSuperview() {
        super.didMoveToSuperview()
        paintOrderMoved()
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
        defer { video?.update(); applyCssVisibility(); if columnRecord?.columns.isEmpty == false { layoutColumnRules() } }
        let origin = style["transform_origin"]
        let old = style
        style = s
        if old["display"] != s["display"] || old["visibility"] != s["visibility"] { isHidden = hostHidden }
        if old["color_scheme"] != s["color_scheme"] { applyColorScheme() }
        updateSymbol(); syncDynamicRange(from: old)
        (clipPath, clipRule) = (ClipPath.path(s["clip_path"]), ClipPath.rule(s["clip_path"]))
        applyBoxMask()
        applyFilter()
        updateMaterial()
        syncVibrancy()
        syncScroll()
        applyAffordances()
        styleTextArea()
        field?.textAlignment = NSTextAlignment(rawValue: textAlignmentCode) ?? .left
        if let f = field, let t = text {
            f.font = t.font(size: number("font_size", PageFacts.defaultRootFontSize), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic", numeric: Int(number("font_variant_numeric")))
            f.textColor = color("text_color", SystemColor.canvasText)
            applyPlaceholder(f)
            styleNativeField()
        }
        if s["transform_origin"] != origin { applyTransform() }
        applySpace(changedFrom: old)
        setNeedsDisplay()
    }

    /// Scrolling and clipping come from the effective overflow the host
    /// wrote in (never from the node's kind): `scroll` on an axis makes a
    /// scroll container that scrolls that axis; `hidden` clips.
    /// Whether the style clips the children (`overflow: hidden`); a waiting
    /// scroll clips as its scroll view would. Without a clip box, it is the
    /// layer's `masksToBounds` (`syncScroll`; a landing flight, `FlightsIOS`).
    var overflowClips: Bool {
        let ox = style["overflow_x"]?.string ?? "visible", oy = style["overflow_y"]?.string ?? "visible"
        return ox == "hidden" || oy == "hidden" || scrollDormant
    }

    func syncScroll() {
        let ox = style["overflow_x"]?.string ?? "visible", oy = style["overflow_y"]?.string ?? "visible"
        let scrolls = (ox == "scroll" || ox == "auto") || (oy == "scroll" || oy == "auto")
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
        scroll?.scrollsX = (ox == "scroll" || ox == "auto")
        scroll?.scrollsY = (oy == "scroll" || oy == "auto")
        // UIKit's default indicator is already thin. CSS permits `thin`
        // to match `auto` on such platforms; `none` only hides the track.
        // Indicators and deceleration are the app's once a hatch sets them
        // (LLP 1075.003 §3.5): written when what the style says changes.
        let snap = style["scroll_snap_type"]?.string == "x mandatory"
        let indicators = (style["scrollbar_width"]?.string ?? "auto") != "none"
        if let sv = scroll, scrollWritten != "\(snap)|\(ox)|\(oy)|\(indicators)" {
            scrollWritten = "\(snap)|\(ox)|\(oy)|\(indicators)"
            sv.decelerationRate = snap ? .fast : .normal
            sv.showsHorizontalScrollIndicator = (ox == "scroll" || ox == "auto") && indicators
            sv.showsVerticalScrollIndicator = (oy == "scroll" || oy == "auto") && indicators
        }
        updateKeyboardDismissal()
        fitScroll()
        let clips = overflowClips
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
        scroll == nil && (["scroll", "auto"].contains(style["overflow_x"]?.string ?? "") || ["scroll", "auto"].contains(style["overflow_y"]?.string ?? ""))
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
        // Forced bounce on a carousel's computed y traps Mac wheel input; one travels a quarter of its width or lays a
        // row (itself, or a child wider than the port), and a page a few points too wide keeps its bounce.
        let travel = size.width - sv.bounds.width
        let row = { (v: NodeView) in v.style["display"]?.string == "flex" || v.style["flex_direction"] != nil ? !(v.style["flex_direction"]?.string ?? "row").hasPrefix("column") : false }
        let carousel = travel > 0.5 && (travel > sv.bounds.width / 4 || row(self) || sv.subviews.contains { ($0 as? NodeView).map { $0.frame.width > sv.bounds.width + 0.5 && row($0) } ?? false })
        sv.alwaysBounceVertical = sv.scrollsY && (size.height > sv.bounds.height + 0.5 || !carousel)
    }

    package override func layoutSubviews() {
        if focusRing != nil { showFocusRing(true) }
        if let s = presenter?.session, s.firstLayoutMs == nil { s.firstLayoutMs = ExactEnv.wall() }
        super.layoutSubviews()
        if materialView != nil { applyMaterialRadius() }
        syncEllipticalClip()
        if style["perspective"] != nil { applyPerspective() }
        if kind == "image" { if flightLook == nil { presenter?.session?.rasters.resized(self) }; if raster != nil { applyImageLayer() } }
        presenter?.collections.changed(id)
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
        layoutField()
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

    package override func draw(_ rect: CGRect) {
        repaintThrough(); syncDrawnRange()
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        if !cssVisibilityHidden, Capture.capturing, kind == "canvas", let picture = canvases?.picture(of: self) {
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
        paintBackground(ctx, border: path.cgPath, color: boxDrawn && !cssVisibilityHidden)
        if boxDrawn, !cssVisibilityHidden {
            // Sides that differ in colour or width, or a radius the layer
            // cannot say: each side in its colour, joined as the web joins
            // them (`BorderPaint`).
            let widths = ["top", "right", "bottom", "left"].map { number("border_width_" + $0, uniform) }
            let top = color("border_color_top", .clear)
            let colors = ["top", "right", "bottom", "left"].map { color("border_color_" + $0, top).cgColor }
            let radii = BorderPaint.radii(style, in: bounds)
            BorderPaint.paint(ctx, box: bounds, widths: widths, colors: colors, radii: radii, shape: CornerShape(style["corner_shape"]))
        }
        if !cssVisibilityHidden, kind == "image", symbolView == nil, flightLook == nil || imageLayer == nil, let bitmap = raster?.image {
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
            let object = RasterGeometry.rect(natural: bitmap.naturalSize, content: content, fit: fit)
            let rect = bitmap.displayRect(object)
            ctx.saveGState()
            ctx.clip(to: object)
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
                if let paragraph = paragraphLayout() { eachFragment(ctx) { box, at in
                    paintBackgroundThroughText(ctx, paragraph: paragraph, spec: spec, in: box)
                    TextEngine.draw(paragraph, spec: spec, in: box, context: ctx, dirty: rect.offsetBy(dx: -at.x, dy: -at.y)) } }
            }
        }
        if !cssVisibilityHidden, Capture.capturing, let picture = Capture.web[id] {
            // A capture that populated an arm snapshot draws that one WebKit
            // source at the node's hierarchy position (@ref LLP 1020 D4).
            picture.draw(in: bounds)
        }
    }

}
#endif
