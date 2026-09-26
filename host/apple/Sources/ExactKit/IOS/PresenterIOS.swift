// The UIKit presenter (LLP 1008 §9): the viewport scroll view over a
// content-sized document, one `NodeView` per kernel node, the host's
// batches applied, the keyboard's inset and the field it reveals. It
// belongs to one session (LLP 1031 D1) and reaches the session's canvases,
// web views, and menus through it.
#if os(iOS)
import UIKit
import os

final class Presenter {
    static let signposts = OSSignposter(subsystem: "com.exact.host", category: "scroll")
    var scrollCreatedRows = 0
    var scrollOffscreenRows = 0

    var autofocusProcessed: Set<ObjectIdentifier> = []
    /// The session this presenter shows (LLP 1031 D1).
    weak var session: ExactSession?
    /// The document: the roots live here, content-sized like a page.
    let root = PlainView(frame: .zero)
    /// The viewport over it: the window's content, scrolling like a browser's.
    let viewport: ScrollView = Viewport(frame: .zero)
    var views: [UInt32: NodeView] = [:]
    private(set) var chrome = ChromeIndex()
    func propsChanged(_ view: NodeView) { chrome.note(view.id, props: view.props); view.updateReorderGesture(); view.updateRefresh() }
    func carrying(_ key: String) -> [NodeView] { chrome.ids(key).sorted().compactMap { views[$0] } }
    func takeChangedNames() -> Set<String> { chrome.takeChangedNames() }
    var scrollers: Set<UInt32> = []
    var pendingScrolls: Set<UInt32> = []
    var materialNodes: Set<UInt32> = []
    var contextNodes: Set<UInt32> = []
    var inlineOwners: [UInt32: (owner: UInt32, index: Int)] = [:]
    var heightBindings: [UInt32: HeightDragBinding] = [:]
    var transformBindings: [UInt32: TransformDragBinding] = [:]
    /// The one Arrange contact, until its source settles; a test's calls.
    var reorder: ReorderHold?
    var reorderCalls: ReorderCalls?
    lazy var transformGeometry = TransformGeometryHost(self)
    var videoVisibility: VideoVisibilityHost?
    lazy var collections = CollectionHost(self)
    lazy var pool = NodePool(self)
    /// The native menu arm (LLP 1021 D3).
    lazy var swipeActions = SwipeActionsHost(self)
    lazy var menus = MenuHost(presenter: self)
    lazy var segments = SegmentHost(self)
    lazy var navigation = NavigationHost(presenter: self)
    lazy var modals = ModalHost(presenter: self)
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

    var hasKeyboardEditor: Bool {
        views.values.contains { $0.field?.isFirstResponder == true || $0.textArea?.isFirstResponder == true }
    }

    /// A notification describes the keyboard's target, not its current
    /// presence: UIKit announces hiding even during a cancelled sideways pop.
    /// The guide observes the keyboard in its owning container's coordinates.
    func keyboardGuideTop(in container: UIView) -> CGFloat? {
        guard hasKeyboardEditor || keyboardInset > 0 else { return nil }
        let guide = container.keyboardLayoutGuide.layoutFrame
        return guide.height > container.safeAreaInsets.bottom + 1 ? guide.minY : nil
    }

    /// Where the platform's pointer last hovered over the viewport, in its
    /// content space — kept only under the agent (LLP 1035.003 §3): the
    /// driver calibrates its desktop-to-device mapping by hovering the Mac's
    /// pointer at known desktop points and reading where the app saw it,
    /// which no window frame can tell it (a Simulator window carries a
    /// bezel and a scale of its own).
    private(set) var lastPointer: CGPoint?

    init() {
        viewport.addSubview(root)
        viewport.delegate = scrollPump
        collections.motion = { [unowned self] id in
            let velocity = scrollPump.velocity(id)
            return velocity == 0 ? nil : velocity
        }
        collections.requestFill = { [unowned self] in scrollPump.requestFill() }
        // Rows a rescue built show this frame: their text paints now, not
        // after the commit, or they show without it (LLP 1050.000 D1).
        collections.rescued = { [unowned self] in paintVisibleText() }
        viewport.contentInsetAdjustmentBehavior = .never
        viewport.backgroundColor = .white
        if ExactEnv.agentMode {
            let hover = UIHoverGestureRecognizer(target: self, action: #selector(pointerMoved(_:)))
            hover.delaysTouchesBegan = false
            hover.delaysTouchesEnded = false
            hover.cancelsTouchesInView = false
            viewport.addGestureRecognizer(hover)
        }
    }

    @objc func pointerMoved(_ gesture: UIHoverGestureRecognizer) {
        lastPointer = gesture.location(in: viewport)
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
        canvasKey = nil
        session?.transformInputHold?.cancel()
        reorder?.abandon()
        session?.rasters.reset()
        collections.reset()
        autofocusProcessed.removeAll()
        segments.reset()
        menus.reset()
        swipeActions.reset()
        pool.reset()
        modals.reset()
        navigation.reset()
        session?.canvases.reset()
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
    /// Images' intrinsic sizes, several at once under one layout.
    var onIntrinsic: (([(UInt32, CGSize?)]) -> Void)?
    /// A capability an action called (LLP 1005 §3), after its commit.
    var onCommand: ((String, [Any]) -> Void)?

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
            let ia = Int(a.element.props["tabIndex"] ?? "0") ?? 0, ib = Int(b.element.props["tabIndex"] ?? "0") ?? 0
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
    private static func tabbable(_ v: NodeView) -> Bool {
        if v.disabled || v.bounds.width == 0 || v.bounds.height == 0 { return false }
        let index = Int(v.props["tabIndex"] ?? "0") ?? 0
        if index < 0 { return false }
        return v.field != nil || v.textArea != nil || v.handlers.contains("press") || v.canBecomeFirstResponder || index > 0
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
        if let reason = NavigationRules.focusRefusal(mounted: target.window != nil, disabled: target.disabled,
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
        if selectText, responder.isFirstResponder {
            if let editor = target.textArea { editor.selectAll(editor) }
            else if let editor = target.field { editor.selectAll(editor) }
        }
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
    var onKey: ((UInt32, String) -> Void)?
    var onContextmenu: ((UInt32) -> Void)?
    var onDblclick: ((UInt32) -> Void)?
    var onSwiperight: ((UInt32) -> Void)?
    var onRefresh: ((UInt32) -> Void)?
    var onPan: ((UInt32, Double, Double) -> Void)?
    var onScroll: ((UInt32, Double, Double) -> Void)?
    var onList: ((UInt32, Double, Double, Double, Double, UInt32, UInt32, UInt32) -> Bool)?
    var interacting: UInt32 = 0
    var listViews: [UInt32: NodeView] = [:]
    var textViews: [UInt32: NodeView] = [:]
    lazy var scrollPump = ScrollPump(self)
    let textRasters = TextRasterizer()
    /// Clip geometry memoized for one text refresh (`TextClips`), else nil.
    var textClips: TextClips?
    func listVelocity(_ id: UInt32) -> Double { scrollPump.velocity(id) }
    func settlePump() {
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

    func press(_ id: UInt32) { onPress?(id) }
    func change(_ id: UInt32, _ value: String) { onChange?(id, value) }
    /// An event a view reports: sent only while the presenter still has the
    /// view (the platform fires editing-ended as a destroyed field leaves the
    /// window; the browser fires no blur on removal, so neither does this
    /// host), and never while a batch is being applied — it waits for the
    /// batch to finish, then goes if its view survived it.
    private(set) var applying = false
    private var waiting: [(UInt32?, () -> Void)] = []
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
    func key(_ id: UInt32, _ name: String) { send(id) { [self] in onKey?(id, name) } }
    func contextmenu(_ id: UInt32) { send(id) { [self] in onContextmenu?(id) } }
    func dblclick(_ id: UInt32) { send(id) { [self] in onDblclick?(id) } }
    func swiperight(_ id: UInt32) { send(id) { [self] in onSwiperight?(id) } }
    func refresh(_ id: UInt32) { send(id) { [self] in onRefresh?(id) } }
    func pan(_ id: UInt32, _ dx: Double, _ dy: Double) { send(id) { [self] in onPan?(id, dx, dy) } }
    func scroll(_ id: UInt32, _ left: Double, _ top: Double) { send(id) { [self] in onScroll?(id, left, top) } }
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
    /// A symbol's size, known as its view is created but not reportable
    /// while the batch that created it applies. Every symbol a turn creates
    /// goes to the runner together, after that turn: one layout for a row's
    /// symbols, not one each.
    private struct QueuedSymbol { weak var view: NodeView?; let generation: Int; let size: CGSize? }
    private var symbolSizes: [QueuedSymbol] = []
    func queueSymbolSize(_ view: NodeView, generation: Int, _ size: CGSize?) {
        if symbolSizes.isEmpty {
            DispatchQueue.main.async { [weak self] in self?.flushSymbolSizes() }
        }
        symbolSizes.append(QueuedSymbol(view: view, generation: generation, size: size))
    }
    private func flushSymbolSizes() {
        let queued = symbolSizes
        symbolSizes = []
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
        guard !applying, batch.error == nil, !batch.ops.isEmpty, batch.ops.allSatisfy({ $0.op == .collections }),
              waiting.isEmpty, pendingScrolls.isEmpty, pendingFocus == nil else { return false }
        collections.beginBatch(batch)
        collections.endBatch()
        return true
    }

    func apply(_ batch: Batch) {
        if applySnapshots(batch) { return }
        let post = Self.signposts.beginInterval("apply")
        defer { Self.signposts.endInterval("apply", post) }
        collections.beginBatch(batch)
        pool.begin(batch)
        swipeActions.prepare()
        prepareContexts(batch)
        modals.prepare(batch)
        navigation.prepare(batch)
        for id in scrollers where !collections.owns(id) { views[id]?.captureScrollPosition() }
        if let e = batch.error { FileHandle.standardError.write(Data("exact: \(e)\n".utf8)) }
        let outermost = !applying
        applying = true
        defer {
            collections.endBatch()
            pool.end()
            if outermost {
                applying = false
                videoVisibility?.changed()
                let q = waiting
                waiting = []
                for (id, f) in q where id.map({ textHost($0) != nil }) ?? true { f() }
                scrollPump.batchApplied()
                flushPendingFocus()
            }
        }
        var beganGeometry = false
        var touchedIDs: [UInt32] = []
        for op in batch.ops {
            let kind = op.op
            switch kind {
            case .create, .props, .style, .children, .paragraph, .flow, .frame: touchedIDs.append(op.id)
            default: break
            }
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
                    if binding.target == nil {
                        if heightBindings[binding.id]?.handleKey == binding.handleKey {
                            heightBindings.removeValue(forKey: binding.id)
                        }
                    } else { heightBindings[binding.id] = binding }
                    views[binding.id]?.updateHeightDragGesture()
                }
            case .create:
                let reused = pool.take(id)
                let v = reused ?? NodeView(id: id, kind: op.kind, presenter: self)
                v.handlers = op.handlers
                // Before the style makes it a scroll container: a swipe row's waits.
                v.swipeOwner = op.props["swipeContent"] != nil
                v.applyStyle(op.style)
                v.applyProps(set: op.props, clear: [])
                if reused != nil { v.finishReuse() }
                views[id] = v
                if v.kind == "list" { listViews[id] = v }
                if v.isParagraph { textViews[id] = v }
            case .paragraph:
                applyParagraph(id, op.runs)
            case .props:
                views[id]?.applyProps(set: op.props, clear: op.clear)
            case .flow:
                views[id]?.applyFlow(op.payload["shapes"] as? [[String: Any]] ?? [])
            case .style:
                views[id]?.applyStyle(op.style)
            case .children:
                guard let parent = views[id] else { continue }
                let want = op.ids.compactMap { views[UInt32($0)] }
                let container = parent.container
                let wanted = Set(want.map(ObjectIdentifier.init))
                var current = container.subviews
                for case let child as NodeView in current where !wanted.contains(ObjectIdentifier(child)) && !pool.isParked(child) {
                    if !modals.retainsRemovedView(child) { child.removeFromSuperview() }
                }
                // In order, below anything else in the container (a scroll
                // view's indicators): inserting a subview at an index moves
                // it when it is already there. One already there stays.
                let contained = want.filter { !navigation.ownsContainment(of: $0, under: parent) }
                current = container.subviews
                for (i, child) in contained.enumerated() where !(i < current.count && current[i] === child) {
                    container.insertSubview(child, at: i)
                    current = container.subviews
                }
            case .surface:
                if let v = views[id] { session?.canvases.surface(view: v, name: op.payload["name"] as? String ?? "", values: op.payload["values"] ?? []) }
            case .command:
                onCommand?(op.payload["name"] as? String ?? "", op.payload["args"] as? [Any] ?? [])
            case .destroy:
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
                if kind == .frame { pool.framed(id) }
                if let node = views[id], !modals.deferGeometry(op, for: node) { applyGeometry(op) }
            case .present:
                guard let v = views[id] else { continue }
                let x = CGFloat(op.x)
                switch op.property {
                case "translate": v.translate = CGPoint(x: x, y: CGFloat(op.y)); v.applyTransform()
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
        session?.canvases.cancelMovedControls()
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
            if let material = node.materialView { node.sendSubviewToBack(material) }
        }
        pendingScrolls = pendingScrolls.filter { views[$0]?.pendingScrollTop != nil || views[$0]?.pendingScrollLeft != nil }
        navigation.sync(batch)
        segments.sync()
        menus.sync()
        let changed = touchedAndAbove(touchedIDs)
        swipeActions.sync(changed: changed)
        positionContexts()
        syncAccessibility(changed: changed)
    }

    /// Everything kept for `id` goes, the view out of the map (not out of
    /// the window): `leaving` forgets it (a destroy) or recycles it (a
    /// parked row, `NodePool`).
    @discardableResult
    func release(_ id: UInt32, _ leaving: (NodeView) -> Void) -> NodeView? {
        session?.canvases.destroy(view: id)
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
            let minimum = max(parent.bounds.minY, port.minY + insets.top + 8)
            let overflow = max(extra / 2, content.maxY + extra - panel.frame.maxY)
            let maximum = min(parent.bounds.maxY, port.maxY - insets.bottom - 8) - panel.bounds.height - overflow
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
            let bottom = min(parent.bounds.maxY, port.maxY - insets.bottom - 8)
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
#endif
