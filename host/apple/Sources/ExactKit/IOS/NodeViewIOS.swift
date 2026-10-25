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
    // The flags, together after `id`: each `Bool` between wider fields
    // took a word of its own, and every view pays its stored properties
    // (a 768-byte block, from 1,024).
    var paintLifted = false
    var paintGhost = false
    /// The box is `draw(_:)`'s to paint: Core Animation cannot say it (`applyBoxLayer`).
    var boxDrawn = false
    /// The key is set and paints the whole paragraph (not a band of it).
    private(set) var textRasterWhole = false
    var textRasterReady = false
    var textRasterFailed = false
    var textLayoutValid = false
    /// What the host's own writers hid (a covered route, a tab a native control
    /// stands in for, a placement); a `display: none` box is hidden besides, as
    /// CSS removes it with its subtree, or its texts paint at its 0×0 frame's
    /// origin (recipes F19). Reading says whether it is hidden, for either reason.
    private var hostHidden = false
    var groupedOwner = false
    package var pressed = false { didSet { if pressed != oldValue { pressChanged() } } }
    /// A surface control's focus from its touch (SurfaceInput), which the
    /// author did not ask for: it does not keep a later autofocus off
    /// (Accessibility). Tab and `focus(id)` clear it (`chooseFocus`).
    package var focusedBySurfaceTouch = false
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
    var incarnation = NodePool.issue()
    /// Bumped where a touch's press contact here is cancelled for good
    /// (moved, out of the window, disabled, made inert): a captured agent
    /// press (`PressPath`) ends there too, even if the change is undone.
    var pressEpoch: UInt64 = 0
    var pressCancellationEpoch: UInt64 = 0
    weak var recognizedPress: RecognizedPress?
    // Twice the kernel rank; transients never overwrite the authored answer.
    var rank: Int64 = 0
    weak var paintParent: PaintView?
    let firstDraw: DrawReceipt?
    package let kind: String
    var inlineText: [InlineText] = []
    /// The rarely set fields (`NodeExtrasIOS.swift`); nil until one is set.
    var extras: NodeExtras?
    /// A box's gradient and shadows (`NodePaintExtras`); nil until one is set.
    var paintExtras: NodePaintExtras?
    /// A transform, an image, paint ghosts, a scroll's extent (`NodeAside`); nil until one is set.
    var aside: NodeAside?
    package override class var layerClass: AnyClass { NodeLayer.self }
    /// A uniform border under the children, where they can reach it.
    var boxBorder: CALayer?
    /// A paragraph's specs and raster, apart from the view (`TextSpecs`);
    /// nil for a node that never had text.
    var specs: TextSpecs?
    var textRasterKey: TextRasterKey? {
        get { specs?.rasterKey }
        set {
            if newValue != nil || specs != nil { textMore.rasterKey = newValue }
            textRasterWhole = newValue.map { $0.clip == nil } ?? false
        }
    }
    package var props: [String: String] = [:] { didSet { presenter?.propsChanged(self) } }
    package var style: NodeStyle = [:]
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
            let hidden = hostHidden || css
            let changed = super.isHidden != hidden
            super.isHidden = hidden
            if changed, scrollView != nil { presenter?.keyboardToolbars.backendChanged() }
        }
    }
    package var handlers: Set<String> = [] {
        didSet {
            updateContextGestures()
            updateSwipeGesture()
            updateLayoutPan()
            updateMaterial()
            updateRefresh()
            if handlers.contains("scroll") || handlers.contains("scrollend") { needScroll() }
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
        #if os(iOS)
        if gestureRecognizer === contextRecognizer {
            guard presenter?.menus.eligible(self, contextMenu: true) == true,
                  hasOwnContextMenu || inlineContextTarget(at: touch.location(in: self)) != nil else { return false }
            if !ExactEnv.agentMode {
                let run = inlineContextTarget(at: touch.location(in: self))
                if (run?.props["contextPopover"] ?? (run == nil ? props["contextPopover"] : nil))?.isEmpty == false { return false }
            }
            if let hit = touch.view as? NodeView, let source = hit.contextMenuSource(at: touch.location(in: hit)), source !== self { return false }
        }
        #endif
        if hitsNativeEditor(touch.view) { return false }
        var hit = touch.view
        while let current = hit, current !== self {
            if (gestureRecognizer === heightRecognizer || gestureRecognizer === transformRecognizer || gestureRecognizer === transformContact?.pinch), current is UIScrollView { return false }
            hit = current.superview
        }
        return true
    }
    /// A nested editor owns its selection gestures, including read-only
    /// text. Plain gestures and native context menus use the same boundary.
    func hitsNativeEditor(_ view: UIView?) -> Bool { Self.nativeEditor(view, below: self) }
    /// Whether `view` is in a native editor's view below `boundary`, which
    /// owns its selection gestures.
    static func nativeEditor(_ view: UIView?, below boundary: UIView) -> Bool {
        var hit = view
        while let current = hit, current !== boundary {
            if current is UITextView || current is UITextField { return true }
            hit = current.superview
        }
        return false
    }
    /// Whether a touch on `view` is native input's, which an ancestor's
    /// long press or double tap never takes, below `boundary`: a canvas that
    /// takes its own touches (`CanvasInputs`), or a native editor.
    static func nativeInputOwns(_ view: UIView?, below boundary: UIView) -> Bool {
        CanvasInputs.owns(view) || nativeEditor(view, below: boundary)
    }
    weak package var presenter: Presenter?
    /// The actual scroll port; a native list supplies its collection view.
    /// `scroll` remains the plain scroller that holds authored children.
    package var scrollView: UIScrollView? { extras?.nativeScrollView ?? scroll }
    package var scrollsVertically: Bool { ["scroll", "auto"].contains(style["overflow_y"]?.string ?? "") }
    package var ownsScrollDelegate: Bool {
        guard let sv = scrollView else { return false }
        return sv.delegate === (extras?.nativeScrollView == nil ? self : extras?.nativeScrollDelegate)
    }
    package var scroll: ScrollView? {
        didSet {
            if scrollView == nil { presenter?.scrollers.remove(id) }
            else { presenter?.scrollers.insert(id) }
            if scroll !== oldValue && extras?.nativeScrollView == nil { presenter?.keyboardToolbars.backendChanged() }
        }
    }
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
    var hidesAccessibility: Bool {
        placementHidden || props["inert"] == "true" || props["accessibilityElementsHidden"] == "true"
    }
    /// Marked before `becomeFirstResponder` runs: its `focus` handler can
    /// mount an autofocus field, whose pass must see the mark.
    @discardableResult
    func takeSurfaceTouchFocus() -> Bool {
        focusedBySurfaceTouch = true
        if becomeFirstResponder() { return true }
        focusedBySurfaceTouch = false
        return false
    }
    /// The press's feedback (LLP 1061 D2) and the author's pressed paint
    /// (D7), made at a node's first press: most nodes are never pressed, and
    /// inline the two were 200 of a node's 1,336 bytes, a 1,536-byte block
    /// where without them it takes one of 1,280.
    private final class Pressing {
        var feedback = PressFeedback()
        var paint = PressPaint()
    }
    private var pressing: Pressing?
    private var pressingMade: Pressing {
        if let pressing { return pressing }
        let made = Pressing()
        pressing = made
        return made
    }
    var press: PressFeedback {
        get { pressing?.feedback ?? PressFeedback() }
        set { if pressing != nil || !newValue.atRest { pressingMade.feedback = newValue } }
        _modify {
            if let pressing { yield &pressing.feedback; return }
            var feedback = PressFeedback()
            yield &feedback
            if !feedback.atRest { pressingMade.feedback = feedback }
        }
    }
    var pressPaint: PressPaint {
        get { pressing?.paint ?? PressPaint() }
        set { if pressing != nil || !newValue.atRest { pressingMade.paint = newValue } }
        _modify {
            if let pressing { yield &pressing.paint; return }
            var paint = PressPaint()
            yield &paint
            if !paint.atRest { pressingMade.paint = paint }
        }
    }
    package var disabled: Bool { props["disabled"] == "true" }
    /// HTML inertness covers the subtree, including direct agent activation.
    package var inert: Bool { presenter?.modals.inertBehind(self) == true || authoredInert }
    /// Inert by an `inert` attribute at or above it, whatever is presented:
    /// what an event bubbling out of a presentation still reaches.
    package var authoredInert: Bool {
        // Its logical ancestors, a presented route's included (LLP 1123 D2)
        (presenter?.logicalAncestry(of: self) ?? sequence(first: self as UIView, next: \.superview))
            .contains { ($0 as? NodeView)?.props["inert"] == "true" }
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
        get {
            guard kind == "native" else { return accessibilityExposed && super.isAccessibilityElement }
            // A native module's box (LLP 1024): the module's own elements
            // when its view has some; else the box, named by the Contract.
            return accessibilityExposed && (super.isAccessibilityElement || authoredLabel != nil) && presenter?.session?.natives.speaksForItself(self) != true
        }
        set { super.isAccessibilityElement = newValue }
    }
    /// A native module's box whose view has elements of its own, named by
    /// the Contract (`aria-label`): VoiceOver says the name entering them.
    package override var accessibilityContainerType: UIAccessibilityContainerType {
        get {
            guard kind == "native", authoredLabel != nil, presenter?.session?.natives.speaksForItself(self) == true else { return super.accessibilityContainerType }
            return .semanticGroup
        }
        set { super.accessibilityContainerType = newValue }
    }
    /// It hears focus, blur, keys or clipboard events (`focusEvents`), names a tabindex, or is a canvas taking input
    private var asksForFocus: Bool { reorderKeys || explicitTabIndex != nil || canvases?.wantsInput(id) == true || !handlers.isDisjoint(with: Self.focusEvents) }
    /// A tap focuses only a node that asked for focus: UIKit's buttons never become first responder from a
    /// touch, though Tab and `focus(id)` reach them
    var focusesOnPress: Bool { canBecomeFirstResponder && asksForFocus }
    package override var canBecomeFirstResponder: Bool { !formDisabled && !inert && !cssVisibilityHidden && !isHidden && field == nil && textArea == nil && (kind == "button" || isNativeButton || isRadio || asksForFocus) }
    package override func becomeFirstResponder() -> Bool {
        guard !formDisabled, !inert, !cssVisibilityHidden else { return false }
        let ok = super.becomeFirstResponder()
        if ok { presenter?.collections.pinsChanged(); inputCanvas?.canvasInput?.focused() }
        if ok, presenter?.menus.focus.quiet != true { presenter?.focus(id) } // a menu's set-aside focus returns quietly (MenuFocusIOS)
        return ok
    }
    package override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { showFocusRing(false) }
        if ok { presenter?.collections.pinsChanged() }
        if ok && !isSurfaceControl { inputCanvas?.canvasInput?.blur() }
        if ok { inputCanvas?.canvasInput?.focusLeft() }
        if ok, presenter?.menus.focus.quiet != true { presenter?.blur(id) }
        return ok
    }
    #if os(iOS)
    /// A focused node is not text entry, as the view's shortcut focus is not
    /// (ExactViewIOS): a menu that reloads its responder's input views (a
    /// `select`'s) raised the software keyboard over its options once a
    /// press had left a button focused (polls, LLP 1116 D2). A canvas
    /// taking input keeps the system's.
    private static let noKeyboard = UIView(frame: .zero)
    package override var inputView: UIView? { isFirstResponder && canvases?.wantsInput(id) != true ? Self.noKeyboard : nil }
    #endif
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
        if let name, activateKey(name) { return }
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
        get { textAccessibilityChildren() ?? super.accessibilityElements ?? NodeView.emptyBoxAccessibility(of: self) }
        set { super.accessibilityElements = newValue }
    }
    /// A text field's Enter as a key (its characters are its `input`, the
    /// Enter commits its `change`), then what Return does (`textFieldShouldReturn`).
    package func textFieldShouldBeginEditing(_ textField: UITextField) -> Bool { applyPadBar(); return !disabled && !inert }
    package func textViewShouldBeginEditing(_ textView: UITextView) -> Bool { !disabled && !inert }

    package func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        presenter?.focusAsked = nil
        guard !disabled, !inert else { return false }
        // The software keyboard's Return is a key the handlers hear first (a
        // hardware one's they heard in `pressesBegan`); prevented, it neither
        // commits nor submits.
        if (textField as? TextField)?.heard != "Enter", presenter?.keyDown(at: self, "Enter") == true { return false }
        presenter?.commitEdit(id, textField.text ?? "", change: handlers.contains("change"))
        // Return runs the field's `submit`, then puts the keyboard away, as
        // a hand-built field's delegate resigns it (LLP 1115.000 D9.2),
        // unless that action focused this field again: "add another" keeps
        // the keyboard and never lets go of it on the way. Next moves to the
        // next field instead, and from the last one puts the keyboard away.
        if handlers.contains("submit") { presenter?.submit(id) }
        if props["enterKeyHint"] == "next", presenter?.focusNextField(after: self) == true { return false }
        if presenter?.focusAsked !== self, textField.isFirstResponder { textField.resignFirstResponder() }
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
        if source.hasPrefix("symbol:") { presenter?.session?.rasters.cancel(id); raster = nil; updateSymbol(); return }
        loadGeneration += 1
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
        presenter.boxFilters.changed(self)
        return lease.image.naturalSize
    }

    // A symbol's box is Exact's; UIKit renders its glyph, including pixel alignment.
    func clearSymbol() {
        symbolView?.removeFromSuperview(); symbolView = nil; symbolKey = nil; symbolFound = false
    }
    func updateSymbol() {
        guard kind == "image", let source = imageSource, source.hasPrefix("symbol:") else { return }
        let name = props["symbolName"] ?? "", points = number("font_size", PageFacts.defaultRootFontSize), weight = number("font_weight", 400)
        let key = "\(source):\(name):\(points):\(weight):\(symbolLookKey)" + (props["symbolFillRule"].map { ":\($0)" } ?? "")
        if symbolKey != key {
            symbolKey = key; loadGeneration += 1
            let generation = loadGeneration
            // An icon with no SF Symbol draws its own path (D10, stage 9 A).
            image = name.isEmpty ? iconPathImage(side: points) : symbolImage(name, symbolShape(SymbolMeasure.configuration(points: points, weight: weight)))
            symbolFound = image != nil; if points <= 0 { image = nil }
            if name.isEmpty, props["symbolPath"] == nil, !source.hasPrefix("symbol:sf/"), source != "symbol:", symbolRefusal != source { symbolRefusal = source; presenter?.session?.log("image \(source) refused: unknown symbol role") }
            if !name.isEmpty || source.hasPrefix("symbol:sf/") || source == "symbol:" { symbolRefusal = nil }
            let leaf = symbolView ?? UIImageView()
            if symbolView == nil { symbolView = leaf; addSubview(leaf) }
            // What the image says is its node's (`aria-label`), never the
            // name UIKit gives the symbol, which a cell gathering its
            // children's words would read ("checkmark.circle" is "selected").
            showSymbol(image, on: leaf); leaf.isAccessibilityElement = false; leaf.accessibilityElementsHidden = true; leaf.isUserInteractionEnabled = false
            // The symbol's size when the kernel's measurer could not say
            presenter?.queueIntrinsicSize(self, generation: generation, SymbolMeasure.size(name, points: points, weight: weight))
        }
        symbolView?.tintColor = drawnSymbolInk
        symbolView?.preferredSymbolConfiguration = symbolConfiguration(SymbolMeasure.configuration(points: points, weight: weight))
        if let leaf = symbolView { applySymbolEffect(leaf) }
        layoutSymbol()
    }
    func layoutSymbol() {
        guard let leaf = symbolView else { return }
        let content = contentBox()
        leaf.frame = content; leaf.clipsToBounds = !listSymbol // a list cell's image is its own size, centred, as UIKit's
        switch listSymbol ? "none" : style["object_fit"]?.string ?? "fill" {
        case "cover": leaf.contentMode = .scaleAspectFill
        case "none": leaf.contentMode = .center
        case "scale-down":
            let size = image?.size ?? .zero
            leaf.contentMode = size.width <= content.width && size.height <= content.height ? .center : .scaleAspectFit
        // `contain`, and `fill`: a symbol is a vector glyph, which keeps its
        // aspect in a box of another shape as an SVG's default
        // `preserveAspectRatio` (`xMidYMid meet`) keeps it, and as the web
        // host's symbol mask draws it (LLP 1011 §4).
        default: leaf.contentMode = .scaleAspectFit
        }
        // An icon path is a vector: drawn again at the box it fills, not its em square stretched (UIKit scales a bitmap).
        if props["symbolPath"] != nil, let image {
            let side = max(content.width, content.height)
            // (Centred, it is its em square again, whatever the box was before.)
            if leaf.contentMode == .center { if leaf.image !== image { leaf.image = image } }
            else if side > 0, leaf.image?.size.width != side { leaf.image = iconPathImage(side: side) }
        }
        // A square box clips nothing the content box does not.
        if cornerRadii(in: bounds).allSatisfy({ $0 == 0 }) { if leaf.layer.mask != nil { leaf.layer.mask = nil }; return }
        let path = roundedPath(in: bounds).cgPath
        var transform = CGAffineTransform(translationX: -content.minX, y: -content.minY)
        let mask = CAShapeLayer(); mask.path = path.copy(using: &transform); leaf.layer.mask = mask
    }

    /// The view is gone: no load in flight may report for it.
    func forget() {
        holdGone() // out of the map: a hold on it is let go (HeldState.swift)
        cancelPressContact()
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
        buttonRasterView = nil
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
        firstDraw = presenter.session?.drawReceipt()
        self.kind = kind
        self.presenter = presenter
        super.init(frame: .zero)
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
        if kind == "input" { if presenter.fieldsWaiting.contains(id) { fieldWaits = true } else { makeField() } }
        presenter.leaves.embed(self) // a video's player, an iframe's web view, a module's box (LLP 1068 §5.1)
    }
    required init?(coder: NSCoder) { nil }

    /// Platform colours follow contrast and level too (LLP 1095 D5). UIKit
    /// keeps a registry of about 700 bytes for each view that registers for
    /// trait changes; this override hears the same changes for none.
    @available(iOS, deprecated: 17.0)
    @available(tvOS, deprecated: 17.0)
    package override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        super.traitCollectionDidChange(previous)
        guard let previous, SystemColor.resolvesDifferently(previous, traitCollection) else { return }
        // A control's accent, a tablist's tint and a grouped list's
        // switches resolve per appearance in their projections.
        if kind == "control" || kind == "list" || ["tablist", "radiogroup"].contains(props["accessibilityRole"] ?? "") {
            presenter?.requestProjectionSync()
        }
        paragraphOwner.invalidateText()
        paragraphOwner.setNeedsDisplay()
        applyStyle(style); refreshGroupedSeparator()
        presenter?.flats.appearanceChanged(id)
        // Its paint motion resolves in its own appearance (LLP 1062 D4),
        // a `color-scheme` above it included (LLP 1034 §8).
        presenter?.session?.noteAppearance(self)
        // An `svg`'s paints are resolved into its scene's layers.
        presenter?.svg.reappear(id, dark: drawsDark, clock: presenter?.session?.clock, tint: viewTint)
        presenter?.requestTextPublication()
        if let presenter, superview === presenter.root { presenter.paintCanvas() }
    }

    package override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil { pressEpoch &+= 1 }
        // Focused by its autofocus and moved out of the window in the same
        // turn, which resigned it: a sheet's route going into the controller
        // UIKit presents, a grouped list's row into its cell. Its autofocus
        // runs again where it lands, as a hand-built form's field made first
        // responder on present keeps the keyboard.
        if window == nil, let presenter, presenter.autofocusedThisTurn === self {
            presenter.autofocusedThisTurn = nil
            presenter.autofocusProcessed.remove(ObjectIdentifier(self))
        }
        if window == nil, pressContact?.routesBubbledPress == true { cancelPressContact() }
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
        if window != nil { presenter?.flushPendingFocus(); presenter?.elementScrolls.flush() }
        if window == nil, textRaster != nil { DispatchQueue.main.async { [weak self] in self?.dropTextRasterOffWindow() } }
    }
    /// Still off every window a turn later (a tab not shown, a covered
    /// route, a parked row; not a view moved within a batch), its text lets
    /// go of its pixels, as Core Animation lets go of a backing store there.
    /// `display` paints them again when it is back: at once where it shows.
    private func dropTextRasterOffWindow() {
        guard window == nil, textRaster != nil else { return }
        dropTextRaster()
        layer.setNeedsDisplay()
    }

    /// Glass content participates in UIKit's interactive effect. Other
    /// materials remain background siblings of the authored children. A
    /// glass group is innermost (`GlassGroup.swift`).
    package var container: UIView { glassGroupView?.contentView ?? baseContainer }
    /// Where layout put this box and where it is drawn (`PixelGridIOS.swift`).
    package var grid: GridPlace?

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
        guard scrollView === self.scrollView else { return }
        // A follow of the end that animates ends with its animation.
        if !decelerate, !followEndIfOwed() { endScroll() }
    }
    package func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) { if scrollView === self.scrollView, !followEndIfOwed() { scrollMayHaveEnded(scrollView) } }
    package func scrollViewDidEndScrollingAnimation(_ scrollView: UIScrollView) {
        guard scrollView === self.scrollView, presenter?.elementScrolls.cutting !== scrollView else { return }
        followingEndAnimated = false
        presenter?.elementScrolls.revealEnded(scrollView)
        // A smooth correction's driver ends its own motion (`OffsetDriver`);
        // a UIKit animation's end is not its end.
        if let c = presenter?.collections, c.offsetDrivers[id] != nil || c.startOwed.contains(id) { return }
        presenter?.collections.animationEnded(id, atTarget: endedAtTarget(scrollView))
        scrollMayHaveEnded(scrollView)
    }
    /// Whether it started an animation.
    @discardableResult private func followEndIfOwed() -> Bool {
        guard followsEndAfterInteraction, let sv = scrollView else { return false }
        followsEndAfterInteraction = false
        let maximum = max(-sv.adjustedContentInset.top, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height)
        anchoredScrollTop = maximum
        guard sv.contentOffset.y >= maximum - 80, abs(sv.contentOffset.y - maximum) > 0.5 else { return false }
        sv.setContentOffset(PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: maximum), in: sv), animated: true)
        return true
    }

    package func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        presenter?.elementScrolls.userScrolled(scrollView) // its own scroller, or a grouped list's collection
        guard scrollView === self.scrollView else { return }
        followingEndAnimated = false
        presenter?.collections.animationEnded(id, dragging: true)
        presenter?.collections.userIntent(id, travel: true)
        retainedScrollTop = nil
        #if os(iOS)
        if #available(iOS 27.1, *) { presenter?.arrangements.cancelScrollHandoff(id) }
        if swipeOwner { presenter?.swipeActions.painted.began(self) }
        #endif
    }

    /// A scroll under a canvas repaints it (LLP 1014 D4 c).
    package func scrollViewDidScroll(_ scrollView: UIScrollView) {
        guard scrollView === self.scrollView else { return }
        let post = Presenter.signposts.beginInterval("scrolled")
        defer { Presenter.signposts.endInterval("scrolled", post) }
        // The kernel's authored frames already include this content room.
        // Keep the physical UI inset basis, but do not count native padding twice.
        let room = extras?.nativeScrollContentInsets ?? .zero
        presenter?.onScrolled?(id, Double(scrollView.contentOffset.x + room.left), Double(scrollView.contentOffset.y + room.top + pageBand.top))
        presenter?.stickies.scrolled(id); presenter?.collections.changed(id, user: true)
        #if os(iOS)
        if swipeOwner, scrollView.isTracking { presenter?.swipeActions.painted.scrolled(self) }
        #endif
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
        presenter?.reaimFixedGradients()
        presenter?.scrollPump.scrolled(self)
        repaintThrough()
        scrollEndMoved(scrollView)
        // User scrolling is already a coherent position. Deliver before the
        // frame paints so authored scroll-linked geometry cannot lag a frame.
        // Layout-generated offsets still coalesce after the batch completes.
        if presenter?.applying == false && !dispatchingScrollEvent &&
            (scrollView.isTracking || scrollView.isDecelerating) {
            sendScrollEvent()
        } else { queueScrollEvent() }
    }
    func sendScrollEvent() {
        guard handlers.contains("scroll"), let sv = scrollView,
              presenter?.views[id] === self, hasScrollLayoutBox else { return }
        let point = sv.contentOffset, top = scrollTopInset(sv)
        guard point != lastScrollEvent.offset || top != lastScrollEvent.topInset else { return }
        lastScrollEvent = (point, top)
        dispatchingScrollEvent = true
        defer { dispatchingScrollEvent = false }
        // CSS's extents (`ScrollEvent`): the port inside UI insets (native
        // padding and footer room remain content), and the
        // port plus the range UIKit clamps a settled offset to, so at the
        // end `scrollHeight - scrollTop - clientHeight` is 0 (chat F4).
        let inset = sv.adjustedContentInset, port = scrollPort(sv).size
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
        // A true leaf cannot hit beyond its bounds; flat children can overflow.
        if !bounds.contains(point), subviews.isEmpty,
           presenter?.flats.holdsLeaves(id) != true { return nil }
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
            // A scroller's scroll view reaching into the page's bands takes them too (LLP 1130 D3).
            let band = scrollView == nil ? .zero : pageBand
            let outsideX = point.x < bounds.minX || point.x > bounds.maxX
            let outsideY = point.y < bounds.minY - band.top || point.y > bounds.maxY + band.bottom
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
                // (a native module's, paint F9) are not targets either. The
                // view holding its children (a clip that leaves its shadow
                // outside, a scroller) is not one, but leads to them, unless
                // hidden or refusing input: an `auto` descendant is still a target.
                if passesThrough, !(child is NodeView) {
                    let holder = self.container
                    guard holder !== self, holder === child || holder.isDescendant(of: child) else { return nil }
                    let chain = sequence(first: holder, next: { $0 === child ? nil : $0.superview })
                    guard !chain.contains(where: { $0.isHidden || !$0.isUserInteractionEnabled }) else { return nil }
                    return NodeView.hitChildren(in: holder, at: self.convert(point, to: holder), with: event)
                }
                return child.hitTest(self.convert(point, to: child), with: event)
            }) { return hit }
            // CSS `pointer-events: none` (inherited): the box is never the
            // target, so a touch goes to what is under it — a header's blur
            // over a list must not stop the list scrolling. A descendant
            // that sets `auto` again is still a target.
            if passesThrough { return nil }
            guard bounds.contains(point) else { return nil }
            if kind == "native", presenter?.session?.natives.declined(self, at: point, with: event) == true { return nil }
            if event?.type == .touches, isParagraph {
                touchedText()
                #if os(iOS)
                if let surface = presenter?.textSelection.text(for: self)?.document?.surface { return surface }
                #endif
            } // selectable text takes its interaction for this touch
            // Interactive glass (a pressable glass box) answers a finger
            // itself, swelling and lighting under it, only for touches that
            // reach its effect view. Its content view is the target; the
            // touch climbs the responder chain to this node, whose press is unchanged.
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
    /// CSS `::placeholder { color }` (LLP 1104 D3); nil, the platform's.
    var placeholderColor: UIColor? { cgColor("placeholder_color").map { UIColor(cgColor: $0) } }
    package func cgColor(_ key: String, dark: Bool? = nil) -> CGColor? { style[key].flatMap { $0.cgColor(dark: dark ?? drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: ownTint(for: $0)) } }
    package func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
        if let n = style[key]?.number { return CGFloat(n) }
        return fallback
    }

    func applyPlaceholder(_ f: UITextField) {
        let text = props["placeholder"] ?? ""
        let font = f.font ?? UIFont.systemFont(ofSize: 17)
        if text.isEmpty {
            f.attributedPlaceholder = nil
            f.placeholder = nil
            return
        }
        // UIKit's placeholder colour, which follows Increase Contrast: on a
        // native field (D3), and on a bare one whose `color` is not its own
        // (LLP 1115 §2, "inherited is unsaid": a grouped list's row gives
        // its field the label colour). A bare field that writes its own
        // colour mutes that ink.
        // An authored `::placeholder` colour wins, its alpha its own (LLP 1104 D3).
        let ink = placeholderColor ?? (isNativeTextControl || ownColor("text_color") == nil ? ThemeRoles.color("placeholder", in: self, or: UIColor.placeholderText) : (f.textColor ?? SystemColor.canvasText).withAlphaComponent(0.30))
        f.attributedPlaceholder = NSAttributedString(string: text, attributes: [
            .font: font,
            .foregroundColor: ink,
        ])
    }

    // A hidden or host-replaced box retains its scroll position while native layout
    // collapses its extent. Restore it and resume scroll events when the box returns.
    var hasScrollLayoutBox: Bool {
        var ancestor: UIView? = self
        while let current = ancestor {
            if let node = current as? NodeView,
               node.style["display"]?.string == "none" || node.presenter?.navigation.covers[node.id] == .whole { return false }
            ancestor = current.superview
        }
        return true
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
        defer { syncGlassSlot(); syncGlassGroup(); settleVibrancy(); syncDefaultSurfaceScroll() }
        let kind = materialRequest
        let supported = kind != nil
        let interactive = Materials.glass(kind) && handlers.contains("press") && !disabled
        if materialKind != (supported ? kind : nil) {
            let children = container.subviews.compactMap { $0 as? NodeView }
            let old = materialView
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
            // The children move before the old effect goes, so they never
            // leave the window: UIKit cancels the touches of a view that
            // does, and a press held inside would end at the swap (euxy's
            // dice, whose charge turned the capsule's glass off).
            GlassGroups.moving(in: self) {
                for (index, child) in children.enumerated() { container.insertSubview(child, at: index) }
                old?.removeFromSuperview()
            }
            presenter?.flats.containerChanged(id)
        }
        guard let materialView else { rehomeMaterialChildren(); return }
        if cssVisibilityHidden { materialView.effect = nil }
        else if materialView.effect == nil || materialInteractive != interactive || backdropStale {
            materialView.effect = backdropEffect() ?? materialEffect(kind ?? "ultra-thin", interactive: interactive)
            materialInteractive = interactive
        }
        applyMaterialRadius()
        rehomeMaterialChildren()
    }
    func applyMaterialRadius() {
        guard let materialView else { return }
        let radius = BorderPaint.clip(materialView.layer, in: bounds, radii: effectiveCornerSizes(in: bounds))
        if #available(iOS 26.0, tvOS 26.0, *) {
            materialView.cornerConfiguration = .corners(radius: .fixed(Double(radius)))
        } else {
            materialView.layer.cornerRadius = radius
            materialView.clipsToBounds = true
        }
        if style["mask_image"] != nil { applyBoxMask() }
    }
    var kernelContentBox: CGRect?
    /// The padding layout resolved and sent with `kernelContentBox`: top,
    /// right, bottom, left (`nil` on a native button's unsaid side).
    var kernelPadding: [CGFloat?]?
    var pendingScrollLeft: Double? {
        get { extras?.pendingScrollLeft }
        set { if newValue != nil || extras != nil { more.pendingScrollLeft = newValue; more.pendingScrollTransfer = false }; presenter?.pendingScrolls.insert(id) }
    }
    var pendingScrollTop: Double? {
        get { extras?.pendingScrollTop }
        set { if newValue != nil || extras != nil { more.pendingScrollTop = newValue; more.pendingScrollTransfer = false }; presenter?.pendingScrolls.insert(id) }
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
        #if os(iOS)
        let nativeList = kind == "list" && next["listStyle"] != nil
        if groupedOwner != nativeList { groupedOwner = nativeList; syncScroll() }
        #endif
        if disabled { pressEpoch &+= 1; cancelPressContact() }
        if props["inert"] == "true" { cancelPressContactsInSubtree() }
        swipeOwner = props["swipeContent"] != nil
        if set["groupedSeparator"] != nil || clear.contains("groupedSeparator") { applyGroupedSeparator() }
        if set["symbolEffectValue"] != nil || set["symbolColorWritten"] != nil || clear.contains("symbolColorWritten")
            || set["symbolFillRule"] != nil || clear.contains("symbolFillRule") { updateSymbol() }
        if (pendingScrollLeft ?? 0) != 0 || (pendingScrollTop ?? 0) != 0 { needScroll() }
        if set["inert"] != nil || clear.contains("inert") {
            let ownInert = props["inert"] == "true"
            if ownInert { endEditing(true) }
            isUserInteractionEnabled = !ownInert
        }
        // UIKit keeps each accessibility property a view is given, an unchanged
        // one too, in an associated object table: so each is set only on change.
        if accessibilityElementsHidden != hidesAccessibility { accessibilityElementsHidden = hidesAccessibility }
        updateKeyboardDismissal()
        updateMaterial()
        applyTextArea()
        if let f = field { applyField(f, set: set, clear: clear) }
        if accessibilityTraits.contains(.notEnabled) != (disabled || ariaUnavailable) { accessibilityTraits.formSymmetricDifference(.notEnabled) }
        if accessibilityIdentifier != props["testId"] { accessibilityIdentifier = props["testId"] }
        if accessibilityLabel != props["accessibilityLabel"] { accessibilityLabel = props["accessibilityLabel"] }
        updateTextAccessibility()
        if actsAsButton {
            isAccessibilityElement = true
            // A `link`, pressable or a box with the role, is a link to VoiceOver, never also a button (LLP 1115.000 D11.6).
            let link = props["accessibilityRole"] == "link"
            accessibilityTraits.insert(link ? .link : .button); accessibilityTraits.remove(link ? .button : .link)
            // `aria-current` (not `false`) is the selected trait too: UIKit has no
            // current, and a bar's current item is its selected one (theme2 2026-10-22 06:40).
            let current = props["accessibilityCurrent"].map { !["", "false"].contains($0) } ?? false
            if props["accessibilitySelected"] == "true" || current { accessibilityTraits.insert(.selected) } else { accessibilityTraits.remove(.selected) }
            setAccessibilityToggle(pressedState)
            if let checked = checkedRole { setAccessibilityChecked(checked.role, checked.checked) }
            if #available(iOS 18, tvOS 18, *) {
                accessibilityExpandedStatus = props["accessibilityExpanded"].map { $0 == "true" ? .expanded : .collapsed } ?? .unsupported
            }
        } else if rangeRole != nil {
            // A range role is one element and its value (LLP 1116 D8).
            isAccessibilityElement = true
            accessibilityTraits.remove(.image)
            accessibilityValue = rangeValueText
        } else if kind == "image" || props["accessibilityRole"] == "img" {
            updateImageAccessibility()
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
        // Paragraph ops publish the new ink separately. A change of words
        // leaves the box's paint alone; bitmap text still needs its draw.
        let wordsOnly = isParagraph && set.keys.allSatisfy { $0 == "text" } && clear.allSatisfy { $0 == "text" }
        if !wordsOnly || !canRasterText { setNeedsDisplay() }
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

    /// Inside `GlassGroups.moving`: a container comes or goes under the
    /// same authored parent, which is no reparenting.
    static var containerMoves = 0
    package override func didMoveToSuperview() {
        if Self.containerMoves == 0 { pressEpoch &+= 1; cancelPressContact(reparenting: true) }
        super.didMoveToSuperview()
        paintOrderMoved()
        if superview == nil { boxFilter?.remove() } else if boxFilter != nil { renderFilter() }
        // A box styled before it joined its parent learns its material now.
        if superview != nil { syncVibrancy() }
        if superview != nil, style["color_scheme"] != nil || overrideUserInterfaceStyle != .unspecified { settleColorScheme() }
    }
    func applyStyle(_ s: NodeStyle) {
        defer { video?.update(); layoutContent(); applyCssVisibility(); if columnRecord?.columns.isEmpty == false { layoutColumnRules() } }
        let origin = style["transform_origin"]
        let old = style
        style = s
        if old["display"] != s["display"] || old["visibility"] != s["visibility"] { isHidden = hostHidden }
        if old["zoom"] != s["zoom"], kind == "iframe" { updateEmbedded() } // the guest's page zoom (LLP 1020.002 D6)
        if old["color_scheme"] != s["color_scheme"] { applyColorScheme() }
        if old["accent_color"] != s["accent_color"] { applyAccentTint() }
        updateSymbol(); syncDynamicRange(from: old)
        (clipPath, clipRule) = (ClipPath.path(s["clip_path"]), ClipPath.rule(s["clip_path"]))
        applyBoxMask()
        applyFilter()
        updateMaterial()
        syncVibrancy()
        presenter?.accent.restyled(self)
        syncScroll()
        applyAffordances()
        styleTextArea()
        applyFieldStyle()
        if s["transform_origin"] != origin { applyTransform() }
        applySpace(changedFrom: old)
        decorationChanged(from: old)
        setNeedsDisplay()
    }

    /// Scrolling and clipping come from the effective overflow the host
    /// wrote in (never from the node's kind): `scroll` on an axis makes a
    /// scroll container that scrolls that axis; `hidden` clips.
    /// Whether the style clips the children (`overflow: hidden` or `clip`); a waiting
    /// scroll clips as its scroll view would. Without a clip box, it is the
    /// layer's `masksToBounds` (`syncScroll`; a landing flight, `FlightsIOS`).
    var overflowClips: Bool {
        let ox = style["overflow_x"]?.string ?? "visible", oy = style["overflow_y"]?.string ?? "visible"
        return ["hidden", "clip"].contains(ox) || ["hidden", "clip"].contains(oy) || scrollDormant
    }

    func syncScroll() {
        let ox = style["overflow_x"]?.string ?? "visible", oy = style["overflow_y"]?.string ?? "visible"
        let scrolls = (ox == "scroll" || ox == "auto") || (oy == "scroll" || oy == "auto")
        if scrolls { syncClipBox(false) }
        if scrolls && scroll == nil && !scrollWaits && !groupedOwner {
            if pageBand != .zero, extras?.nativeScrollView != nil { setPageBand(.zero) } // the list leaving keeps its scrollTop
            let sv = ScrollView(frame: bounds)
            sv.backgroundColor = .clear
            sv.contentInsetAdjustmentBehavior = .never
            sv.delegate = self
            sv.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            GlassGroups.moving(in: self) {
                // In the window first, as `syncClipBox` says.
                let children = subviews.filter { $0 is NodeView }
                addSubview(sv)
                for child in children { sv.addSubview(child) }
            }
            scroll = sv
            if pageBand != .zero || bandInset != .zero { more.pageBand = .zero; more.bandInset = .zero } // a new scroll view reaches no band until asked (LLP 1130 D3)
            scrollWritten = nil
            updateRefresh()
            presenter?.navigation.scrollBackendChanged(self)
        }
        if !scrolls || groupedOwner, let sv = scroll {
            if groupedOwner { retainScrollPosition(from: sv) }
            // Neither axis scrolls any more: the children come back out.
            GlassGroups.moving(in: self) {
                for child in sv.subviews where child is NodeView { addSubview(child) }
                sv.removeFromSuperview()
            }
            scroll = nil
            if pageBand != .zero || bandInset != .zero { more.pageBand = .zero; more.bandInset = .zero }
        }
        scroll?.scrollsX = (ox == "scroll" || ox == "auto")
        scroll?.scrollsY = (oy == "scroll" || oy == "auto")
        // UIKit's default indicator is already thin. CSS permits `thin`
        // to match `auto` on such platforms; `none` only hides the track.
        // Indicators and deceleration are the app's once a hatch sets them
        // (LLP 1075.003 §3.5): written when what the style says changes.
        let snap = style["scroll_snap_type"]?.string == "x mandatory"
        let indicators = (style["scrollbar_width"]?.string ?? "auto") != "none"
        if let sv = scrollView, scrollWritten != "\(snap)|\(ox)|\(oy)|\(indicators)" {
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

    var scrollWaits: Bool { swipeOwner && !scrollNeeded && !handlers.contains("scroll") && !handlers.contains("scrollend") }
    /// The node's overflow scrolls, and its scroll view is still waiting.
    var scrollDormant: Bool {
        !groupedOwner && scrollView == nil && (["scroll", "auto"].contains(style["overflow_x"]?.string ?? "") || ["scroll", "auto"].contains(style["overflow_y"]?.string ?? ""))
    }

    func needScroll() {
        guard scrollDormant else { return }
        scrollNeeded = true
        syncScroll()
    }

    /// The scroll container's content size: the kernel's extent on an axis
    /// that scrolls, the box on one that does not (so UIKit cannot pan it
    /// there), never less than the box (or the port UIKit's insets leave, `scrollFloor`).
    func fitScroll() {
        guard let sv = scroll else { return }
        // Ending where its last edge is drawn, from this box's place on the
        // pixel grid, so the furthest offset is a whole pixel and reaches it (`PixelGridIOS.swift`).
        let s = presenter?.gridScale ?? 1, o = grid?.origin ?? .zero
        let size = CGSize(width: sv.scrollsX ? max(PixelGrid.length(content.width, from: o, vertical: false, scale: s), sv.bounds.width) : sv.bounds.width,
                          height: sv.scrollsY ? max(PixelGrid.length(content.height, from: o, vertical: true, scale: s), scrollFloor(sv)) : sv.bounds.height)
        if sv.contentSize != size { sv.contentSize = size }
        // Forced bounce on a carousel's computed y traps Mac wheel input; one travels a quarter of its width or lays a
        // row (itself, or a child wider than the port), and a page a few points too wide keeps its bounce.
        let travel = size.width - sv.bounds.width
        let row = { (v: NodeView) in v.style["display"]?.string == "flex" || v.style["flex_direction"] != nil ? !(v.style["flex_direction"]?.string ?? "row").hasPrefix("column") : false }
        let carousel = travel > 0.5 && (travel > sv.bounds.width / 4 || row(self) || sv.subviews.contains { ($0 as? NodeView).map { $0.frame.width > sv.bounds.width + 0.5 && row($0) } ?? false })
        // One that fits inside an author's scroller that scrolls down leaves
        // the drag to it, as a nested UIScrollView does (an `EmptyState` in a
        // page's `scroll`). The presenter's viewport is no author's.
        let fits = size.height <= sv.bounds.height + 0.5
        // Pull to refresh needs its own drag.
        var nested = false, up = fits && !handlers.contains("refresh") ? sv.superview : nil
        while let v = up, !nested {
            if let outer = v as? ScrollView, !(outer is Viewport) { nested = outer.scrollsY }
            up = v.superview
        }
        sv.alwaysBounceVertical = sv.scrollsY && !nested && (!fits || !carousel)
    }

    package override func layoutSubviews() {
        if focusRing != nil { showFocusRing(true) }
        if let s = presenter?.session, s.firstLayoutMs == nil { s.firstLayoutMs = ExactEnv.wall() }
        super.layoutSubviews()
        if materialView != nil { applyMaterialRadius(); syncDefaultSurfaceScroll() } // its corners, at its size
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
        layoutContent()
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
        if presenter?.views[id] === self { firstDraw?.drawn() }
        let path = roundedPath(in: bounds)
        let uniform = number("border_width")
        // A box Core Animation can say is the layer's (`applyBoxLayer`);
        // the background within its `background-clip` (LLP 1077 D6).
        paintBackground(ctx, border: path.cgPath, color: boxDrawn && !cssVisibilityHidden && presenter?.segments.control(of: id) == nil)
        if boxDrawn, !cssVisibilityHidden {
            // Sides that differ in colour or width, or a radius the layer
            // cannot say: each side in its colour, joined as the web joins
            // them (`BorderPaint`).
            let widths = ["top", "right", "bottom", "left"].map { devicePixels(number("border_width_" + $0, uniform)) }
            let top = color("border_color_top", .clear)
            let colors = ["top", "right", "bottom", "left"].map { color("border_color_" + $0, top).cgColor }
            let radii = BorderPaint.radii(style, in: bounds)
            let styles = ["top", "right", "bottom", "left"].map { style["border_style_" + $0]?.string ?? "solid" }
            BorderPaint.paint(ctx, box: bounds, widths: widths, colors: colors, radii: radii, styles: styles, shape: CornerShape(style["corner_shape"]))
        }
        if !cssVisibilityHidden, kind == "image", symbolView == nil, flightLook == nil || imageLayer == nil, let bitmap = raster?.image {
            // CSS object-fit over the content box (the frame inside border
            // and padding), clipped by the border box's radius: `fill`
            // stretches, `contain`/`cover` keep the ratio, `none` is the
            // natural size, `scale-down` the smaller of none and contain;
            // an unknown value is the initial `fill`.
            let fit = style["object_fit"]?.string ?? "fill"
            let content = contentBox()
            let rect = RasterGeometry.rect(natural: bitmap.naturalSize, content: content, fit: fit)
            ctx.saveGState()
            path.addClip()
            UIBezierPath(rect: content).addClip()
            ctx.translateBy(x: rect.minX, y: rect.maxY)
            ctx.scaleBy(x: 1, y: -1)
            RasterGeometry.draw(ctx, AnimatedRasters.shared.frame(for: self) ?? bitmap.image, in: CGRect(origin: .zero, size: rect.size), tint: channels("tint_color").map { TextEngine.color($0).cgColor })
            ctx.restoreGState()
        }
        if isParagraph, rollsNumerals, let engine = text {
            dropTextRaster()
            NumeralLink.installed?.show(self, attributed: engine.attributed(paragraphSpec()), box: contentBox(),
                                        align: paragraphSpec().align, mode: style["content_transition"]?.string ?? "numeric")
        } else if isParagraph {
            NumeralLink.installed?.hide(self)
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
