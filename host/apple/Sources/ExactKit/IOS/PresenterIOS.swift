// The UIKit presenter (LLP 1008 §9): a viewport scroll view over a content-sized
// document, one `NodeView` per kernel node, host batches, keyboard insets and
// field reveal, owned by one session (LLP 1031 D1).
#if os(iOS) || os(tvOS)
import UIKit
import CoreText
import os
package final class Presenter {
    var documentLanguage = ""
    var documentDirection = "ltr"
    static let signposts = OSSignposter(subsystem: "com.exact.host", category: "scroll")
    var autofocusProcessed: Set<ObjectIdentifier> = []
    weak var autofocusedThisTurn: NodeView? // taken out of the window this turn, it has not had its autofocus (NodeViewIOS)
    var pressHandoff: PressHandoff? // the press whose work may hand its control's focus on (PressHandoffIOS)
    /// While a batch a person's input caused is presented, when it was marked
    /// (`Batch.input`): a glass that joins a group then materializes.
    var inputStarted: CFTimeInterval?
    /// A booted session's autofocus waits for its first activation.
    var launchAutofocusReleased = false
    private var projectionSyncOwed = false
    /// Nodes whose frame or content box applied since the batch or projection
    /// sync last looked, those applied after a batch among them: their lists
    /// redraw separators by them
    private var geometryApplied: [UInt32] = []
    /// The views `children` ops put under a parent they were not under (`placeChildren`).
    private var placedAnew: [UInt32] = []
    /// Views whose drawn origin a batch moved (`PixelGridIOS.swift`).
    var gridMoved: [NodeView] = []
    /// The views Core Animation's next commit may show (`unshown`).
    var awaitingFrame: [WeakNodeView] = []
    /// What the native projections show changed outside a batch (a subtree's appearance or
    /// size traits, geometry applied after the batch, as a flight's landing is).
    /// Run the projection steps an empty batch used to run, once, on the next
    /// turn, without waiting for a batch an idle app may never commit (LLP
    /// 1079's amendment of 2026-10-04): grouped lists remount (frames, cell
    /// heights, switches), lists redraw separators where geometry applied,
    /// then tab bars and controls.
    package func requestProjectionSync() {
        guard !projectionSyncOwed else { return }
        projectionSyncOwed = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            projectionSyncOwed = false
            guard !applying else { return }
            // Rows stay carried: a trait refresh never pulls a row (and a
            // first responder or a touch in it) out of its cell. Lists mount
            // first, so a segment or control in a carried row is judged where it shows.
            groupedLists?.sync(changed: [])
            rowCells.sync(changed: touchedAndAbove(geometryApplied))
            geometryApplied = []
            segments.sync()
            controls.sync()
        }
    }
    /// The `aria-modal` view VoiceOver was last moved into, and the views
    /// made modal, which a cleared prop drops from the index (`syncModal`).
    var announcedModal: (id: UInt32, incarnation: UInt64)?
    static var postScreenChanged: (Any?) -> Void = { UIAccessibility.post(notification: .screenChanged, argument: $0) }
    let modalViews = NSHashTable<NodeView>.weakObjects()
    weak package var session: ExactSession?
    /// The document: the roots live here, content-sized like a page.
    package let root = PlainView(frame: .zero)
    /// The viewport over it: the window's content, scrolling like a browser's.
    package let viewport: ScrollView = Viewport(frame: .zero)
    /// The document's mount while a native container owns the screen (`hostDocument`).
    package let documentMount = DocumentMount()
    package var views: [UInt32: NodeView] = [:]
    lazy var focusEvents = FocusEvents(self)
    /// Views leaving with their exit, by id (LLP 1063, `PresenceIOS.swift`).
    var leaving: [UInt32: Leaving] = [:]
    /// Shared elements in flight, by the arriver's id (LLP 1013.000, `FlightsIOS.swift`).
    var flights: [UInt32: Flight] = [:]
    private(set) var chrome = ChromeIndex()
    func propsChanged(_ view: NodeView) { chrome.note(view.id, props: view.props); DragLink.installed?.updateReorderGesture(view); view.updateRefresh(); view.relook(); view.controlMayHaveChanged() }
    /// The nodes that were controls when their props last changed (`NodeView.controlMayHaveChanged`)
    var controlNodes: Set<UInt32> = []
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
    package var heightBindings: [UInt32: HeightDragBinding] = [:]
    package var transformBindings: [UInt32: TransformDragBinding] = [:]
    /// The one Arrange contact, until its source settles; a test's calls.
    package var reorder: ReorderLift?
    package var reorderCalls: ReorderCalls?
    /// A grouped session (LLP 1094), until its ghost lands; a test's calls.
    package var reorderGroup: ReorderGroupHold?
    var reorderGroupCalls: ReorderGroupCalls?
    lazy package var transformGeometry = TransformGeometryHost(self)
    lazy var nativeGeometry = NativeGeometry(self)
    /// Nodes showing a `background-attachment: fixed` gradient (LLP 1066
    /// D7): re-aimed at the viewport when anything scrolls or a batch lands.
    let fixedGradients = NSHashTable<NodeView>.weakObjects()
    func reaimFixedGradients() {
        guard fixedGradients.count > 0 else { return }
        for node in fixedGradients.allObjects where node.window != nil { node.reaimFixedGradient() }
    }
    var videoVisibility: VideoVisibilityHost?
    lazy package var collections = CollectionHost(self)
    lazy var stickies = StickyHost(self)
    lazy var pool = NodePool(self)
    /// Heavy leaves held mid-fling (LLP 1068 §5.1).
    lazy var leaves = HeavyLeaves(self)
    lazy var flats = FlatLeaves(self)
    /// The native swipe rows (LLP 1008 §9), a grouped list's included.
    package lazy var swipeActions = SwipeActionsHost(self); lazy var rowCells = RowCells(self)
    lazy var accent = AppAccent(self)
    /// The native menu arm (LLP 1021 D3).
    lazy var menus = MenuHost(presenter: self)
    /// The `input`s the batch being applied makes in a closed box (`MenuHost.waitingFields`).
    var fieldsWaiting = Set<UInt32>()
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
    lazy var owners = OwnerBlocks(self)
    #if os(iOS)
    private var arrangementHost: AnyObject?
    @available(iOS 27.1, *)
    var arrangements: ArrangementHost {
        if let host = arrangementHost as? ArrangementHost { return host }
        let host = ArrangementHost(self)
        arrangementHost = host
        return host
    }
    #endif
    /// What a capability module sees of the viewport and the keyboard's
    /// container (LLP 1047.001 D4): UIKit's types, not the core's.
    package var viewportScroll: UIScrollView { viewport }
    package var keyboardContainer: UIView? { modals.coordinateView ?? session?.view }
    /// SVG scenes and CSS animations (LLP 1055 D4, D7).
    let svg = SvgHost()
    /// Boxes under CSS `filter`, drawn again after each batch (LLP 1055.000 D14).
    let boxFilters = BoxFilters()
    let canvas2d: Canvas2DCanvases = SurfacesLink.installed?.canvas2D() ?? NoCanvas2D()
    /// The input being edited, if any (UIKit exposes no first responder):
    /// what a canvas painted through its surface captures every frame for
    /// (LLP 1014 D4 d), and what the keyboard reveals.
    weak package var editing: NodeView?
    /// The first root's `viewportFit` prop (`"cover"` or nothing), as of the
    /// last batch; `onViewportFit` fires when it changes.
    private(set) var viewportFit: String?
    var onViewportFit: (() -> Void)?
    /// The first root's `interactiveWidget` prop, as written (`keyboardPolicy`
    /// is what applies).
    private(set) var interactiveWidget: String?
    /// Keyboard avoidance defaults to resizing the editing route's layout block,
    /// or the viewport of an ordinary page. Visual resizing instead insets its
    /// scrollport; overlays-content leaves layout to keyboard toolbars.
    var keyboardPolicy: String { interactiveWidget ?? "resizes-content" }
    /// The keyboard guide's top in window coordinates for this session's current layout.
    var keyboardTop: CGFloat?
    /// The safe-area insets the kernel was given (LLP 1008 §9): the
    /// screen's under `viewport-fit=cover`, zero when the viewport is the
    /// safe area itself. Reported to the agent as `env`.
    var insets = UIEdgeInsets.zero
    /// The layout regions the kernel was given (LLP 1123 D10), `nil` for none.
    var regions: [CGFloat]?
    /// The posture and the viewport segments last told (LLP 1078 D5), for `layout.env`.
    var fold = ViewportFold.flat
    /// Whether the device has a fold — UIKit 27.1 reported a division region
    /// or a hinge — in which case `prefer posture`/`segments` is refused:
    /// the device decides (LLP 1078 D7).
    var hasFold = false
    /// Whether `UIHingeInteraction` has reported once (a hinge or none):
    /// before that, a 27.1 device's fold is unknown (LLP 1078 D7).
    var hingeReported = false
    /// The keyboard's overlap with the viewport before avoidance,
    /// derived by `ExactView.fit`.
    package var keyboardInset: CGFloat = 0
    /// The keyboard's own contribution to the viewport's shared insets.
    private let keyboardViewportInset = KeyboardInset()
    /// What scrollers took below their content to show a field above pinned
    /// content, while the keyboard is up (`PinnedRoom`).
    let pinnedRoom = PinnedRoom()
    /// A native prompt outside the session's mounts, associated by its creator.
    package weak var keyboardPrompt: UITextField?
    var keyboardEditor: UIView? {
        guard let responder = FirstResponder.current as? UIView, responder is UIKeyInput,
              mounts(responder) || responder === keyboardPrompt else { return nil }
        return responder
    }
    var hasKeyboardEditor: Bool { keyboardEditor != nil }

    /// A route's content or its projected search field currently owns text input.
    func ownsKeyboard(in container: UIView) -> Bool { edits(in: container) || editsInBar(of: container) }
    /// Whether the keyboard's editor is in `container`'s content.
    func edits(in container: UIView) -> Bool { keyboardEditor?.isDescendant(of: container) == true }
    /// Whether the keyboard's editor is the search field of the bar over `container`'s route.
    func editsInBar(of container: UIView) -> Bool {
        guard let editor = keyboardEditor else { return false }
        return (ViewPresentation.controller(for: container) as? RouteController)?.search?.field?.headerField === editor
    }
    /// The docked keyboard's occupied geometry in its owning container.
    package func keyboardGuideTop(in container: UIView) -> CGFloat? {
        #if os(tvOS)
        return nil
        #else
        guard container.window != nil, hasKeyboardEditor else { return nil }
        if let view = container as? ExactView, !view.hasKeyboardGuide { return nil }
        let guide = container.keyboardLayoutGuide.layoutFrame
        return guide.height > 0 ? guide.minY : nil
        #endif
    }

    init() {
        viewport.addSubview(root)
        svg.boxGrey = { [unowned self] id in views[id]?.boxTrackGrey }
        viewport.delegate = scrollPump
        collections.motion = { [unowned self] id in
            if collections.animating.contains(id) { return nil }
            let velocity = scrollPump.velocity(id)
            return velocity == 0 ? nil : velocity
        }
        collections.requestFill = { [unowned self] in scrollPump.requestFill() }
        collections.glimpse = { [unowned self] id in scrollPump.glimpse(id) }
        collections.glimpsed = { [unowned self] id, seconds, rows in scrollPump.glimpsed(id, seconds: seconds, rows: rows) }
        // Rows a rescue built show this frame: their text paints now, not
        // after the commit, or they show without it (LLP 1050.000 D1).
        collections.rescued = { [unowned self] in paintVisibleText() }
        viewport.keyboardDismissMode = .interactive
        viewport.contentInsetAdjustmentBehavior = .never
        viewport.backgroundColor = .platformBackground(in: root)
    }

    func observeKeyboard() {
        #if !os(tvOS)
        let c = NotificationCenter.default
        c.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
        c.addObserver(self, selector: #selector(keyboardSettled(_:)), name: UIResponder.keyboardDidChangeFrameNotification, object: nil)
        #endif
    }

    #if !os(tvOS)
    /// Use notifications for animation timing; read geometry from the guide at layout.
    @objc func keyboardChanged(_ n: Notification) {
        if let screen = n.object as? UIScreen, screen !== viewport.window?.screen { return }
        HostLog.note(session, {
            let end = (n.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect) ?? .zero
            let shown = end.height > 0 && end.minY < (viewport.window?.screen.bounds.height ?? .infinity)
            return shown ? "keyboard: shown, \(Int(end.width))×\(Int(end.height)) at y \(Int(end.minY))" : "keyboard: hidden"
        }())
        refitKeyboard(duration: n.userInfo?[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double ?? 0,
                      curve: n.userInfo?[UIResponder.keyboardAnimationCurveUserInfoKey] as? UInt ?? 7)
    }
    #endif

    var interactiveKeyboardDrag: Bool {
        let drags = { (sv: UIScrollView) in sv.keyboardDismissMode == .interactive && (sv.isTracking || sv.isDragging) }
        return drags(viewport) || scrollers.contains { views[$0]?.scrollView.map(drags) ?? false }
            || navigation.primaryRoutes.contains { $0.contentViewport.map(drags) ?? false }
    }

    /// Reconcile through UIKit layout in the keyboard's animation, after batches finish.
    /// Deferred work captures timing only and reads current ownership and geometry.
    func refitKeyboard(duration: Double = 0, curve: UInt = 7) {
        guard let view = session?.view, view.window != nil else { return }
        if Owner.shared.isServingMain {
            DispatchQueue.main.async { [weak self] in self?.refitKeyboard(duration: duration, curve: curve) }
            return
        }
        if applying {
            afterBatch { [weak self] in self?.refitKeyboard(duration: duration, curve: curve) }
            return
        }
        #if !os(tvOS)
        view.installKeyboardProbe()
        #endif
        let change = {
            view.setNeedsLayout()
            view.layoutIfNeeded()
        }
        // Inherit an active animation; apply interactive transition geometry without a separate one
        if ExactEnv.agentFreezes || duration <= 0 || UIView.inheritedAnimationDuration > 0 || navigation.hasInteractiveTransition { change(); return }
        UIView.animate(withDuration: duration, delay: 0, options: [UIView.AnimationOptions(rawValue: curve << 16), .beginFromCurrentState, .allowUserInteraction], animations: change)
    }

    func setKeyboardInset(_ overlap: CGFloat) {
        keyboardInset = overlap
        let amount = keyboardPolicy == "resizes-visual" && !documentFixed ? overlap : 0
        keyboardViewportInset.apply(amount, to: viewport)
    }

    /// The viewport's size in points: what the kernel lays out under, the
    /// page's area (`pageMargins`, PageEdgesIOS.swift).
    var viewportSize: CGSize { viewport.pageSize }
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
        resetting = true; defer { resetting = false }
        MainActor.assumeIsolated { HeldState.retire(self) } // before its views leave the map (HeldState.swift)
        WakeLock.hold(self, false) // the new host's first batch says it again
        releaseOrientationLock()
        focusEvents.reset()
        // Every hatched node ends first, its view and platform object there.
        elements.reset()
        resetFlights()
        canvasKey = nil
        session?.transformInputHold?.cancel()
        reorder?.abandon()
        reorderGroup?.abandon()
        session?.rasters.reset()
        collections.reset(); elementScrolls.cancelAll()
        autofocusProcessed.removeAll()
        segments.reset()
        accent.release()
        groupedLists?.reset()
        controls.reset()
        fieldSelections.reset()
        edited = nil
        menus.reset()
        swipeActions.reset(); rowCells.reset()
        pool.reset()
        leaves.reset()
        flats.reset()
        #if os(iOS)
        if #available(iOS 27.1, *) { arrangements.reset() }
        #endif
        owners.reset {
            modals.reset()
            navigation.reset(); navigation.rebooting()
        }
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
        transformGeometry.reset(); nativeGeometry.reset()
        videoVisibility?.reset()
    }

    /// Size the document to its roots and what overflows one that does not clip (`content`), never smaller
    /// than the page's area, at the area's origin inside the viewport's page margins.
    func fitDocument() {
        let m = pageMargins; var size = viewportSize
        for r in root.subviews {
            let v = r as? NodeView, extent = v.flatMap { $0.scrollView == nil && !$0.overflowClips && !$0.isHidden ? $0.content : nil }
            let reach = extent.flatMap { _ in v?.documentOverflow(in: root) } ?? .zero
            size = CGSize(width: max(size.width, r.frame.maxX, reach.maxX), height: max(size.height, r.frame.maxY, reach.maxY))
        }
        let frame = CGRect(origin: CGPoint(x: m.left, y: m.top), size: size)
        if root.frame != frame { root.frame = frame }
        let content = CGSize(width: size.width + m.left + m.right, height: size.height + m.top + m.bottom)
        if viewport.contentSize != content { viewport.contentSize = content }
    }
    var onPress, onClose: ((UInt32) -> Void)?
    var onChange: ((UInt32, String) -> Void)?
    /// Host intrinsic sizes, several at once under one layout.
    var onIntrinsic: (([(UInt32, CGSize?)]) -> Void)?
    /// A capability an action called (LLP 1005 §3), after its commit.
    var onCommand: ((String, [Any], UInt32?) -> Void)?
    /// A `key` handler called `preventDefault()` (`keyDown(at:_:)`, KeyEvents.swift).
    var defaultPrevented = false
    /// The touches whose `pointerdown` an action prevented, a finger each:
    /// the view each went down on takes no focus from it (LLP 1005 §3;
    /// `keepsFocus(from:)`).
    let preventedTouches = NSHashTable<UITouch>.weakObjects()
    /// An action's `focus()` (or the agent's) is being delivered: a prevented
    /// touch's view takes it all the same, as on the web and the Mac.
    var authoredFocus = false
    /// A `key` handler called `stopPropagation()` (`keyDown(at:_:)`, KeyEvents.swift).
    var propagationStopped = false

    /// One focus intent, bound to the editor even without an authored id.
    /// Replacing a node with the same HTML id cannot inherit it.
    private var pendingFocus: (node: NodeView, name: String?, selectText: Bool, reason: String)?
    private var deliveringFocus = false; lazy var elementScrolls = ElementScrolls(self) // held `scrollIntoView` requests (IntoView.swift)
    var personScrolls = 0, fragmentArm: (activation: Int, scrolls: Int)?, fragmentSeen = 0, fragmentHeld: String? // the person's drags of a scroller; the activation a router change asked a fragment's scroll for and their count then, which one since cancels; the last armed; the fragment's request held (Fragment.swift)
    var pendingFocusObservation: [String: String]? {
        guard let pending = pendingFocus else { return nil }
        return ["target": pending.name ?? "#\(pending.node.id)", "reason": pending.reason]
    }

    func cancelPendingFocus() { pendingFocus = nil }
    var pendingFocusNode: NodeView? { pendingFocus?.node }

    func flushPendingFocus() {
        guard let pending = pendingFocus, !applying, !modals.defersFocus else { return }
        let arriving = navigation.isArriving(pending.node)
        guard !navigation.defersFocus || arriving else { return }
        guard views[pending.node.id] === pending.node,
              pending.node.props["id"] == pending.name,
              arriving || !navigation.isInactiveRoute(containing: pending.node) else {
            pendingFocus = nil
            session?.log("pending focus cancelled: its target or route was replaced")
            return
        }
        guard (pending.node.headerField ?? pending.node).window != nil, Self.sized(pending.node) else { return }
        pendingFocus = nil
        deliveringFocus = true
        focus(pending.node, selectText: pending.selectText)
        deliveringFocus = false
    }

    /// Sequential focus for a hardware keyboard, the order macOS's key-view
    /// loop has (`PresenterMac.syncKeyViewLoop`): inputs, pressables and nodes
    /// with focus, blur or key handlers, never plain text; tree order, with a
    /// positive `tabIndex` first; nothing hidden, inert, disabled or unmounted.
    /// UIKit hosts routes in its own controllers, so the order is the window's
    /// view order, kept to the topmost presentation as UIKit's focus system
    /// keeps it in a modal sheet. From no focused node, Tab takes the first,
    /// and the node it reaches is scrolled into view, as a browser's is.
    /// `fields`: the next text field only, never wrapping (the keyboard's
    /// Next); false when there is none.
    @discardableResult func moveFocus(backward: Bool, fields: Bool = false) -> Bool {
        guard let window = root.window else { return false }
        var top = window.rootViewController
        while let next = top?.presentedViewController, !next.isBeingDismissed { top = next }
        let scope: UIView? = top === window.rootViewController ? window : top?.presentationController?.containerView ?? top?.view
        guard let scope else { return false }
        var listed: [NodeView] = []
        func walk(_ view: UIView) {
            if view.isHidden || (view as? NodeView)?.props["inert"] == "true" { return }
            if let node = view as? NodeView, views[node.id] === node, Self.tabbable(node),
               !fields || node.field != nil || node.textArea != nil { listed.append(node) }
            view.subviews.forEach(walk)
        }
        walk(scope)
        let order = listed.enumerated().sorted { a, b in
            let ia = a.element.tabOrder, ib = b.element.tabOrder
            let pa = ia > 0 ? ia : Int.max, pb = ib > 0 ? ib : Int.max
            return pa != pb ? pa < pb : a.offset < b.offset
        }.map(\.element)
        guard !order.isEmpty else { return false }
        let focused = order.firstIndex { $0.isFirstResponder || $0.field?.isFirstResponder == true || $0.textArea?.isFirstResponder == true }
        if fields, focused.map({ backward ? $0 == 0 : $0 == order.count - 1 }) ?? true { return false }
        let next = focused.map { (backward ? $0 - 1 + order.count : $0 + 1) % order.count } ?? (backward ? order.count - 1 : 0)
        let target = order[next]
        let responder: UIView = target.headerField ?? target.textArea ?? target.field ?? target
        chooseFocus(target)
        let became = responder.becomeFirstResponder()
        if became, responder === target { target.showFocusRing(true) }
        if became { reveal(target) }
        return became
    }
    /// A Tab stop (LLP 1088 D7.3): an explicit `tabindex` ≥ 0 or what is one
    /// by kind; an explicit negative never, though a tap still focuses it.
    private static func tabbable(_ v: NodeView) -> Bool {
        if v.formDisabled || v.inert || v.cssVisibilityHidden || v.bounds.width == 0 || v.bounds.height == 0 { return false }
        if let index = v.explicitTabIndex, index < 0 { return false }
        // A radio group is one stop (x2apps survey #2), an authored one too.
        return v.ariaRadioGroup != nil ? v.ariaRadioTabStop : v.field != nil || v.textArea != nil || v.handlers.contains("press") || v.canBecomeFirstResponder && (!v.isRadio || v.radioTabStop)
    }

    lazy var popoverEvents = PopoverEvents(self)
    /// `showModal(id)`, `close(id)` from an action (LLP 1115 D6): a
    /// confirmation by its id, the platform's alert or sheet (MenusIOS).
    func dialogCommand(_ name: String, _ id: String) {
        if name == "showModal" { menus.showModal(id) } else { menus.closeModal(id) }
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
        focus(target, selectText: selectText)
    }

    /// A hatch's `focus()` on the node it resolved (LLP 1075.003 §3.4).
    func focusNode(_ target: NodeView) { focus(target, selectText: false) }

    /// Whether `target` has a box to focus. A header's search field is the
    /// bar's, which UIKit collapses to no height until it is pulled down or
    /// its search is active.
    private static func sized(_ target: NodeView) -> Bool {
        target.headerField != nil || (target.bounds.width > 0 && target.bounds.height > 0)
    }

    /// The node an action's `focus()` last named, delivered yet or not: a field's Return reads it to learn whether its `submit` asked for the keyboard back (LLP 1115.000 D9.2).
    weak var focusAsked: NodeView?
    private func focus(_ target: NodeView, selectText: Bool) {
        authoredFocus = true
        defer { authoredFocus = false }
        focusAsked = target
        let name = target.props["id"] ?? "#\(target.id)"
        let shown: UIView = target.headerField ?? target
        // UIKit may not announce its next transition until after the outgoing
        // sheet starts dismissing. The selected route's editor already exists,
        // but cannot receive focus until its controller mounts it in a window.
        if !deliveringFocus, (navigation.defersFocus && !navigation.isArriving(target)) || modals.defersFocus || shown.window == nil {
            pendingFocus = (target, target.props["id"], selectText, navigation.defersFocus
                ? "navigation is still transitioning" : modals.defersFocus
                ? "the sheet is still presenting" : "the target is not mounted")
            flushPendingFocus()
            return
        }
        let textEditor: UIView? = target.textArea ?? target.field
        let hidden = sequence(first: shown, next: \.superview).contains { view in
            guard view.isHidden else { return false }
            // Let text editors leave the host-hidden section for a cell. Other targets
            // stay refused here, before scrolling to their hidden authored copy.
            if textEditor != nil, let section = view as? NodeView, groupedLists?.hidSection(section) == true { return false }
            return true
        }
        // Require this session's window. Inert comes from the author or a
        // presentation that owns interaction (LLP 1123 D2).
        if let reason = NavigationRules.focusRefusal(mounted: shown.window != nil, disabled: target.formDisabled,
                                                     zeroSize: !Self.sized(target),
                                                     hiddenAncestor: hidden, inertAncestor: target.inert) {
            session?.log("focus \"\(name)\" refused: \(reason)")
            return
        }
        let responder: UIView = target.headerField ?? textEditor ?? target
        if selectText, textEditor == nil {
            session?.log("selectText \"\(name)\" refused: not a text editor")
            return
        }
        chooseFocus(target)
        guard responder.canBecomeFirstResponder else {
            session?.log("focus \"\(name)\" refused: not focusable")
            return
        }
        carryGroupedRow(containing: target)
        // A row the collection could not carry still cannot receive focus
        if sequence(first: shown, next: \.superview).contains(where: { $0.isHidden }) {
            session?.log("focus \"\(name)\" refused: hidden ancestor")
            return
        }
        guard responder.becomeFirstResponder() else {
            session?.log("focus \"\(name)\" refused: UIKit did not accept first responder")
            return
        }
        if selectText, responder.isFirstResponder { fieldSelections.selectAll(target) }
    }

    /// The action's blur(): drop focus and the keyboard, and any focus still
    /// waiting to be delivered; blur(html-id) only when that node holds it.
    func blurElement(_ args: [Any]) {
        pendingFocus = nil
        if let name = args.first as? String {
            guard let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else { return }
            let responder: UIView = target.headerField ?? target.textArea ?? target.field ?? target
            if responder.isFirstResponder { _ = responder.resignFirstResponder() }
            return
        }
        // `endEditing` resigns text editors only; a focused control resigns itself.
        if !endEditing(), let held = views.values.first(where: { $0.isFirstResponder }) { _ = held.resignFirstResponder() }
    }

    /// The events beyond press and change (LLP 1005 §3).
    var onHover: ((UInt32, Bool) -> Void)?
    var onFocus, onBlur: ((UInt32) -> Void)?
    /// A `key` or `keyup` (`KeyPress.up`) at a node (KeyEvents.swift).
    var onKey: ((UInt32, KeyPress) -> Void)?
    var onClipboard: ((UInt32, UInt32, String) -> Void)?
    var onContextmenu: ((UInt32) -> Void)?
    var onDblclick: ((UInt32) -> Void)?
    var onSelectionChange: ((UInt32, String, Int, Int, [CGRect]) -> Void)? // a `text`'s part of the selection (`TextSelectionIOS`)
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
    var onScrollend: ((UInt32) -> Void)?
    /// A scroller (nil: the page) moved, handler or not: `frame()` reads
    /// boxes where the viewer sees them (LLP 1051.000 D1).
    var onScrolled: ((UInt32?, Double, Double) -> Void)?
    var interacting: UInt32 = 0
    var listViews: [UInt32: NodeView] = [:]
    var textViews: [UInt32: NodeView] = [:] { didSet { NodeView.textGeometry &+= 1 } }
    let textIndex = TextIndex()
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
    package func requestTextPublication() { scrollPump.requestText() }
    var onSubmit: ((UInt32) -> Void)?
    var onLoad: ((UInt32) -> Void)?
    var onMessage: ((UInt32, String) -> Void)?
    /// The node the pointer is over, of those with a hover handler: it hears
    /// the leave when the pointer moves onto another (the agent's `hover`).
    weak var hovered: NodeView?
    var hoveredInline: UInt32?
    var inlineHoverTransition: UInt64 = 0
    /// The modifiers held for the press being sent (its `MouseEvent`'s; gallery F20).
    private(set) var pressHeld = ""
    /// Where its touch lifted in the viewport, its `MouseEvent`'s `clientX`/`clientY`
    /// (LLP 1021 D2); none for a key's press, which DOM puts at 0, 0.
    private(set) var pressPoint: CGPoint?
    /// `at`: the touch's point in the window.
    package func press(_ id: UInt32, held: String = "", at: CGPoint? = nil, bubbled: Bool = false) {
        pressHeld = held; pressPoint = at.map(client); defer { pressHeld = ""; pressPoint = nil }
        if let node = views[id], let url = node.defaultLink, node.activateLink(url, bubbled: bubbled) { return }
        let original = views[id]
        dispatchPressHandler(id, from: original)
        if let original, views[id] === original { menus.invoke(original, bubbled: bubbled) }
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
        guard edited == id, !menus.preservesFocus(id) else { return }
        edited = nil
        var value = value
        // A number input's typed value past `min` or `max` commits as the
        // bound (LLP 1116.008 D5), told as a step is: `input`, then `change`.
        if let node = views[id], let bound = node.clampTypedNumber(value) {
            value = bound
            if node.handlers.contains("input") { onInput?(id, bound) }
        }
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
    /// Each painted popover's region not yet told the kernel, the latest, with the view it is for (`tellRegion`).
    var regionsUntold: [UInt32: (view: ObjectIdentifier?, block: HostBlock?)] = [:]
    /// The dev loop's status pill (DevPill.swift), while one shows.
    var devPill: PlatformView?
    /// Painted popovers' measures in their regions this batch (`placePainted`).
    var regionMeasures = PositionArea.MeasureCache()
    /// Paragraphs whose words the batch being applied changed (`paintChangedText`)
    var changedText: Set<UInt32> = []
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
    func key(_ id: UInt32, _ press: KeyPress) { send(id) { [self] in onKey?(id, press) } }
    func clipboard(_ id: UInt32, _ kind: UInt32, _ text: String) { send(id) { [self] in onClipboard?(id, kind, text) } }
    func contextmenu(_ id: UInt32) { send(id) { [self] in onContextmenu?(id) } }
    /// A `contextmenu` with its point (studio diary R22): kind 10 and its line.
    func contextmenu(_ id: UInt32, line: String) { send(id) { [self] in onClipboard?(id, 10, line) } }
    func dblclick(_ id: UInt32) { send(id) { [self] in onDblclick?(id) } }
    func selectionChange(_ id: UInt32, _ text: String, _ start: Int, _ end: Int, rects: [CGRect] = []) { send(id) { [self] in onSelectionChange?(id, text, start, end, rects) } }
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
    func scrollend(_ id: UInt32) { send(id) { [self] in onScrollend?(id) } }
    func submit(_ id: UInt32) { send(id) { [self] in onSubmit?(id) } }
    func load(_ id: UInt32) { send(id) { [self] in onLoad?(id) } }
    package func message(_ id: UInt32, _ value: String) {
        guard let view = views[id], view.handlers.contains("message") else { return }
        send(id) { [weak self, weak view] in
            guard let self, let view, views[id] === view, view.handlers.contains("message") else { return }
            onMessage?(id, value)
        }
    }
    func intrinsic(_ id: UInt32, _ size: CGSize?) { onIntrinsic?([(id, size)]) }
    /// What native containers cover of boxes (LLP 1075.003 §3.5).
    var onCovers: (([(UInt32, HostCover?)]) -> Void)?
    /// The border box a person dragged a box's `resize` grip to; nil keeps an axis (ResizeGrip.swift).
    var onResizeBox: ((UInt32, CGFloat?, CGFloat?) -> Void)?
    /// In `reset`: host state its views would move with no runtime to tell stays as it was (FocusEvents' focus,
    /// OwnersIOS' blocks, ExactView `fit`'s geometry; ResetGate).
    private(set) var resetting = false
    /// Work for after the batch being applied, or now: a hatch's act on an
    /// authored element never lands inside a batch (LLP 1075.003 §3.4).
    package func afterBatch(_ work: @escaping () -> Void) { if applying { waiting.append((nil, work)) } else { work() } }
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
        session?.fieldChrome.presented(batch.layoutProvisional)
        let outerWaiting = fieldsWaiting; fieldsWaiting = menus.waitingFields(batch)
        defer { fieldsWaiting = outerWaiting }
        let outerInput = inputStarted; inputStarted = batch.input
        defer { inputStarted = outerInput }
        defer { applyLanguage(batch); applyWakeLock(batch); applyOrientationLock(batch) }
        if applySnapshots(batch) { return }
        PaintOrder.begin(); let schemes = ColorSchemes.begin(); defer { schemes.end() }
        let post = Self.signposts.beginInterval("apply"), phase = HostPhases.shared.begin(.apply)
        defer { Self.signposts.endInterval("apply", post); HostPhases.shared.end(phase) }
        collections.beginBatch(batch)
        if !applying { flats.begin(batch) }
        pool.begin(batch)
        swipeActions.prepare(batch)
        groupedLists?.prepare(batch.ops)
        prepareContexts(batch)
        modals.prepare(batch)
        navigation.prepare(batch)
        anchorChanges.reset()
        let anchorFocus = scrollers.isEmpty ? nil : focusedNode
        for id in scrollers where !collections.owns(id) { views[id]?.captureScrollPosition(focused: anchorFocus) }
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
                paintChangedText()
                if !boxFilters.isEmpty { boxFilters.render() }
                videoVisibility?.changed()
                let q = waiting
                waiting = []
                for (id, f) in q where id.map({ textHost($0) != nil }) ?? true { f() }
                scrollPump.batchApplied()
                leaves.batchApplied(moved: moved)
                #if os(iOS)
                resolveStatusBar()
                textSelection.batchEnded()
                #endif
                if moved { session?.natives.refreshWorldGeometry() }
                flushPendingFocus(); elementScrolls.flush()
                #if os(iOS)
                session?.view?.syncKeyboardFocus()
                #endif
            }
        }
        var beganGeometry = false, touchedIDs: [UInt32] = [], reshapedIDs: [UInt32] = [], restyledIDs: [UInt32] = []
        // Read once: an animation UIKit has open around the batch is open around each of its ops
        let animating = UIView.inheritedAnimationDuration > 0
        if outermost { placedAnew.removeAll(keepingCapacity: true) } // a flat leaf promoted between batches
        let anewFrom = placedAnew.count
        let rowsOnly = onlyListRows(batch)
        if batch.controls { controls.facesChanged() }
        // An op inside an animation UIKit has open only if its view showed in the last frame (`moveOrPlace`).
        func applyOp(_ op: BatchOp, _ kind: BatchOp.Kind, _ id: UInt32) {
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
                    if let v = views[binding.id] { DragLink.installed?.updateTransformGesture(v) }
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
                    if let v = views[binding.id] { DragLink.installed?.updateHeightGesture(v) }
                }
            case .create:
                // An inert leaf box is a layer in its parent's (LLP 1068 §6.1).
                if flats.create(op) { return }
                let reused = pool.take(id)
                if let reused, !fieldsWaiting.contains(id) { reused.makeWaitingField() }
                let v = reused ?? NodeView(id: id, kind: op.kind, presenter: self)
                unshown(v) // a reused view too: it is another node
                #if os(iOS)
                v.groupedOwner = op.kind == "list" && op.props["listStyle"] != nil
                #endif
                v.handlers = op.handlers
                // Before the style makes it a scroll container: a swipe row's waits.
                v.swipeOwner = op.props["swipeContent"] != nil
                v.applyStyle(v.presented(op.style))
                v.applyProps(set: op.props, clear: [])
                if reused != nil { v.finishReuse() } else { v.embedFrame(self) }
                views[id] = v
                elements.created(v)
                if v.kind == "video" { leaves.created(v) }
                if v.kind == "list" { listViews[id] = v }
                if v.isParagraph { textViews[id] = v }
            case .paragraph:
                applyParagraph(id, op.runs)
            case .props:
                if flats.isFlat(id) { if flats.props(id, op) { return }; flats.promote(id) }
                if let view = views[id] { anchorChanges.noteProps(op, on: view) }
                views[id]?.applyProps(set: op.props, clear: op.clear)
                elements.propsChanged(id)
            case .flow:
                views[id]?.applyFlow(op.payload["shapes"] as? [[String: Any]] ?? [])
            case .style:
                if flats.isFlat(id), flats.style(id, op) { return }
                guard let v = views[id] ?? leaving[id]?.view else { return }
                let color = v.style["text_color"], old = v.style
                v.applyStyle(v.presented(op.style))
                v.refreshPressInheritance()
                anchorChanges.note(id, from: old, to: v.style, popover: v.props["popover"] != nil)
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
                if flats.isFlat(id) { if flats.adopt(id, op.ids) { return }; flats.promote(id) }
                guard let parent = views[id] else { return }
                placeChildren(parent, op.ids)
            case .surface:
                if let v = views[id] { session?.canvases.surface(view: v, name: op.payload["name"] as? String ?? "", values: op.payload["values"] ?? []) }
            case .canvas2d: if let v = views[id] { canvas2d.apply(id, op.payload, layer: v.layer) }
            case .svg: if views[id] != nil { applyGeometry(op) }
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
                views[id]?.holdGone()
                elements.destroyed(id)
                stickies.forget(id)
                forgetFlight(id)
                if flats.isFlat(id) { flats.destroy(id); return }
                if endExit(id) { return }
                // A collection's retired row parks for the next of its shape.
                if let view = views[id], pool.retire(view) { return }
                // Out of the map before out of the window: the editing-ended
                // notification removal fires finds no view to send for.
                let gone = release(id) { $0.forget() }
                if let gone, !modals.retainsRemovedView(gone) { gone.removeFromSuperview() }
            case .roots:
                root.subviews.forEach { $0.removeFromSuperview() }
                accent.attach(op.ids.compactMap { views[UInt32($0)] }) { for r in $0 { root.addSubview(r); regrid(r) } }
            case .frame, .content, .fieldContent:
                if flats.isFlat(id) {
                    if kind == .frame { flats.frame(id, CGRect(x: op.x, y: op.y, width: op.w, height: op.h)) }
                    if kind == .fieldContent { flats.content(id, op.payload) }
                    return
                }
                if kind == .frame { pool.framed(id) }
                if flightFrame(op) { return }
                if views[id] != nil { applyGeometry(op) }
            case .present:
                if flats.isFlat(id) {
                    if op.property == "opacity" { flats.opacity(id, Float(op.x)); return }
                    flats.promote(id)
                }
                guard let v = views[id] ?? leaving[id]?.view else { return }
                anchorChanges.notePresentation(op, on: v)
                let x = CGFloat(op.x)
                switch op.property {
                case "translate": v.translatePx = CGPoint(x: x, y: CGFloat(op.y)); v.translatePercent = CGPoint(x: CGFloat(op.w), y: CGFloat(op.h)); v.applyTransform()
                case "layout": v.layoutOffset = CGPoint(x: x, y: CGFloat(op.y)); v.layoutResize = CGSize(width: CGFloat(op.w), height: CGFloat(op.h)); v.applyTransform(); v.applySurface()
                case "scale": v.scale = x; v.applyTransform()
                case "rotate": v.rotate = x; v.applyTransform()
                case "opacity": v.alpha = x; v.refreshPressOpacity()
                case "flight": presentFlight(id, x)
                default: break
                }
            default: menus.sheets.applied(op); owners.applied(op)
            }
        }
        for op in batch.ops {
            let kind = op.op
            switch kind {
            case .create, .props, .style, .children, .paragraph, .flow: touchedIDs.append(op.id); reshapedIDs.append(op.id); controls.faceChanged(op.id)
            case .frame: touchedIDs.append(op.id); controls.faceChanged(op.id)
            default: break
            }
            if kind == .props || kind == .style { restyledIDs.append(op.id) }
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
            if kind == .children { touched(id, children: true) } else if ![.roots, .create, .fieldContent].contains(kind) {
                let wordsOnly = kind == .props && op.props.keys.allSatisfy { $0 == "text" } && op.clear.allSatisfy { $0 == "text" }
                touched(id, textChanged: [.props, .style, .destroy].contains(kind), boxChanged: kind != .paragraph && !wordsOnly)
            }
            if animating { moveOrPlace(kind == .create ? nil : views[id] ?? leaving[id]?.view) { applyOp(op, kind, id) } } else { applyOp(op, kind, id) }
        }
        schemes.end(); regridMoved() // every override and move made: the traits before anything reads them
        flats.flush()
        // A view the flush promoted and mounted may have moved its children.
        while !gridMoved.isEmpty { regridMoved(); flats.flush() }
        fitDocument(); reachPageEdges()
        paintCanvas()
        let first = root.subviews.first as? NodeView
        let widget = first?.props["interactiveWidget"]
        if widget != interactiveWidget {
            interactiveWidget = widget
            session?.view?.setNeedsLayout()
        }
        let fit = first?.props["viewportFit"]
        if fit != viewportFit { viewportFit = fit; onViewportFit?() }
        session?.canvases.cancelMovedControls()
        PaintOrder.flush()
        session?.canvases.captureIfNeeded()
        let changed = touchedAndAbove(touchedIDs + geometryApplied)
        geometryApplied = []
        swipeActions.sync(changed: changed, reshaped: touchedAndAbove(reshapedIDs)); rowCells.sync(changed: changed)
        groupedLists?.sync(changed: changed); reachPageEdges() // a list mounted now reaches its bands too
        if !flights.isEmpty { flightsBatchApplied() }
        #if os(tvOS)
        menuKey.sync()
        playPauseKey.sync()
        #endif
        segments.sync()
        // A batch that changes a control's contents (a select's options, a
        // button's face) reads every control again. At or above a control,
        // only props and style reach it (a disabled box, a closing one), and
        // a move to another parent: the children and frame ops of a list
        // whose rows a scroll replaced leave the controls of the rows it
        // kept as they were. Every checkbox in a list of tasks was
        // configured again each batch.
        controls.sync(contents: false, changed: batch.controls ? nil : changed, touched: Set(restyledIDs + placedAnew[anewFrom...]))
        #if os(tvOS)
        // A remembered node's replacement owns focus only after its native
        // control exists, so retarget guides in this same batch after sync.
        focusGuides.sync()
        #endif
        menus.sync()
        glassGroups.reconcile()
        positionContexts()
        // A view a `children` op moved within its parent keeps its ancestors.
        let ariaMoved = Set(placedAnew[anewFrom...].filter { id in
            guard let node = views[id] else { return false }
            return node.accessibilityTraits.contains(.notEnabled) != (node.disabled || node.ariaUnavailable)
        })
        let ariaProps = batch.ops.filter { $0.op == .props && ($0.props["accessibilityDisabled"] != nil || $0.clear.contains("accessibilityDisabled")) }.map(\.id)
        let disabledRoots = Set(ariaProps).union(ariaMoved)
        placedAnew.removeSubrange(anewFrom...)
        // What a node's name, state and focus read: not its box (an autofocus waiting for one is indexed).
        let named = touchedAndAbove(batch.ops.compactMap { [.create, .props, .style, .children, .paragraph, .flow].contains($0.op) ? $0.id : nil })
        // Finish controls and frames before UIKit synchronously lays out the destination.
        afterBatch { [weak self] in
            guard let self else { return }
            navigation.batchApplied(batch, rowsOnly: rowsOnly)
            #if os(iOS)
            if #available(iOS 27.1, *) { arrangements.sync() }
            #endif
            keyboardToolbars.finishBackendChange()
            for id in scrollers.union(pendingScrolls).union(materialNodes) {
                guard let node = views[id] else { continue }
                // Navigation can resize a native port after its row projection.
                // Settle that layout before restoring or authoring an offset.
                if node.groupedOwner { node.scrollView?.layoutIfNeeded() }
                if !collections.owns(node.id) { node.restoreScrollPosition() }
                if node.pendingScrollTop != nil || node.pendingScrollLeft != nil { collections.userIntent(node.id) }
                node.applyPendingScroll()
                if let material = node.materialView { node.sendSubviewToBack(node.glassSlot ?? material) }
            }
            pendingScrolls = pendingScrolls.filter { self.views[$0]?.pendingScrollTop != nil || self.views[$0]?.pendingScrollLeft != nil }
            syncAccessibility(changed: named.union(ariaMoved), disabledRoots: disabledRoots)
        }
        #if os(iOS)
        syncScrollsToTop()
        #endif
    }

    /// Everything kept for `id` goes, the view out of the map (not out of
    /// the window): `leaving` forgets it (a destroy) or recycles it (a
    /// parked row, `NodePool`).
    @discardableResult
    func release(_ id: UInt32, _ leaving: (NodeView) -> Void) -> NodeView? {
        #if os(iOS)
        if #available(iOS 27.1, *) { arrangements.remove(id) }
        #endif
        focusEvents.removed(id)
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
        scrollers.remove(id); pendingScrolls.remove(id); materialNodes.remove(id); contextNodes.remove(id); controlNodes.remove(id)
        return views.removeValue(forKey: id)
    }

    /// `parent`'s children, in order: its views as subviews of its container,
    /// then its flat leaves' layers among them (LLP 1068 §6.1).
    func placeChildren(_ parent: NodeView, _ ids: [UInt32]) {
        #if os(iOS)
        if #available(iOS 27.1, *) { arrangements.record(parent, children: ids) }
        #endif
        // Make any views required by this holder before assembling its list.
        for id in flats.place(parent, ids) { flats.promote(id) }
        let container = parent.container
        // A row a grouped list carries stays in its cell (`GroupedLists.placing`).
        let want = groupedLists.map { $0.placing(container, ids.compactMap { views[$0] }) } ?? ids.compactMap { views[$0] }
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
        for child in want where child.superview !== container { placedAnew.append(child.id) }
        let heldByArrangement: (NodeView) -> Bool = { node in
            #if os(iOS)
            if #available(iOS 27.1, *) { return self.arrangements.owns(node, under: parent) }
            #endif
            return false
        }
        let contained = NodeView.keepingGhosts(want.filter { !heldByArrangement($0) && !navigation.ownsContainment(of: $0, under: parent) && !menus.lifted($0) && !isFlying($0) }, in: container)
        current = container.subviews
        for (i, child) in contained.enumerated() where !(i < current.count && current[i] === child) {
            // One already here is past `i` (those before it are in place):
            // moved as UIKit moves it, without asking for every subview again
            // (a pooled row comes back from past the rows, each a move).
            let wasHere = child.superview === container
            let was = wasHere && i < current.count ? current[i...].firstIndex { $0 === child } : nil
            container.insertSubview(child, at: i)
            if wasHere {
                if let was { current.remove(at: was); current.insert(child, at: i) } else { current = container.subviews }
            } else { current.insert(child, at: i); regrid(child) }
        }
        flats.mounted(parent.id)
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

    /// A frame, content box or scene, in a batch or after one (a flight
    /// landing, FlightsIOS.swift).
    func applyGeometry(_ op: BatchOp) {
        // Outside a batch, carried rows go back to their authored parents
        // first, as before a batch, so the new box is theirs; the projections
        // remount on the next turn. Inside one, a carried row's box is placed
        // from its authored parent (`gridOrigin`) and goes home with it.
        if !applying {
            groupedLists?.prepare()
            requestProjectionSync()
        }
        let id = op.id
        guard let v = views[id] else { return }
        if op.op == .frame || op.op == .fieldContent { geometryApplied.append(id) }
        switch op.op {
        case .frame:
            applyFrame(v, CGRect(x: op.x, y: op.y, width: op.w, height: op.h))
        case .content:
            v.content = CGSize(width: op.w, height: op.h)
            v.fitScroll()
            if v.props["navigationDetent"] != nil { modals.contentChanged(v) }
        case .fieldContent: applyContentGeometry(v, op)
        // A scene is laid out in its content box (its `viewBox` scaled to it)
        case .svg: svg.scene(id, op.payload, layer: v.layer, dark: v.drawsDark, clock: session?.clock, limit: v.style["dynamic_range_limit"]?.string, tint: v.viewTint)
        default: break
        }
    }

    /// Context actions belong to the existing editing session. Resigning
    /// it before dispatch can move the panel out from under the same tap.
    func contextRetainsFocus(_ view: UIView) -> Bool {
        var ancestor: UIView? = view
        while let current = ancestor {
            if let node = current as? NodeView, node.props["retainFocus"] == "true" { return true }
            ancestor = eventParent(of: current)
        }
        return carrying("contextTarget").contains { preview in
            guard preview.props["contextTarget"] != nil, let panel = contextPanel(preview) else { return false }
            return view.isDescendant(of: panel)
        }
    }

    private func contextPanel(_ preview: NodeView) -> NodeView? {
        var parent = preview.superview as? NodeView
        while let node = parent {
            if node.layoutPosition == "absolute" { return node }
            parent = node.superview as? NodeView
        }
        return nil
    }

    private struct ContextAnchor {
        let target: String
        let source: NodeView
        /// Its source's box and scroller's box are in `port`'s coordinates: the
        /// source's port when they were captured (LLP 1123 D2)
        weak var port: UIView?
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
        let scroll = contextContent(source)?.scroll, port = port(of: source)
        return ContextAnchor(target: target, source: source, port: port,
            box: source.convert(source.bounds, to: port), viewportWidth: port.bounds.width,
            scroll: scroll, scrollBox: scroll.map { $0.convert($0.bounds, to: port) })
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
            moveOrPlace(node) { node.applyTransform() }
        }
        contextAnchors = contextAnchors.filter { id, anchor in
            views[id]?.props["contextTarget"] == anchor.target &&
                views[anchor.source.id] === anchor.source && anchor.source.window != nil
        }
        func project(_ node: NodeView, _ transform: CGAffineTransform) {
            node.contextTransform = transform
            moveOrPlace(node) { node.applyTransform() }
        }
        for preview in carrying("contextTarget") {
            guard let target = preview.props["contextTarget"],
                  let source = carrying("id").first(where: { $0.props["id"] == target }),
                  let panel = contextPanel(preview), let parent = panel.superview,
                  source.window != nil, preview.bounds.width > 0, preview.bounds.height > 0 else { continue }
            let liveSourceBox = source.convert(source.bounds, to: parent)
            // The source is read where it was captured; the preview is kept
            // inside its own port, the viewport or its presentation's mount
            let vp = port(of: preview), insets = vp === viewport ? insets : vp.safeAreaInsets
            if contextAnchors[preview.id] == nil ||
                contextAnchors[preview.id]?.viewportWidth != port(of: source).bounds.width {
                contextAnchors[preview.id] = contextAnchor(target, source)
            }
            let anchor = contextAnchors[preview.id]!
            let sourceBox = (anchor.port ?? viewport).convert(anchor.box, to: parent)
            let content = preview.convert(preview.bounds, to: parent)
            let port = vp.convert(vp === viewport ? vp.bounds.inset(by: pageMargins) : vp.bounds, to: parent)
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
                if let node = containing as? NodeView, node.layoutPosition != "static" { break }
                // A route's root, whose container is its controller's view, is as far as a presented one goes
                guard let ancestor = containing.superview, ancestor is NodeView || ancestor === root else { break }
                containing = ancestor
            }
            let region = containing.convert(containing.bounds, to: parent)
            let minimum = max(region.minY, port.minY + insets.top + 8)
            let overflow = max(extra / 2, content.maxY + extra - panel.frame.maxY)
            let maximum = min(region.maxY, port.maxY - insets.bottom - 8) - panel.bounds.height - overflow
            let wanted = sourceBox.midY - (content.minY - panel.frame.minY) - content.height * scale / 2
            let top = max(minimum, min(wanted, maximum))
            if panel.frame.origin.y != top { moveOrPlace(panel) { panel.frame.origin.y = top } }
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
                    scrollDelta = (anchor.port ?? viewport).convert(box, to: parent).minY -
                        context.scroll.convert(context.scroll.bounds, to: parent).minY
                    project(context.scroll, CGAffineTransform(translationX: 0, y: scrollDelta))
                }
                let displacement = sourceBox.minY + top - wanted - liveSourceBox.minY
                project(context.content, CGAffineTransform(translationX: 0, y: displacement - scrollDelta))
            }
        }
    }

    /// The page's canvas colour — behind the document and into the safe
    /// areas the layout keeps out of — is the first root's background, as a
    /// view controller's view carries its own under the status bar and the
    /// home indicator; `systemBackground` when it sets none (LLP 1115 D2).
    var onCanvasColor: ((UIColor) -> Void)?
    /// The head's title (LLP 1048.003 D1), for the app adapter that owns the
    /// scene; an embedded view never claims it.
    private(set) var title: String?
    var onTitle: ((String?) -> Void)?
    #if os(iOS)
    lazy var textSelection = TextSelectionHost(self) // `user-select: text` (LLP 1115.000 D11.6)
    /// The status bar's resolved style (LLP 1105), and who is told of a change.
    var statusBar = StatusBarChoice()
    var onStatusBar: ((StatusBarChoice) -> Void)?
    var statusBarNoted = Set<String>()
    #endif
    /// The session's orientation lock (LLP 1133.029 D5, `OrientationLock.swift`).
    package var orientationLock = OrientationLock()
    func headTitle(_ title: String?) {
        guard title != self.title else { return }
        self.title = title
        onTitle?(title)
    }
    struct CanvasKey: Equatable { let root: ObjectIdentifier?; let channels: [Double]?; var theme = ThemeRoles.generation }
    private var canvasKey: CanvasKey?
    func paintCanvas() {
        // The root's colour, resolved and compared only when what it is made
        // of changes: every batch asked UIKit for a colour it then compared.
        let first = root.subviews.first as? NodeView
        // As written, never greyed: the viewport keeps it once the dimming ends (TintDimmingIOS.swift)
        let key = CanvasKey(root: first.map(ObjectIdentifier.init), channels: first.flatMap { $0.undimmedChannels("background_color", dark: $0.drawsDark) })
        guard key != canvasKey else { return }
        canvasKey = key
        // A root without a background shows the platform's (LLP 1115 D2).
        let color = first?.undimmedCGColor("background_color").map { UIColor(cgColor: $0) } ?? .platformBackground(in: root)
        if viewport.backgroundColor != color { viewport.backgroundColor = color; onCanvasColor?(color) }
    }
    /// An op touched a node (LLP 1014 D4 a): every canvas it is painted
    /// through captures again at the end of the batch — the canvas above
    /// it, and itself for its own `children` op.
    func touched(_ id: UInt32, children: Bool = false, textChanged: Bool = false, boxChanged: Bool = true) {
        // A filtered box draws again when it or anything in it changes, a
        // flat leaf (a layer in its parent's view) included.
        if !boxFilters.isEmpty, let v = views[id] ?? flats.parent(of: id).flatMap({ views[$0] }) { boxFilters.touched(v) }
        guard let start = views[id] else { return }
        var paragraph: NodeView? = start
        while let node = paragraph, node.kind == "text" {
            if textChanged || children { node.invalidateText() }
            if boxChanged || !node.canRasterText { node.setNeedsDisplay() }
            paragraph = node.superview as? NodeView
        }
        if children, start.overlay != nil { start.needsCapture = true }
        if let c = start.paragraphOwner.canvasAbove { c.needsCapture = true }
    }
}
extension CGRect {
    /// The rect inside the given edges (never negative in size).
    func insetBy(left: CGFloat, top: CGFloat, right: CGFloat, bottom: CGFloat) -> CGRect {
        CGRect(x: minX + left, y: minY + top, width: max(0, width - left - right), height: max(0, height - top - bottom))
    }
}
#endif
