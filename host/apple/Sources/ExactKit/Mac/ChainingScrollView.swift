#if os(macOS)
import AppKit

/// A `scroll` node's scroll view. AppKit scrolls it (LLP 1115.000 D8.2): its
/// momentum, its wheel step, its rubber band. What is decided here is whose
/// a wheel event is. One this view cannot take in its dominant direction —
/// nothing to scroll, or a gesture that begins at that edge — goes to the
/// next responder, so an inner `scroll` node never traps the page
/// (`overscroll-behavior: auto`; AppKit's default is to swallow it).
package final class ChainingScrollView: NSScrollView {
    /// Empty document and clip hits take this view. The node owns their
    /// mouse contact, even if a handler removes the scroll view mid-press.
    weak var pointerOwner: NodeView? { didSet { followScrollerStyle() } }
    package override func accessibilityChildrenInNavigationOrder() -> [any NSAccessibilityElementProtocol]? { treeNavigationOrder }
    private var mouseHeld = false
    private var removeAfterMouseUp = false
    /// Removed mid-press, the view stays, transparent, until the up, which
    /// AppKit sends only to it. Only in the window, as `NodeView`'s
    /// (`MouseChainMac`): one already out hears no up, and AppKit also asks
    /// here when it frees the parent (`-[NSView _finalize]`). A view that
    /// stayed would reach its own dealloc still a subview, which AppKit
    /// raises (the T3 Code clone's abort on a thread change, X87).
    package override func removeFromSuperview() {
        if mouseHeld, window != nil {
            removeAfterMouseUp = true
            alphaValue = 0
        } else { super.removeFromSuperview() }
    }
    package override func hitTest(_ point: NSPoint) -> NSView? {
        removeAfterMouseUp ? nil : super.hitTest(point)
    }
    package override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        pointerOwner?.acceptsFirstMouse(for: event) ?? super.acceptsFirstMouse(for: event)
    }
    package override func mouseDown(with event: NSEvent) {
        if let pointerOwner {
            mouseHeld = true
            pointerOwner.mouseDown(with: event)
        } else { super.mouseDown(with: event) }
    }
    package override func mouseDragged(with event: NSEvent) {
        if let pointerOwner { pointerOwner.mouseDragged(with: event) } else { super.mouseDragged(with: event) }
    }
    package override func mouseUp(with event: NSEvent) {
        defer {
            mouseHeld = false
            if removeAfterMouseUp { super.removeFromSuperview() }
        }
        if let pointerOwner { pointerOwner.mouseUp(with: event) } else { super.mouseUp(with: event) }
    }

    /// AppKit withdraws responsive scrolling from a subclass that overrides
    /// `scrollWheel(with:)`, and then every frame of a gesture is driven from
    /// the main thread (`NSScrollingBehaviorSingleThreadedVBL`), behind
    /// whatever else that thread is doing — mounting list rows, above all.
    /// The override below only *routes*: a gesture it keeps goes to `super`
    /// whole, which is the contract AppKit asks for. Compatible, so a
    /// contained scroller moves on AppKit's scrolling thread and the main
    /// thread follows it (LLP 1002 D4; LLP 1044 F3).
    package override class var isCompatibleWithResponsiveScrolling: Bool { true }

    private var styleObserver: NSObjectProtocol?
    package override init(frame: NSRect) {
        super.init(frame: frame)
        followScrollerStyle()
        styleObserver = NotificationCenter.default.addObserver(forName: NSScroller.preferredScrollerStyleDidChangeNotification,
            object: nil, queue: .main) { [weak self] _ in self?.followScrollerStyle() }
    }
    required init?(coder: NSCoder) { nil }
    deinit { if let styleObserver { NotificationCenter.default.removeObserver(styleObserver) } }

    private func followScrollerStyle() {
        // Region surfaces own their layout separately; this claim is for
        // ordinary overflow boxes, whose children the kernel lays out.
        scrollerStyle = ExactEnv.agentMode || pointerOwner == nil ? .overlay : NSScroller.preferredScrollerStyle
        tile()
        pointerOwner?.fitScroll()
    }

    package override func tile() {
        if let owner = pointerOwner, userInterfaceLayoutDirection != owner.userInterfaceLayoutDirection {
            userInterfaceLayoutDirection = owner.userInterfaceLayoutDirection
        }
        super.tile()
        guard let owner = pointerOwner else { return }
        // Clip geometry can differ through rounding or insets even without
        // a scrollbar. Only a visible legacy scroller occupies a gutter.
        let right = scrollerStyle == .legacy && verticalScroller?.isHidden == false ? verticalScroller?.frame.width ?? 0 : 0
        let bottom = scrollerStyle == .legacy && horizontalScroller?.isHidden == false ? horizontalScroller?.frame.height ?? 0 : 0
        owner.presenter?.covers.report("scroller-\(owner.id)", [owner.id: .gutter(right: right, bottom: bottom)])
    }

    /// Taffy reserves an RTL vertical gutter on the left. AppKit keeps
    /// its scroller on the right even with an RTL layout direction.
    var kernelDocumentOrigin: CGPoint {
        let gutter = scrollerStyle == .legacy && verticalScroller?.isHidden == false ? verticalScroller?.frame.width ?? 0 : 0
        return CGPoint(x: userInterfaceLayoutDirection == .rightToLeft ? gutter : 0, y: 0)
    }

    /// Whether a wheel this view takes is applied here, at once, and not
    /// handed to AppKit: under the agent, whose drive reads an offset back in
    /// the same turn and whose clock AppKit's scrolling does not follow (it
    /// applies a wheel some tens of milliseconds later, on its own frames,
    /// and animates a notch; and a gesture the agent hands over whole from
    /// one call is not tracked as the event queue's is: measured on macOS
    /// 27.0, AppKit applies one event of it). For a person a `scroll` node
    /// is scrolled by AppKit (LLP 1115.000 D8.2), as the page is, and so it
    /// is while a drive holds the frames for a person's own flings (`perf
    /// frames hold` under platform timing, LLP 1144 D1): momentum and rubber
    /// band included.
    static var appliesAtOnce = ExactEnv.agentMode
    /// The driver's trackpad gesture (`tap … wheel … fling|hold|release`,
    /// LLP 1142 D3) comes through the window server, whose gesture AppKit
    /// tracks as a person's, marked with this in `kCGEventSourceUserData`.
    /// It is AppKit's to run under the agent too.
    static let driverMark: Int64 = 0x4558_4143_5444
    /// The driver's events a scroller handed to AppKit, and the last such
    /// scroller: what the driver reads back to confirm delivery.
    static var driverWheels = 0
    static weak var driverOwner: ChainingScrollView?
    /// Which axes scroll (the node's effective `overflow_x`/`overflow_y`).
    var collectionWillScroll: (() -> Void)?
    var scrollsX = true
    var scrollsY = true
    /// `overscroll-behavior` per axis: what happens to a gesture this view
    /// has run out of room for. `auto` hands the rest to the enclosing
    /// scroller (CSS scroll chaining); `contain` and `none` keep it here.
    var containX = false
    var containY = false
    /// Whether a contained axis shows the platform's overscroll affordance —
    /// the rubber band. `contain` does, `none` does not (CSS Overscroll §3).
    var bouncesX = false
    var bouncesY = false

    /// What a wheel event does here.
    ///
    /// The decision is named, and computed apart from acting on it, because
    /// the defects this code has had were routing choices and not drawn
    /// frames: a zero-delta lift dropped before AppKit could see it left a
    /// rubber band stretched forever, and nothing about that is visible in a
    /// screenshot or reachable by an agent's wheel. Named, it is a pure
    /// function of the event and the geometry, and `ExactKitTests` checks it
    /// without an animation, a clock, or a window.
    enum Routing: Equatable {
        /// Hand it to `NSScrollView`: this view scrolls, and its scrolling,
        /// its momentum, its wheel step and its rubber band are AppKit's to
        /// run (LLP 1115.000 D8.2).
        case appKit
        /// Scroll this view by hand, at once: the agent's wheel
        /// (`appliesAtOnce`).
        case here
        /// Pass it to the enclosing scroller (CSS scroll chaining).
        case chain
        /// Nothing here wants it, and nothing else may have it.
        case drop
    }

    /// The geometry a routing decision reads. Passed in so a test can state
    /// one without building a window.
    struct Extent {
        var maxX: CGFloat
        var maxY: CGFloat
        var origin: CGPoint
    }

    var extent: Extent {
        let document = documentView?.frame.size ?? .zero
        let visible = contentView.bounds.size
        return Extent(maxX: max(0, document.width - visible.width),
                      maxY: max(0, document.height - visible.height),
                      origin: contentView.bounds.origin)
    }

    /// Where `dx`/`dy` goes. `phased` is whether the event carries a gesture
    /// phase — a lift or a momentum end, which have no delta at all.
    func routing(dx: CGFloat, dy: CGFloat, phased: Bool, in extent: Extent, driver: Bool = false) -> Routing {
        // What this view takes is AppKit's to scroll; the agent's wheel is
        // applied here (`appliesAtOnce`), unless the driver posted it
        // through the window server (`driverMark`).
        let taken: Routing = Self.appliesAtOnce && !driver ? .here : .appKit
        // A gesture's phase transitions carry no delta — the lift that ends a
        // trackpad scroll is a zero-delta `.ended`, and momentum ends the
        // same way — and AppKit's elastic state machine needs them: without
        // the lift it never learns the gesture is over, so a stretched rubber
        // band stays stretched. A contained axis is AppKit's to drive, so it
        // sees every event of the gesture, delta or not. An `auto` axis sees
        // them once the gesture is its own (`latched`): before any scroller
        // owns a gesture, one that may yet go to the scroller around this
        // one is not begun here.
        if dx == 0 && dy == 0 {
            return (containX || containY) && phased ? .appKit : .drop
        }
        // A wheel with no phases is its own gesture and is not split (LLP
        // 1070 G2, Chrome's rule, measured in its §2): the deepest scroller
        // that can take any of its components takes it; one that can take
        // none chains, unless the tick's axis is contained here. A mouse
        // wheel does not rubber-band on this platform, whoever scrolls it.
        if !phased {
            if take(dx, scrolls: scrollsX, limit: extent.maxX, at: extent.origin.x)
                || take(dy, scrolls: scrollsY, limit: extent.maxY, at: extent.origin.y) {
                return taken
            }
            return (dx != 0 && containX) || (dy != 0 && containY) ? .drop : .chain
        }
        // The dominant axis decides who owns a gesture: a gesture is one thing.
        let vertical = abs(dy) >= abs(dx)
        let room = vertical ? (scrollsY && extent.maxY > 0) : (scrollsX && extent.maxX > 0)
        // A contained axis never hands a gesture to an ancestor. Contained
        // with nowhere to go, it stops here: passing it on is exactly what
        // `contain` forbids. With room the gesture is AppKit's, under the
        // agent too: its rubber band is only AppKit's to draw.
        if vertical ? containY : containX {
            return room ? .appKit : .drop
        }
        // Per axis: can this view move in the delta's direction? (Flipped
        // document: origin grows as content scrolls up; a negative delta
        // scrolls content up.) A gesture that begins at this view's end goes
        // to the scroller around it.
        return take(vertical ? dy : dx,
                    scrolls: vertical ? scrollsY : scrollsX,
                    limit: vertical ? extent.maxY : extent.maxX,
                    at: vertical ? extent.origin.y : extent.origin.x) ? taken : .chain
    }

    private static func handedDriverWheel(_ view: ChainingScrollView) {
        driverWheels += 1
        driverOwner = view
    }

    private func take(_ delta: CGFloat, scrolls: Bool, limit: CGFloat, at origin: CGFloat) -> Bool {
        scrolls && delta != 0 && limit > 0 && ((delta < 0 && origin < limit) || (delta > 0 && origin > 0))
    }

    /// Chrome's wheel scroll latching for a trackpad gesture (LLP 1070 G1):
    /// the gesture belongs to one scroller from its first moving event to
    /// its end, momentum included, and is never handed on mid-gesture. The
    /// owner is the first scroller, innermost out, whose routing of that
    /// event is not `.chain`; one destroyed mid-gesture takes the rest with
    /// it. A new `began` starts a new gesture.
    struct Latch {
        weak var owner: ChainingScrollView?
        var chosen = false
        var active = false
        /// The owner's gesture is AppKit's to run.
        var appKit = false
    }
    static var latch = Latch()
    /// The gesture's zero-delta `began`, while no scroller owns the gesture.
    static var unsentBegan: NSEvent?

    /// What a phased event does here, given the latch: the routing to act
    /// on, choosing this view as the owner when it is the first to take it.
    /// Pure over the latch, so `ScrollRoutingTests` can drive a gesture.
    static func latched(_ latch: inout Latch, view: ChainingScrollView, began: Bool, over: Bool,
                        moving: Bool, routing: Routing) -> Routing {
        if began { latch = Latch(owner: nil, chosen: false, active: true) }
        defer { if over { latch = Latch() } }
        guard latch.active else { return routing }
        if latch.chosen {
            guard let owner = latch.owner else { return .drop }
            if owner !== view { return view.isDescendant(of: owner) ? .chain : .drop }
            // A gesture AppKit runs is AppKit's whole, which is the contract
            // it asks for: the events past this view's end, where it
            // rubber-bands or stops as its elasticity says and never hands
            // on, and the zero-delta lift and momentum's end, without which
            // a stretched band stays stretched.
            if latch.appKit { return .appKit }
            // Scrolled by hand, the owner keeps what it cannot take: the
            // rest is dropped, never chained.
            return routing == .chain ? .drop : routing
        }
        guard moving, routing != .chain else { return routing }
        latch.owner = view
        latch.chosen = true
        latch.appKit = routing == .appKit
        return routing
    }

    package override func scrollWheel(with event: NSEvent) {
        (superview as? NodeView)?.cancelElementScroll()
        // Precise deltas (a trackpad) are in points; a wheel's are in lines,
        // of the height AppKit scrolls a line by (10 points unless set; it
        // was 40 here, a browser's tick).
        let precise = event.hasPreciseScrollingDeltas
        var dx = precise ? event.scrollingDeltaX : event.deltaX * horizontalLineScroll
        var dy = precise ? event.scrollingDeltaY : event.deltaY * verticalLineScroll
        let phased = !(event.phase.isEmpty && event.momentumPhase.isEmpty)
        let extent = self.extent
        let driver = event.cgEvent?.getIntegerValueField(.eventSourceUserData) == Self.driverMark
        var route = routing(dx: dx, dy: dy, phased: phased, in: extent, driver: driver)
        if phased {
            let began = event.phase.contains(.began) && event.momentumPhase.isEmpty
            let over = event.momentumPhase.contains(.ended) || event.momentumPhase.contains(.cancelled)
                || event.phase.contains(.cancelled)
            let owned = Self.latch.chosen
            route = Self.latched(&Self.latch, view: self, began: began, over: over,
                                 moving: dx != 0 || dy != 0, routing: route)
            // A `began` with no delta is nobody's yet and is dropped where it
            // lands. The scroller that comes to own the gesture hands it to
            // AppKit first, so the gesture AppKit runs has its beginning; the
            // event that made it the owner goes back to the head of the
            // queue, where AppKit's tracking, or the window, takes it next.
            if began { Self.unsentBegan = dx == 0 && dy == 0 && route == .drop ? event : nil }
            if route == .appKit, !owned, Self.latch.chosen, let first = Self.unsentBegan, first !== event {
                Self.unsentBegan = nil
                collectionWillScroll?()
                if driver { Self.handedDriverWheel(self) }
                NSApp.postEvent(event, atStart: true)
                super.scrollWheel(with: first)
                return
            }
            if over { Self.unsentBegan = nil }
        }
        switch route {
        case .drop:
            return
        case .appKit:
            collectionWillScroll?()
            if driver { Self.handedDriverWheel(self) }
            super.scrollWheel(with: event)
        case .chain:
            nextResponder?.scrollWheel(with: event)
        case .here:
            collectionWillScroll?()
            // The agent's wheel: what this view can take of it, it takes
            // itself, at once, and the rest is dropped.
            if !take(dx, scrolls: scrollsX, limit: extent.maxX, at: extent.origin.x) { dx = 0 }
            if !take(dy, scrolls: scrollsY, limit: extent.maxY, at: extent.origin.y) { dy = 0 }
            let target = NSPoint(x: min(max(extent.origin.x - dx, 0), extent.maxX),
                                 y: min(max(extent.origin.y - dy, 0), extent.maxY))
            contentView.scroll(to: target)
            reflectScrolledClipView(contentView)
        }
    }
}

extension NodeView {
    /// Kernel frames include the gutter; the document starts at the clip.
    func frameInScrollDocument(_ frame: NSRect) -> NSRect {
        guard let clip = superview?.superview as? NSClipView,
              let scroll = clip.superview as? ChainingScrollView,
              scroll.documentView === superview else { return frame }
        let origin = scroll.kernelDocumentOrigin
        return frame.offsetBy(dx: -origin.x, dy: -origin.y)
    }
}

#endif
