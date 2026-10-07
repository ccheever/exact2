// The UIKit presenter (LLP 1008 §9): the viewport scroll view over a
// content-sized document, one `NodeView` per kernel node, the host's
// batches applied, the keyboard's inset and the field it reveals. It
// belongs to one session (LLP 1031 D1) and reaches the session's canvases,
// web views, and menus through it.
#if os(iOS) || os(tvOS)
import UIKit
import CoreText
import os

package final class Presenter {
    var documentLanguage = ""
    var documentDirection = "ltr"
    static let signposts = OSSignposter(subsystem: "com.exact.host", category: "scroll")

    var autofocusProcessed: Set<ObjectIdentifier> = []
    /// Set the turn after the session's first activation. A booted session's autofocus waits for it.
    var launchAutofocusReleased = false
    private var projectionSyncOwed = false
    /// What the native projections show changed outside a batch (a subtree's
    /// appearance or size traits, geometry a sheet replayed after the batch).
    /// Run the projection steps an empty batch used to run, once, on the next
    /// turn, without waiting for a batch an idle app may never commit (LLP
    /// 1079's amendment of 2026-10-04): grouped lists remount (frames, cell
    /// heights, switches), then tab bars and controls.
    func requestProjectionSync() {
        guard !projectionSyncOwed else { return }
        projectionSyncOwed = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            projectionSyncOwed = false
            guard !applying else { return }
            // Rows stay carried: a trait refresh never pulls a row (and a
            // first responder or a touch in it) out of its cell. Lists mount
            // first, so a segment or control in a carried row is judged
            // where it shows.
            groupedLists?.sync(changed: [])
            segments.sync()
            controls.sync()
        }
    }
    /// The `aria-modal` view VoiceOver was last moved into, and the views
    /// made modal, which a cleared prop drops from the index (`syncModal`).
    var announcedModal: (id: UInt32, incarnation: UInt64)?
    /// The `.screenChanged` post, replaceable in tests.
    static var postScreenChanged: (Any?) -> Void = { UIAccessibility.post(notification: .screenChanged, argument: $0) }
    let modalViews = NSHashTable<NodeView>.weakObjects()
    /// The session this presenter shows (LLP 1031 D1).
    weak package var session: ExactSession?
    /// The document: the roots live here, content-sized like a page.
    let root = PlainView(frame: .zero)
    /// The viewport over it: the window's content, scrolling like a browser's.
    let viewport: ScrollView = Viewport(frame: .zero)
    package var views: [UInt32: NodeView] = [:]
    /// Views leaving with their exit, by id (LLP 1063, `PresenceIOS.swift`).
    var leaving: [UInt32: Leaving] = [:]
    /// Shared elements in flight, by the arriver's id (LLP 1013.000, `FlightsIOS.swift`).
    var flights: [UInt32: Flight] = [:]
    private(set) var chrome = ChromeIndex()
    func propsChanged(_ view: NodeView) { chrome.note(view.id, props: view.props); view.updateReorderGesture(); view.updateRefresh() }
    package func carrying(_ key: String) -> [NodeView] { chrome.ids(key).sorted().compactMap { views[$0] } }
    func takeChangedNames() -> Set<String> { chrome.takeChangedNames() }
    var scrollers: Set<UInt32> = []
    var pendingScrolls: Set<UInt32> = []
    /// The batch's suppression triggers for scroll anchoring (`ScrollAnchoring.swift`).
    var anchorChanges = ScrollAnchoring.Changes()
    var materialNodes: Set<UInt32> = []
    let glassGroups = GlassGroups()
    var contextNodes: Set<UInt32> = []
    var inlineOwners: [UInt32: (owner: UInt32, index: Int)] = [:]
    var heightBindings: [UInt32: HeightDragBinding] = [:]
    var transformBindings: [UInt32: TransformDragBinding] = [:]
    /// The one Arrange contact, until its source settles; a test's calls.
    var reorder: ReorderHold?
    var reorderCalls: ReorderCalls?
    /// A grouped session (LLP 1094), until its ghost lands; a test's calls.
    var reorderGroup: ReorderGroupHold?
    var reorderGroupCalls: ReorderGroupCalls?
    lazy var transformGeometry = TransformGeometryHost(self)
    /// Nodes showing a `background-attachment: fixed` gradient (LLP 1066
    /// D7): re-aimed at the viewport when anything scrolls or a batch lands.
    let fixedGradients = NSHashTable<NodeView>.weakObjects()
    func reaimFixedGradients() {
        guard fixedGradients.count > 0 else { return }
        for node in fixedGradients.allObjects where node.window != nil { node.reaimFixedGradient() }
    }
    var videoVisibility: VideoVisibilityHost?
    lazy var collections = CollectionHost(self)
    lazy var stickies = StickyHost(self)
    lazy var pool = NodePool(self)
    /// Heavy leaves held mid-fling (LLP 1068 §5.1).
    lazy var leaves = HeavyLeaves(self)
    lazy var flats = FlatLeaves(self)
    /// The native menu arm (LLP 1021 D3).
    lazy var swipeActions = SwipeActionsHost(self)
    lazy var menus = MenuHost(presenter: self)
    lazy var keyboardToolbars = KeyboardToolbars(self)
    lazy var segments = SegmentHost(self)
    /// Present only when the composition links them (LLP 1047.001 D4).
    package lazy var groupedLists: GroupedLists? = GroupedListsLink.make?(self)
    lazy var controls = ControlHost(self)
    lazy var fieldSelections = FieldSelections(self)
    /// Nodes marked `hatch="word"` (LLP 1075.003.000).
    lazy var elements = ElementHatches(self)
    lazy var navigation = NavigationHost(presenter: self)
    #if os(tvOS)
    lazy var menuKey = MenuKey(presenter: self)
    lazy var focusGuides = FocusGuides(presenter: self)
    lazy var playPauseKey = PlayPauseKey(presenter: self)
    /// The `testId` of the node that last held the remote's focus.
    var focusKey: String?
    #endif
    lazy var modals = ModalHost(presenter: self)
    /// What a capability module sees of the viewport and the keyboard's
    /// container (LLP 1047.001 D4): UIKit's types, not the core's.
    package var viewportScroll: UIScrollView { viewport }
    package var keyboardContainer: UIView? { modals.coordinateView ?? session?.view }
    /// SVG scenes and CSS animations (LLP 1055 D4, D7).
    let svg = SvgHost()
    /// Boxes under CSS `filter`, drawn again after each batch (LLP 1055.000 D14).
    let boxFilters = BoxFilters()
    let canvas2d = Canvas2DHost()
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
    /// The posture and the viewport segments last told (LLP 1078 D5), for `layout.env`.
    var fold = ViewportFold.flat
    /// Whether the device has a fold — UIKit 27.1 reported a division region
    /// or a hinge — in which case `prefer posture`/`segments` is refused:
    /// the device decides (LLP 1078 D7).
    var hasFold = false
    /// Whether `UIHingeInteraction` has reported once (a hinge or none):
    /// before that, a 27.1 device's fold is unknown (LLP 1078 D7).
    var hingeReported = false
    /// The keyboard's inset on the viewport: the points of the screen's
    /// viewport a software keyboard covers. By default the web's visual
    /// viewport — the layout viewport does not change; the viewport insets
    /// its content by this and reveals the field being edited, in the
    /// keyboard's own animation. Under `resizes-content` the controller
    /// sets it, measured against the viewport it would frame without a
    /// keyboard.
    var keyboardInset: CGFloat = 0

    var hasKeyboardEditor: Bool {
        views.values.contains { $0.field?.isFirstResponder == true || $0.textArea?.isFirstResponder == true }
            || hasNativeKeyboardEditor
    }

    /// A native module's own text input (a composer's UITextView) focused
    /// inside this viewport: the keyboard is this session's business as for
    /// its own fields, so the page resizes above it.
    var hasNativeKeyboardEditor: Bool {
        guard let responder = FirstResponder.current as? UIView, responder is UIKeyInput else { return false }
        return responder.isDescendant(of: viewport)
    }

    /// A notification describes the keyboard's target, not its current
    /// presence: UIKit announces hiding even during a cancelled sideways pop.
    /// The guide observes the keyboard in its owning container's coordinates.
    package func keyboardGuideTop(in container: UIView) -> CGFloat? {
        #if os(tvOS)
        // tvOS has no keyboard layout guide.
        return nil
        #else
        guard hasKeyboardEditor || keyboardInset > 0 else { return nil }
        let guide = container.keyboardLayoutGuide.layoutFrame
        return guide.height > container.safeAreaInsets.bottom + 1 ? guide.minY : nil
        #endif
    }

    init() {
        viewport.addSubview(root)
        viewport.delegate = scrollPump
        collections.motion = { [unowned self] id in
            if collections.animating.contains(id) { return nil }
            let velocity = scrollPump.velocity(id)
            return velocity == 0 ? nil : velocity
        }
        collections.requestFill = { [unowned self] in scrollPump.requestFill() }
        // Rows a rescue built show this frame: their text paints now, not
        // after the commit, or they show without it (LLP 1050.000 D1).
        collections.rescued = { [unowned self] in paintVisibleText() }
        viewport.contentInsetAdjustmentBehavior = .never
        viewport.backgroundColor = .white
    }

    func observeKeyboard() {
        // tvOS has no keyboard frame notifications.
        #if !os(tvOS)
        let c = NotificationCenter.default
        c.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
        c.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillHideNotification, object: nil)
        #endif
    }

    #if !os(tvOS)

    /// The keyboard is about to move: inset the viewport by what it will
    /// cover and reveal the field, inside an animation with the keyboard's
    /// own duration and curve — Core Animation runs both in the same
    /// transaction, so the content moves in lockstep with the keyboard,
    /// never a frame behind it.
    @objc func keyboardChanged(_ n: Notification) {
        guard let info = n.userInfo, let window = viewport.window, viewport.superview != nil else { return }
        // A keyboard is this session's business only for its own editor, or
        // while it still holds an inset it applied; another session's editor
        // in the same window is not (LLP 1035.001 D5, two-session host).
        let ownEditor = editing != nil || hasKeyboardEditor
        guard NavigationRules.keyboardConcerns(editing: ownEditor, holdsInset: keyboardTop != nil) else { return }
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
        if duration > 0 || interactiveKeyboardDrag {
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
    #endif
    /// The last no-duration keyboard change, waiting to be applied.
    private var keyboardDebounce: DispatchWorkItem?
    var hasPendingKeyboardResize: Bool { keyboardDebounce != nil }
    var interactiveKeyboardDrag: Bool {
        views.values.contains { node in
            guard let sv = node.scroll else { return false }
            return sv.keyboardDismissMode == .interactive && (sv.isTracking || sv.isDragging)
        }
    }

    /// The keyboard's top edge (nil: hidden) takes effect: the viewport is
    /// inset by the overlap (`resizes-visual`, the default) or laid out to
    /// end there (`resizes-content`, the controller's `fit`), the field
    /// being edited revealed after — inside an animation with the keyboard's
    /// own duration and curve, so the frames the batch sets are Core
    /// Animation moves in the keyboard's transaction, never a frame behind.
    func applyKeyboard(top: CGFloat?, duration: Double, curve: UInt) {
        // Removing a focused view can synchronously announce keyboard hiding.
        // Finish its batch before resizing; otherwise that older batch can
        // overwrite the new viewport's frames after this call returns.
        if applying {
            waiting.append((nil, { [weak self] in self?.applyKeyboard(top: top, duration: duration, curve: curve) }))
            return
        }
        guard let window = viewport.window, let parent = viewport.superview else { return }
        keyboardTop = top
        let change = {
            if self.interactiveWidget == "resizes-content" {
                self.onKeyboardResize?()
            } else if self.interactiveWidget == "overlays-content" {
                let frame = parent.convert(self.viewport.frame, to: window)
                let overlap = top.map { min(max(0, frame.maxY - max($0, frame.minY)), frame.height) } ?? 0
                self.keyboardInset = overlap
                self.keyboardToolbars.ride(overlap: overlap)
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
        if ExactEnv.agentFreezes || duration <= 0 { change(); return }
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
            var target = cur as? ScrollView
            if let waiting = cur as? NodeView, waiting.scrollDormant { waiting.needScroll(); target = waiting.scroll }
            if let sv = target {
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

    /// The viewport's size in points: what the kernel lays out under.
    var viewportSize: CGSize { viewport.bounds.size }
    /// The first root's frame size, zero before the first batch.
    var rootSize: CGSize { root.subviews.first?.frame.size ?? .zero }

    /// An asset's bytes changed (LLP 1030 D10): every image showing it loads
    /// it again — the old picture stays until the new one is decoded, as a
    /// browser keeps the old `src`.
    func assetChanged(_ name: String) {
        session?.rasters.invalidate(name)
        for v in views.values where v.kind == "image" && v.imageSource == name { v.loadImage(name) }
    }

    /// A restart: every view goes.
    func reset() {
        // Every hatched node ends first, its view and platform object there.
        elements.reset()
        resetFlights()
        canvasKey = nil
        session?.transformInputHold?.cancel()
        reorder?.abandon()
        reorderGroup?.abandon()
        session?.rasters.reset()
        collections.reset()
        autofocusProcessed.removeAll()
        segments.reset()
        groupedLists?.reset()
        controls.reset()
        fieldSelections.reset()
        edited = nil
        menus.reset()
        swipeActions.reset()
        pool.reset()
        leaves.reset()
        flats.reset()
        modals.reset()
        navigation.reset()
        session?.canvases.reset()
        for id in Array(leaving.keys) { _ = endExit(id) }
        views.values.forEach { $0.forget() }
        root.subviews.forEach { $0.removeFromSuperview() }
        chrome = ChromeIndex()
        views.removeAll()
        inlineOwners.removeAll()
        scrollers.removeAll(); pendingScrolls.removeAll(); materialNodes.removeAll(); contextNodes.removeAll()
        hoveredInline = nil
        scrollPump.reset()
        textViews.removeAll()
        listViews.removeAll()
        heightBindings.removeAll()
        transformBindings.removeAll()
        transformGeometry.reset()
        videoVisibility?.reset()
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
    /// Host intrinsic sizes, several at once under one layout.
    var onIntrinsic: (([(UInt32, CGSize?)]) -> Void)?
    /// A capability an action called (LLP 1005 §3), after its commit.
    var onCommand: ((String, [Any], UInt32?) -> Void)?
    /// A `key` handler called `preventDefault()` (`keyDown(at:_:)`, KeyEvents.swift).
    var defaultPrevented = false
    /// A `key` handler called `stopPropagation()` (`keyDown(at:_:)`, KeyEvents.swift).
    var propagationStopped = false

    /// One native focus intent, bound to the actual editor across controller
    /// transitions. Replacing a node with the same HTML id cannot inherit it.
    private var pendingFocus: (node: NodeView, args: [Any], selectText: Bool, reason: String)?
    private var deliveringFocus = false
    var pendingFocusObservation: [String: String]? {
        guard let pending = pendingFocus, let name = pending.args.first as? String else { return nil }
        return ["target": name, "reason": pending.reason]
    }

    func cancelPendingFocus() { pendingFocus = nil }
    var pendingFocusNode: NodeView? { pendingFocus?.node }

    func flushPendingFocus() {
        guard let pending = pendingFocus, !applying, !navigation.defersFocus, !modals.defersFocus else { return }
        guard views[pending.node.id] === pending.node,
              pending.node.props["id"] == pending.args.first as? String,
              !navigation.isInactiveRoute(containing: pending.node) else {
            pendingFocus = nil
            session?.log("pending focus cancelled: its target or route was replaced")
            return
        }
        guard pending.node.window != nil, pending.node.bounds.width > 0, pending.node.bounds.height > 0 else { return }
        pendingFocus = nil
        deliveringFocus = true
        focusElement(pending.args, selectText: pending.selectText)
        deliveringFocus = false
    }

    /// Sequential focus for a hardware keyboard, the order macOS's key-view
    /// loop has (`PresenterMac.syncKeyViewLoop`): inputs, pressables and nodes
    /// with focus, blur or key handlers, never plain text; tree order, with a
    /// positive `tabIndex` first; nothing hidden, inert, disabled or unmounted.
    /// UIKit hosts routes in its own controllers, so the order is the window's
    /// view order. From no focused node, Tab takes the first.
    func moveFocus(backward: Bool) {
        guard let window = root.window else { return }
        var listed: [NodeView] = []
        func walk(_ view: UIView) {
            if view.isHidden || (view as? NodeView)?.props["inert"] == "true" { return }
            if let node = view as? NodeView, views[node.id] === node, Self.tabbable(node) { listed.append(node) }
            view.subviews.forEach(walk)
        }
        walk(window)
        let order = listed.enumerated().sorted { a, b in
            let ia = a.element.tabOrder, ib = b.element.tabOrder
            let pa = ia > 0 ? ia : Int.max, pb = ib > 0 ? ib : Int.max
            return pa != pb ? pa < pb : a.offset < b.offset
        }.map(\.element)
        guard !order.isEmpty else { return }
        let focused = order.firstIndex { $0.isFirstResponder || $0.field?.isFirstResponder == true || $0.textArea?.isFirstResponder == true }
        let next = focused.map { (backward ? $0 - 1 + order.count : $0 + 1) % order.count } ?? (backward ? order.count - 1 : 0)
        let target = order[next]
        let responder: UIView = target.textArea ?? target.field ?? target
        if responder.becomeFirstResponder(), responder === target { target.showFocusRing(true) }
    }
    /// A Tab stop (LLP 1088 D7.3): an explicit `tabindex` ≥ 0 or what is one
    /// by kind; an explicit negative never, though a tap still focuses it.
    private static func tabbable(_ v: NodeView) -> Bool {
        if v.formDisabled || v.cssVisibilityHidden || v.bounds.width == 0 || v.bounds.height == 0 { return false }
        if let index = v.explicitTabIndex, index < 0 { return false }
        // A radio group is one stop (x2apps survey #2).
        return v.field != nil || v.textArea != nil || v.handlers.contains("press") || v.canBecomeFirstResponder && (!v.isRadio || v.radioTabStop)
    }

    /// The action's focus(html-id), delivered only after the batch is mounted.
    /// A focus that cannot be delivered is refused with its reason in the
    /// runner's journal (LLP 1035.001 D3/D6, `NavigationRules.focusRefusal`),
    /// never silently.
    func focusElement(_ args: [Any], selectText: Bool = false) {
        if !deliveringFocus { pendingFocus = nil }
        guard args.count == 1, let name = args.first as? String else {
            session?.log("focus refused: one string argument expected")
            return
        }
        guard let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else {
            session?.log("focus \"\(name)\" refused: no live node with that id")
            return
        }
        focus(target, args, selectText: selectText)
    }

    /// A hatch's `focus()` on the node it resolved (LLP 1075.003 §3.4).
    func focusNode(_ target: NodeView) { focus(target, [target.props["id"] ?? ""], selectText: false) }

    private func focus(_ target: NodeView, _ args: [Any], selectText: Bool) {
        let name = target.props["id"] ?? "#\(target.id)"
        // UIKit may not announce its next transition until after the outgoing
        // sheet starts dismissing. The selected route's editor already exists,
        // but cannot receive focus until its controller mounts it in a window.
        if !deliveringFocus, navigation.defersFocus || modals.defersFocus || target.window == nil {
            pendingFocus = (target, args, selectText, navigation.defersFocus
                ? "navigation is still transitioning" : modals.defersFocus
                ? "the sheet is still presenting" : "the target is not mounted")
            flushPendingFocus()
            return
        }
        var hidden = false, inert = false
        var ancestor: UIView? = target
        while let view = ancestor {
            if view.isHidden { hidden = true }
            if (view as? NodeView)?.props["inert"] == "true" { inert = true }
            ancestor = view.superview
        }
        // The mounted window is a prerequisite, never another session's.
        if let reason = NavigationRules.focusRefusal(mounted: target.window != nil, disabled: target.formDisabled,
                                                     zeroSize: target.bounds.width == 0 || target.bounds.height == 0,
                                                     hiddenAncestor: hidden, inertAncestor: inert) {
            session?.log("focus \"\(name)\" refused: \(reason)")
            return
        }
        let responder: UIView = target.textArea ?? target.field ?? target
        if selectText, target.textArea == nil, target.field == nil {
            session?.log("selectText \"\(name)\" refused: not a text editor")
            return
        }
        if responder.canBecomeFirstResponder { _ = responder.becomeFirstResponder() }
        if selectText, responder.isFirstResponder { fieldSelections.selectAll(target) }
    }

    /// The action's blur(): drop focus and the keyboard, and any focus still
    /// waiting to be delivered; blur(html-id) only when that node holds it.
    func blurElement(_ args: [Any]) {
        pendingFocus = nil
        if let name = args.first as? String {
            guard let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else { return }
            let responder: UIView = target.textArea ?? target.field ?? target
            if responder.isFirstResponder { _ = responder.resignFirstResponder() }
            return
        }
        // `endEditing` resigns text editors only; a focused control resigns itself.
        if !viewport.endEditing(true), let held = views.values.first(where: { $0.isFirstResponder }) { _ = held.resignFirstResponder() }
    }

    /// The events beyond press and change (LLP 1005 §3).
    var onHover: ((UInt32, Bool) -> Void)?
    var onFocus: ((UInt32) -> Void)?
    var onBlur: ((UInt32) -> Void)?
    /// A `key` or `keyup` (`KeyPress.up`) at a node (KeyEvents.swift).
    var onKey: ((UInt32, KeyPress) -> Void)?
    var onClipboard: ((UInt32, UInt32, String) -> Void)?
    var onContextmenu: ((UInt32) -> Void)?
    var onDblclick: ((UInt32) -> Void)?
    /// A touch went down on a node (`true`) or came up (LLP 1005 §3).
    var onPointer: ((UInt32, PointerKind, PointerSample) -> Void)?
    var onSwiperight: ((UInt32) -> Void)?
    var onRefresh: ((UInt32) -> Void)?
    var onPan: ((UInt32, Double, Double) -> Void)?
    /// A pan that began ended (LLP 1057 §10.6); the tracker below measures
    /// where the platform gives no velocity (LLP 1057.001 §3).
    var onPanRelease: ((UInt32, Double, Double) -> Void)?
    var onPanSample: ((Bool, Double, Double, Double) -> Void)?
    var panVelocity: ((Double) -> (Double, Double))?
    /// A scroller with a `scroll` handler moved: left, top, then its
    /// `scrollWidth`, `scrollHeight`, `clientWidth`, `clientHeight`.
    var onScroll: ((UInt32, [Double]) -> Void)?
    /// A scroller (nil: the page) moved, handler or not: `frame()` reads
    /// boxes where the viewer sees them (LLP 1051.000 D1).
    var onScrolled: ((UInt32?, Double, Double) -> Void)?
    var interacting: UInt32 = 0
    var listViews: [UInt32: NodeView] = [:]
    var textViews: [UInt32: NodeView] = [:]
    lazy var scrollPump = ScrollPump(self)
    let textRasters = TextRasterizer()
    /// Clip geometry memoized for one text refresh (`TextClips`), else nil.
    var textClips: TextClips?
    func listVelocity(_ id: UInt32) -> Double { scrollPump.velocity(id) }
    func settlePump() {
        leaves.settle()
        scrollPump.settle()
        textRasters.settleVisible(textViews.values.filter { textIsVisible($0) })
    }
    func requestTextPublication() { scrollPump.requestText() }
    var onSubmit: ((UInt32) -> Void)?
    var onLoad: ((UInt32) -> Void)?
    var onMessage: ((UInt32, String) -> Void)?
    /// The node the pointer is over, of those with a hover handler: it hears
    /// the leave when the pointer moves onto another (the agent's `hover`).
    weak var hovered: NodeView?
    var hoveredInline: UInt32?

    /// The modifiers held for the press being sent (its `MouseEvent`'s; gallery F20).
    private(set) var pressHeld = ""
    package func press(_ id: UInt32, held: String = "") {
        pressHeld = held; defer { pressHeld = "" }
        if let node = views[id], let url = node.defaultLink, node.activateLink(url) { return }
        onPress?(id)
    }
    func change(_ id: UInt32, _ value: String) { onChange?(id, value) }
    /// A text field typed into since it took the focus: its `change` fires
    /// when the editing ends or Enter commits it, HTML's `change` (LLP
    /// 1069.001 D4); every keystroke is its `input`.
    var edited: UInt32?
    var onInput: ((UInt32, String) -> Void)?
    /// A checkbox's new state, reported as HTML's `input` then `change`.
    var onChecked: ((UInt32, Bool) -> Void)?
    func typed(_ id: UInt32, _ value: String, input: Bool) {
        edited = id
        if input { onInput?(id, value) }
    }
    func commitEdit(_ id: UInt32, _ value: String, change: Bool) {
        guard edited == id else { return }
        edited = nil
        if change { onChange?(id, value) }
    }
    package func checked(_ id: UInt32, _ on: Bool) { onChecked?(id, on) }
    /// A select's, range's or date's new value (LLP 1069.001 D4): HTML's
    /// `input` as it moves, `change` as it is committed.
    var onControlValue: ((UInt32, String, Bool, Bool) -> Void)?
    func controlValue(_ id: UInt32, _ value: String, input: Bool, change: Bool) { onControlValue?(id, value, input, change) }
    /// A select's options and the one it shows, read from the kernel.
    var selectOptions: ((UInt32) -> SelectMenu)?
    var buttonFace: ((UInt32) -> ButtonFace)?

    /// An event a view reports: sent only while the presenter still has the
    /// view (the platform fires editing-ended as a destroyed field leaves the
    /// window; the browser fires no blur on removal, so neither does this
    /// host), and never while a batch is being applied — it waits for the
    /// batch to finish, then goes if its view survived it.
    private(set) var applying = false
    private var waiting: [(UInt32?, () -> Void)] = []
    /// Paragraphs whose presented colour a batch changed, painted as it ends.
    var presentedText: Set<UInt32> = []
    private func send(_ id: UInt32, _ f: @escaping () -> Void) {
        guard textHost(id) != nil else { return }
        if applying { waiting.append((id, f)) } else { f() }
    }
    func hover(_ view: NodeView, _ over: Bool) {
        if over { hoverInline(nil) }
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
    func key(_ id: UInt32, _ press: KeyPress) { send(id) { [self] in onKey?(id, press) } }
    func clipboard(_ id: UInt32, _ kind: UInt32, _ text: String) { send(id) { [self] in onClipboard?(id, kind, text) } }
    func contextmenu(_ id: UInt32) { send(id) { [self] in onContextmenu?(id) } }
    /// A `contextmenu` with its point (studio diary R22): kind 10 and its line.
    func contextmenu(_ id: UInt32, line: String) { send(id) { [self] in onClipboard?(id, 10, line) } }
    func dblclick(_ id: UInt32) { send(id) { [self] in onDblclick?(id) } }
    func pointer(_ id: UInt32, _ kind: PointerKind, _ sample: PointerSample) { send(id) { [self] in onPointer?(id, kind, sample) } }
    func swiperight(_ id: UInt32) { send(id) { [self] in onSwiperight?(id) } }
    func refresh(_ id: UInt32) { send(id) { [self] in onRefresh?(id) } }
    func pan(_ id: UInt32, _ dx: Double, _ dy: Double) { send(id) { [self] in onPan?(id, dx, dy) } }
    /// Once per pan that began, after its last delta; only to a node that hears it.
    func panRelease(_ id: UInt32, _ vx: Double, _ vy: Double) {
        guard views[id]?.handlers.contains("panrelease") == true else { return }
        send(id) { [self] in onPanRelease?(id, vx, vy) }
    }
    func scroll(_ id: UInt32, _ metrics: [Double]) { send(id) { [self] in onScroll?(id, metrics) } }
    func submit(_ id: UInt32) { send(id) { [self] in onSubmit?(id) } }
    func load(_ id: UInt32) { send(id) { [self] in onLoad?(id) } }
    func message(_ id: UInt32, _ value: String) {
        guard let view = views[id], view.handlers.contains("message") else { return }
        send(id) { [weak self, weak view] in
            guard let self, let view, views[id] === view, view.handlers.contains("message") else { return }
            onMessage?(id, value)
        }
    }
    func intrinsic(_ id: UInt32, _ size: CGSize?) { onIntrinsic?([(id, size)]) }
    /// What native containers cover of boxes (LLP 1075.003 §3.5).
    var onCovers: (([(UInt32, HostCover?)]) -> Void)?
    /// Work for after the batch being applied, or now: a hatch's act on an
    /// authored element never lands inside a batch (LLP 1075.003 §3.4).
    func afterBatch(_ work: @escaping () -> Void) { if applying { waiting.append((nil, work)) } else { work() } }
    /// Symbols and projected controls report after the batch that creates
    /// them: all sizes from a turn reach the runner under one layout.
    private struct QueuedIntrinsic { weak var view: NodeView?; let generation: Int; let size: CGSize? }
    private var intrinsicSizes: [QueuedIntrinsic] = []
    func queueIntrinsicSize(_ view: NodeView, generation: Int, _ size: CGSize?) {
        if intrinsicSizes.isEmpty {
            DispatchQueue.main.async { [weak self] in self?.flushIntrinsicSizes() }
        }
        intrinsicSizes.append(QueuedIntrinsic(view: view, generation: generation, size: size))
    }
    private func flushIntrinsicSizes() {
        let queued = intrinsicSizes
        intrinsicSizes = []
        var latest: [UInt32: CGSize?] = [:], order: [UInt32] = []
        for entry in queued {
            guard let view = entry.view, view.loadGeneration == entry.generation,
                  views[view.id] === view else { continue }
            if latest.updateValue(entry.size, forKey: view.id) == nil { order.append(view.id) }
        }
        if !order.isEmpty { onIntrinsic?(order.map { ($0, latest[$0]!) }) }
    }

    /// A batch that only carries collection snapshots (the runner's revision,
    /// a row owed, heights it measured) changes no view. While a list fills,
    /// that is one batch in two or three; the collection takes the snapshot
    /// and makes its correction without the presenter's whole finalization
    /// pass, unless work waits on a batch (a focus, a callback, a scroll).
    private func applySnapshots(_ batch: Batch) -> Bool {
        guard !applying, batch.error == nil, !batch.controls, !batch.ops.isEmpty, batch.ops.allSatisfy({ $0.op == .collections }),
              waiting.isEmpty, pendingScrolls.isEmpty, pendingFocus == nil else { return false }
        collections.beginBatch(batch)
        collections.endBatch()
        return true
    }

    func apply(_ batch: Batch) {
        defer { applyLanguage(batch) }
        if applySnapshots(batch) { return }
        PaintOrder.begin()
        let post = Self.signposts.beginInterval("apply")
        defer { Self.signposts.endInterval("apply", post) }
        collections.beginBatch(batch)
        if !applying { flats.begin(batch) }
        pool.begin(batch)
        swipeActions.prepare()
        groupedLists?.prepare()
        prepareContexts(batch)
        modals.prepare(batch)
        navigation.prepare(batch)
        anchorChanges.reset()
        for id in scrollers where !collections.owns(id) { views[id]?.captureScrollPosition() }
        if let e = batch.error { FileHandle.standardError.write(Data("exact: \(e)\n".utf8)) }
        if let text = session?.text {
            // SVG text shapes with the session's fonts (LLP 1055.000 D11).
            svg.fonts = { [weak text] size, weight, family, italic in
                (text?.font(size: size, weight: weight, family: family, italic: italic)).map { $0 as CTFont } ?? SvgScene.systemFonts(size, weight, family, italic)
            }
        }
        svg.seek(clock: session?.clock)
        let outermost = !applying
        applying = true
        // Hatched nodes this batch destroys end first, so a reusable hatch has
        // undone its additions before the pool looks at their rows; inside
        // the batch, so what a hatch clicks waits for it (`afterBatch`).
        elements.begin(batch)
        var moved = false // create, frame or content ops: rows may have come or moved (`HeavyLeaves.batchApplied`)
        defer {
            collections.endBatch()
            pool.end()
            PaintOrder.end()
            if outermost {
                applying = false
                reaimFixedGradients()
                paintPresentedText()
                if !boxFilters.isEmpty { boxFilters.render() }
                videoVisibility?.changed()
                let q = waiting
                waiting = []
                for (id, f) in q where id.map({ textHost($0) != nil }) ?? true { f() }
                scrollPump.batchApplied()
                leaves.batchApplied(moved: moved)
                #if os(iOS)
                resolveStatusBar()
                #endif
                if moved { session?.natives.refreshWorldGeometry() }
                flushPendingFocus()
            }
        }
        var beganGeometry = false
        var touchedIDs: [UInt32] = []
        let rowsOnly = onlyListRows(batch)
        for op in batch.ops {
            let kind = op.op
            switch kind {
            case .create, .props, .style, .children, .paragraph, .flow, .frame: touchedIDs.append(op.id)
            default: break
            }
            if kind == .create || kind == .frame || kind == .content { moved = true }
            if !beganGeometry && (kind == .frame || kind == .content) {
                beganGeometry = true
                // Mount the native owner under the root's available box before
                // content geometry lets UIKit settle its scroll relationship.
                if let first = root.subviews.first as? NodeView {
                    for frame in batch.ops where frame.op == .frame && frame.id == first.id {
                        applyGeometry(frame)
                    }
                }
                navigation.installInitialOwner(batch)
            }
            let id = op.id
            if kind == .children { touched(id, children: true) } else if kind != .roots && kind != .create { touched(id, textChanged: kind == .props || kind == .style || kind == .destroy) }
            switch kind {
            case .transformDrag:
                if let binding = TransformDragBinding(op.payload) {
                    for n in [binding.id, binding.target, binding.clip].compactMap({ $0 }) where flats.isFlat(n) { flats.promote(n) }
                    if binding.target == nil {
                        if transformBindings[binding.id]?.handleKey == binding.handleKey
                            && transformBindings[binding.id]?.runtime == binding.runtime {
                            transformBindings.removeValue(forKey: binding.id)
                            transformGeometry.retire(binding.id)
                        }
                    } else { transformBindings[binding.id] = binding }
                    views[binding.id]?.updateTransformDragGesture()
                }
            case .retireMotion:
                if let rawRuntime = op.payload["runtime"] as? String, let runtime = UInt64(rawRuntime),
                   let rawToken = op.payload["token"] as? String, let token = UInt64(rawToken) {
                    session?.transformInputHold?.retire(runtime: runtime, token: token)
                }
            case .heightDrag:
                if let binding = HeightDragBinding(op.payload) {
                    for n in [binding.id, binding.target].compactMap({ $0 }) where flats.isFlat(n) { flats.promote(n) }
                    if binding.target == nil {
                        if heightBindings[binding.id]?.handleKey == binding.handleKey {
                            heightBindings.removeValue(forKey: binding.id)
                        }
                    } else { heightBindings[binding.id] = binding }
                    views[binding.id]?.updateHeightDragGesture()
                }
            case .create:
                // An inert leaf box is a layer in its parent's (LLP 1068 §6.1).
                if flats.create(op) { continue }
                let reused = pool.take(id)
                let v = reused ?? NodeView(id: id, kind: op.kind, presenter: self)
                v.handlers = op.handlers
                // Before the style makes it a scroll container: a swipe row's waits.
                v.swipeOwner = op.props["swipeContent"] != nil
                v.applyStyle(op.style)
                v.applyProps(set: op.props, clear: [])
                if reused != nil { v.finishReuse() }
                views[id] = v
                elements.created(v)
                if v.kind == "video" { leaves.created(v) }
                if v.kind == "list" { listViews[id] = v }
                if v.isParagraph { textViews[id] = v }
            case .paragraph:
                applyParagraph(id, op.runs)
            case .props:
                if flats.isFlat(id) { if FlatLeaves.onlyData(op) { continue }; flats.promote(id) }
                views[id]?.applyProps(set: op.props, clear: op.clear)
                elements.propsChanged(id)
            case .flow:
                views[id]?.applyFlow(op.payload["shapes"] as? [[String: Any]] ?? [])
            case .style:
                if flats.isFlat(id), flats.style(id, op) { continue }
                guard let v = views[id] ?? leaving[id]?.view else { continue }
                let color = v.style["text_color"], old = v.style
                v.applyStyle(op.style)
                anchorChanges.note(id, from: old, to: v.style)
                flats.styleChanged(id)
                if v.surface != nil { v.applySurface() }
                // Paint motion re-sends a style per frame (LLP 1055.000 D6):
                // a paragraph's pixels carry their colour, so a new one is
                // painted as the batch ends, not on a worker a frame later
                // (LLP 1062 D6); and a view that paints in an appearance of
                // its own says so (D4).
                if v.style["text_color"] != color {
                    if v.isParagraph { presentedText.insert(id) }
                    session?.noteAppearance(v)
                }
            case .children:
                if flats.isFlat(id) { if op.ids.isEmpty { continue }; flats.promote(id) }
                guard let parent = views[id] else { continue }
                placeChildren(parent, op.ids)
            case .surface:
                if let v = views[id] { session?.canvases.surface(view: v, name: op.payload["name"] as? String ?? "", values: op.payload["values"] ?? []) }
            case .canvas2d: if let v = views[id] { canvas2d.apply(id, op.payload, layer: v.layer) }
            case .svg: if let v = views[id] { svg.scene(id, op.payload, layer: v.layer, dark: v.drawsDark, clock: session?.clock, limit: v.style["dynamic_range_limit"]?.string) }
            case .animations:
                if flats.isFlat(id) { flats.promote(id) }
                svg.animations(id, op.payload, layer: views[id]?.layer, clock: session?.clock)
            case .command:
                let name = op.payload["name"] as? String ?? ""
                if name == "preventDefault" { defaultPrevented = true; break }
                if name == "stopPropagation" { propagationStopped = true; break }
                onCommand?(name, op.payload["args"] as? [Any] ?? [], (op.payload["source"] as? NSNumber)?.uint32Value)
            case .exit:
                if flats.isFlat(id) { flats.promote(id) }
                beginExit(id)
            case .flight:
                if let from = (op.payload["from"] as? NSNumber)?.uint32Value, flats.isFlat(from) { flats.promote(from) }
                beginFlight(op)
            case .land:
                if let f = flights[id] { landFlight(f) }
            case .rank:
                if let rank = op.payload["rank"] as? NSNumber { flats.rank(id, rank.int64Value) }
            case .sticky:
                if flats.isFlat(id) { flats.promote(id) }
                stickies.apply(id, op.payload)
            case .fragments:
                if flats.isFlat(id) { flats.promote(id) }
                views[id]?.applyColumns(op.payload)
            case .destroy:
                elements.destroyed(id)
                stickies.forget(id)
                forgetFlight(id)
                if flats.isFlat(id) { flats.destroy(id); continue }
                if endExit(id) { continue }
                // A collection's retired row parks for the next of its shape.
                if let view = views[id], pool.retire(view) { continue }
                // Out of the map before out of the window: the editing-ended
                // notification removal fires finds no view to send for.
                let gone = release(id) { $0.forget() }
                if let gone, !modals.retainsRemovedView(gone) { gone.removeFromSuperview() }
            case .roots:
                root.subviews.forEach { $0.removeFromSuperview() }
                for r in op.ids.compactMap({ views[UInt32($0)] }) { root.addSubview(r) }
            case .frame, .content:
                if flats.isFlat(id) {
                    if kind == .frame { flats.frame(id, CGRect(x: op.x, y: op.y, width: op.w, height: op.h)) }
                    continue
                }
                if kind == .frame { pool.framed(id) }
                if flightFrame(op) { continue }
                if let node = views[id], !modals.deferGeometry(op, for: node) { applyGeometry(op) }
            case .present:
                if flats.isFlat(id) {
                    if op.property == "opacity" { flats.opacity(id, Float(op.x)); continue }
                    flats.promote(id)
                }
                guard let v = views[id] ?? leaving[id]?.view else { continue }
                let x = CGFloat(op.x)
                switch op.property {
                case "translate": v.translatePx = CGPoint(x: x, y: CGFloat(op.y)); v.translatePercent = CGPoint(x: CGFloat(op.w), y: CGFloat(op.h)); v.applyTransform()
                case "layout": v.layoutOffset = CGPoint(x: x, y: CGFloat(op.y)); v.layoutScale = CGPoint(x: CGFloat(op.w), y: CGFloat(op.h)); v.applyTransform(); v.applySurface()
                case "scale": v.scale = x; v.applyTransform()
                case "rotate": v.rotate = x; v.applyTransform()
                case "opacity": v.alpha = x
                case "flight": presentFlight(id, x)
                default: break
                }
            default: break
            }
        }
        flats.flush()
        fitDocument()
        paintCanvas()
        let first = root.subviews.first as? NodeView
        interactiveWidget = first?.props["interactiveWidget"]
        let fit = first?.props["viewportFit"]
        if fit != viewportFit { viewportFit = fit; onViewportFit?() }
        session?.canvases.cancelMovedControls()
        PaintOrder.flush()
        session?.canvases.captureIfNeeded()
        for id in scrollers.union(pendingScrolls).union(materialNodes) {
            guard let node = views[id] else { continue }
            // A modal's live source retains its old geometry until release.
            // Its scroll writes must wait too, especially on newly added rows
            // whose extent is still zero. releaseBackground applies both.
            if !modals.defersGeometry(for: node) {
                if !collections.owns(node.id) { node.restoreScrollPosition() }
                if node.pendingScrollTop != nil || node.pendingScrollLeft != nil { collections.userIntent(node.id) }
                node.applyPendingScroll()
            }
            if let material = node.materialView { node.sendSubviewToBack(node.glassSlot ?? material) }
        }
        pendingScrolls = pendingScrolls.filter { views[$0]?.pendingScrollTop != nil || views[$0]?.pendingScrollLeft != nil }
        if !flights.isEmpty { flightsBatchApplied() }
        // A batch that only builds, moves or drops a list's rows changes no
        // route, header or bar: the projection would come out the same.
        if !rowsOnly || navigation.syncOwed { navigation.sync(batch) }
        #if os(tvOS)
        menuKey.sync()
        focusGuides.sync()
        playPauseKey.sync()
        #endif
        segments.sync()
        controls.sync(contents: batch.controls, touched: touchedIDs)
        menus.sync()
        glassGroups.reconcile()
        let changed = touchedAndAbove(touchedIDs)
        swipeActions.sync(changed: changed)
        groupedLists?.sync(changed: changed)
        positionContexts()
        syncAccessibility(changed: changed)
    }

    /// Everything kept for `id` goes, the view out of the map (not out of
    /// the window): `leaving` forgets it (a destroy) or recycles it (a
    /// parked row, `NodePool`).
    @discardableResult
    func release(_ id: UInt32, _ leaving: (NodeView) -> Void) -> NodeView? {
        session?.canvases.destroy(view: id)
        svg.forget(id)
        canvas2d.forget(id)
        if let view = views[id] { autofocusProcessed.remove(ObjectIdentifier(view)); leaving(view) }
        listViews.removeValue(forKey: id)
        textViews.removeValue(forKey: id)
        scrollPump.forget(id)
        heightBindings.removeValue(forKey: id)
        transformBindings.removeValue(forKey: id)
        transformGeometry.retire(id)
        chrome.forget(id)
        scrollers.remove(id); pendingScrolls.remove(id); materialNodes.remove(id); contextNodes.remove(id)
        return views.removeValue(forKey: id)
    }

    /// `parent`'s children, in order: its views as subviews of its container,
    /// then its flat leaves' layers among them (LLP 1068 §6.1).
    func placeChildren(_ parent: NodeView, _ ids: [UInt32]) {
        // Make any views required by this holder before assembling its list.
        for id in flats.place(parent, ids) { flats.promote(id) }
        let want = ids.compactMap { views[$0] }
        let container = parent.container
        // An open popover under the agent stays in the top layer.
        menus.children(container, want)
        let wanted = Set(want.map(ObjectIdentifier.init))
        var current = container.subviews
        for case let child as NodeView in current where !wanted.contains(ObjectIdentifier(child)) && !pool.isParked(child) && !isLeaving(child) {
            if !modals.retainsRemovedView(child) { child.removeFromSuperview() }
        }
        // In order, below anything else in the container (a scroll
        // view's indicators): inserting a subview at an index moves
        // it when it is already there. One already there stays.
        let contained = NodeView.keepingGhosts(want.filter { !navigation.ownsContainment(of: $0, under: parent) && !menus.lifted($0) && !isFlying($0) }, in: container)
        current = container.subviews
        for (i, child) in contained.enumerated() where !(i < current.count && current[i] === child) {
            let wasHere = child.superview === container
            container.insertSubview(child, at: i)
            if wasHere { current = container.subviews }
            else { current.insert(child, at: i) }
        }
        flats.mounted(parent.id)
        parent.invalidateGroupedSeparators()
        if parent === navigation.container { navigation.placeOwner() }
    }

    /// The views a batch touched and every view above them, as the batch
    /// left the hierarchy: what a pass reading a subtree must revisit.
    private func touchedAndAbove(_ ids: [UInt32]) -> Set<UInt32> {
        var seen = Set<UInt32>()
        for id in ids {
            var view: UIView? = views[id]
            while let current = view {
                if let node = current as? NodeView, !seen.insert(node.id).inserted { break }
                view = current.superview
            }
        }
        return seen
    }

    /// Geometry can be deferred for the source route while a modal owns the
    /// session viewport. Replaying it uses the same path as the original batch.
    func applyGeometry(_ op: BatchOp) {
        // Replayed outside a batch (a sheet's dismissal completing): carried
        // rows go back to their authored parents first, as before a batch,
        // so the new box is theirs; the projections remount on the next turn.
        if !applying {
            groupedLists?.prepare()
            requestProjectionSync()
        }
        let id = op.id
        guard let v = views[id] else { return }
        switch op.op {
        case .frame:
            // A frame is set untransformed (UIKit's `frame` is undefined
            // under a transform); the presentation goes back on after.
            v.transform = .identity
            v.frame = CGRect(x: op.x, y: op.y, width: op.w, height: op.h)
            v.textRasterGeometryChanged()
            v.scroll?.frame = v.bounds
            v.field?.frame = v.contentBox()
            v.layoutTextArea()
            v.metal?.frame = v.bounds
            v.overlay?.frame = v.bounds
            v.web?.frame = v.bounds
            v.fitScroll()
            v.applyTransform()
            menus.framed(v)
        case .content:
            v.content = CGSize(width: op.w, height: op.h)
            v.fitScroll()
        default: break
        }
    }

    /// Context actions belong to the existing editing session. Resigning
    /// it before dispatch can move the panel out from under the same tap.
    func contextRetainsFocus(_ view: UIView) -> Bool {
        var ancestor: UIView? = view
        while let current = ancestor {
            if let node = current as? NodeView, node.props["retainFocus"] == "true" { return true }
            ancestor = current.superview
        }
        return carrying("contextTarget").contains { preview in
            guard preview.props["contextTarget"] != nil, let panel = contextPanel(preview) else { return false }
            return view.isDescendant(of: panel)
        }
    }

    private func contextPanel(_ preview: NodeView) -> NodeView? {
        var parent = preview.superview as? NodeView
        while let node = parent {
            if node.style["position_type"]?.string == "absolute" { return node }
            parent = node.superview as? NodeView
        }
        return nil
    }

    private struct ContextAnchor {
        let target: String
        let source: NodeView
        let box: CGRect
        let viewportWidth: CGFloat
        let scroll: NodeView?
        let scrollBox: CGRect?
    }
    private var contextAnchors: [UInt32: ContextAnchor] = [:]

    /// Capture before commands in this batch can hide the keyboard and lay
    /// out the source again. The preview owns this bounded presentation state.
    private func prepareContexts(_ batch: Batch) {
        for op in batch.ops {
            let kind = op.op
            guard kind == .create || kind == .props,
                  let id = op.nodeID,
                  let target = op.props["contextTarget"],
                  contextAnchors[UInt32(id)]?.target != target,
                  let source = carrying("id").first(where: { $0.props["id"] == target }),
                  source.window != nil else { continue }
            contextAnchors[UInt32(id)] = contextAnchor(target, source)
        }
    }

    /// Locate the source's vertical scroll contents and the owner of its clip.
    /// Projection does not move the composer or write a scroll offset.
    private func contextContent(_ source: NodeView) -> (content: NodeView, scroll: NodeView)? {
        var child: UIView = source
        while let parent = child.superview {
            if let scroll = parent as? ScrollView, scroll.scrollsY,
               let content = child as? NodeView, let owner = scroll.superview as? NodeView {
                return (content, owner)
            }
            child = parent
        }
        return nil
    }

    private func contextAnchor(_ target: String, _ source: NodeView) -> ContextAnchor {
        let scroll = contextContent(source)?.scroll
        return ContextAnchor(target: target, source: source,
            box: source.convert(source.bounds, to: viewport), viewportWidth: viewport.bounds.width,
            scroll: scroll, scrollBox: scroll.map { $0.convert($0.bounds, to: viewport) })
    }

    /// Magnify the preview without reflowing its text, keeping the source's
    /// outside edge and vertical center. Later content keeps its source-relative
    /// position; clamp the complete projection above the keyboard/safe area.
    /// A scroll whose batch skipped the pass (`ExactSession.applyUnlessEmpty`):
    /// what the pass does that the scroll moved. A context preview follows its
    /// source out of the scrolled box, and the fixed gradients its
    /// compensation moved are aimed again after it, as the pass aims them.
    func scrolledWithoutPass() {
        positionContexts()
        reaimFixedGradients()
    }

    private func positionContexts() {
        for id in contextNodes {
            guard let node = views[id] else { continue }
            node.contextTransform = .identity
            node.applyTransform()
        }
        contextAnchors = contextAnchors.filter { id, anchor in
            views[id]?.props["contextTarget"] == anchor.target &&
                views[anchor.source.id] === anchor.source && anchor.source.window != nil
        }
        func project(_ node: NodeView, _ transform: CGAffineTransform) {
            node.contextTransform = transform
            node.applyTransform()
        }
        for preview in carrying("contextTarget") {
            guard let target = preview.props["contextTarget"],
                  let source = carrying("id").first(where: { $0.props["id"] == target }),
                  let panel = contextPanel(preview), let parent = panel.superview,
                  source.window != nil, preview.bounds.width > 0, preview.bounds.height > 0 else { continue }
            let liveSourceBox = source.convert(source.bounds, to: parent)
            if contextAnchors[preview.id] == nil ||
                contextAnchors[preview.id]?.viewportWidth != viewport.bounds.width {
                contextAnchors[preview.id] = contextAnchor(target, source)
            }
            let anchor = contextAnchors[preview.id]!
            let sourceBox = viewport.convert(anchor.box, to: parent)
            let content = preview.convert(preview.bounds, to: parent)
            let port = viewport.convert(viewport.bounds, to: parent)
            // Public UIKit target-preview fixture, iPhone 17 / iOS 26.5.
            let scale = preview.props["contextMagnify"] == "false" ? 1 : min(1.15, 1 + 26 / max(content.width, content.height))
            let extra = content.height * (scale - 1)
            let dx = sourceBox.midX > port.midX
                ? sourceBox.maxX - content.maxX - content.width * (scale - 1) / 2
                : sourceBox.minX - content.minX + content.width * (scale - 1) / 2
            // Top-aligned side slots keep their gap from the enlarged edge.
            // Their zero-height layout boxes do not enlarge the balloon row.
            if let row = preview.superview {
                for sibling in row.subviews.compactMap({ $0 as? NodeView })
                    where sibling !== preview && abs(sibling.frame.minY - preview.frame.minY) < 0.01 {
                    if sibling.frame.maxX <= preview.frame.minX + 0.01 {
                        project(sibling, CGAffineTransform(translationX: dx - content.width * (scale - 1) / 2, y: 0))
                    } else if sibling.frame.minX >= preview.frame.maxX - 0.01 {
                        project(sibling, CGAffineTransform(translationX: dx + content.width * (scale - 1) / 2, y: 0))
                    }
                }
            }
            var child: NodeView? = preview
            while let current = child, current !== panel, let container = current.superview {
                let bottom = current.frame.maxY
                for sibling in container.subviews.compactMap({ $0 as? NodeView })
                    where sibling !== current && sibling.frame.minY >= bottom - 0.01 {
                    project(sibling, CGAffineTransform(translationX: 0, y: extra / 2))
                }
                child = container as? NodeView
            }
            project(preview, CGAffineTransform(translationX: dx, y: extra / 2).scaledBy(x: scale, y: scale))
            // The authored containing region can reserve room for other
            // context content, such as a reaction's participant popover.
            // Static wrappers do not contain the panel. Clamp against its
            // nearest positioned ancestor, expressed in the parent coordinates.
            var containing = parent
            while containing !== root && containing.superview !== root {
                if let node = containing as? NodeView,
                   node.style["position_type"]?.string != nil,
                   node.style["position_type"]?.string != "static" { break }
                guard let ancestor = containing.superview else { break }
                containing = ancestor
            }
            let region = containing.convert(containing.bounds, to: parent)
            let minimum = max(region.minY, port.minY + insets.top + 8)
            let overflow = max(extra / 2, content.maxY + extra - panel.frame.maxY)
            let maximum = min(region.maxY, port.maxY - insets.bottom - 8) - panel.bounds.height - overflow
            let wanted = sourceBox.midY - (content.minY - panel.frame.minY) - content.height * scale / 2
            let top = max(minimum, min(wanted, maximum))
            if panel.frame.origin.y != top { panel.frame.origin.y = top }
            // A tall preview can fill the available region. Keep the trailing
            // control group inside it, even when that overlaps the preview
            // (the native 24-line Messages reference does this).
            var branch: UIView = preview
            while let owner = branch.superview, owner !== panel { branch = owner }
            let trailing = panel.subviews.compactMap { $0 as? NodeView }.filter {
                $0 !== branch && $0.frame.minY >= branch.frame.maxY - 0.01
            }
            let bottom = min(region.maxY, port.maxY - insets.bottom - 8)
            if let last = trailing.map({ $0.convert($0.bounds, to: parent).maxY }).max(),
               let first = trailing.map({ $0.convert($0.bounds, to: parent).minY }).min() {
                let overflow = min(max(0, last - bottom), max(0, first - minimum))
                if overflow > 0 {
                    for node in trailing {
                        project(node, node.contextTransform.concatenating(CGAffineTransform(translationX: 0, y: -overflow)))
                    }
                }
            }
            if let context = contextContent(source), !panel.isDescendant(of: context.content) {
                // A centered, content-sized scroller moves when the keyboard
                // closes. Retain its clip position too, or neighbors disappear.
                var scrollDelta: CGFloat = 0
                if context.scroll === anchor.scroll, let box = anchor.scrollBox,
                   !panel.isDescendant(of: context.scroll) {
                    scrollDelta = viewport.convert(box, to: parent).minY -
                        context.scroll.convert(context.scroll.bounds, to: parent).minY
                    project(context.scroll, CGAffineTransform(translationX: 0, y: scrollDelta))
                }
                let displacement = sourceBox.minY + top - wanted - liveSourceBox.minY
                project(context.content, CGAffineTransform(translationX: 0, y: displacement - scrollDelta))
            }
        }
    }

    /// The page's canvas colour — behind the document and into the safe
    /// areas the layout keeps out of — is the first root's background, as
    /// Safari paints the root element's background under the status bar
    /// and the home indicator; white when the root sets none.
    var onCanvasColor: ((UIColor) -> Void)?
    /// The head's title (LLP 1048.003 D1), for the app adapter that owns the
    /// scene; an embedded view never claims it.
    private(set) var title: String?
    var onTitle: ((String?) -> Void)?
    #if os(iOS)
    /// The status bar's resolved style (LLP 1105), and who is told of a change.
    var statusBar = StatusBarChoice()
    var onStatusBar: ((StatusBarChoice) -> Void)?
    var statusBarNoted = Set<String>()
    #endif
    func headTitle(_ title: String?) {
        guard title != self.title else { return }
        self.title = title
        onTitle?(title)
    }
    struct CanvasKey: Equatable { let root: ObjectIdentifier?; let channels: [Double]? }
    private var canvasKey: CanvasKey?
    func paintCanvas() {
        // The root's colour, resolved and compared only when what it is made
        // of changes: every batch asked UIKit for a colour it then compared.
        let first = root.subviews.first as? NodeView
        let key = CanvasKey(root: first.map(ObjectIdentifier.init), channels: first?.channels("background_color"))
        guard key != canvasKey else { return }
        canvasKey = key
        let color = first?.color("background_color", .white) ?? .white
        if viewport.backgroundColor != color { viewport.backgroundColor = color; onCanvasColor?(color) }
    }

    /// An op touched a node (LLP 1014 D4 a): every canvas it is painted
    /// through captures again at the end of the batch — the canvas above
    /// it, and itself for its own `children` op.
    func touched(_ id: UInt32, children: Bool = false, textChanged: Bool = false) {
        guard let start = views[id] else { return }
        var paragraph: NodeView? = start
        while let node = paragraph, node.kind == "text" {
            if textChanged || children { node.invalidateText() }
            node.setNeedsDisplay()
            paragraph = node.superview as? NodeView
        }
        if children, start.overlay != nil { start.needsCapture = true }
        if let c = start.paragraphOwner.canvasAbove { c.needsCapture = true }
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
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
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
                if let n = s as? NodeView, n.placement != nil, !n.isHidden { n.isHidden = true; hidden.append(n); continue }
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
        paintOrderLayer(of: view, root: view.layer).render(in: ctx)
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

/// The window's first responder, found the way UIKit offers: an action sent
/// to nil reaches it.
enum FirstResponder {
    private static weak var found: UIResponder?
    static var current: UIResponder? {
        found = nil
        UIApplication.shared.sendAction(#selector(UIResponder.exactReportFirstResponder), to: nil, from: nil, for: nil)
        return found
    }
    fileprivate static func report(_ responder: UIResponder) { found = responder }
}

extension UIResponder {
    @objc fileprivate func exactReportFirstResponder() { FirstResponder.report(self) }
}
#endif
