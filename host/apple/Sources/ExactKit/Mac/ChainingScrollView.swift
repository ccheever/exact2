#if os(macOS)
import AppKit

/// A scroll container that chains: a wheel event it cannot consume in its
/// dominant direction — nothing to scroll, or already at that edge — goes
/// to the next responder, so an inner `scroll` node never traps the page.
/// The web's rule (`overscroll-behavior: auto`); AppKit's default is to
/// swallow it.
final class ChainingScrollView: NSScrollView {
    /// AppKit withdraws responsive scrolling from a subclass that overrides
    /// `scrollWheel(with:)`, and then every frame of a gesture is driven from
    /// the main thread (`NSScrollingBehaviorSingleThreadedVBL`), behind
    /// whatever else that thread is doing — mounting list rows, above all.
    /// The override below only *routes*: a gesture it keeps goes to `super`
    /// whole, which is the contract AppKit asks for. Compatible, so a
    /// contained scroller moves on AppKit's scrolling thread and the main
    /// thread follows it (LLP 1002 D4; LLP 1044 F3).
    override class var isCompatibleWithResponsiveScrolling: Bool { true }

    /// Always overlay: a `scroll` node's scrollbars take no layout space
    /// (LLP 1010: the kernel's `scrollbar_width` is 0). AppKit sets every
    /// scroll view to the system's preferred style when it posts
    /// `NSPreferredScrollerStyleDidChangeNotification` — on a Mac with a
    /// mouse, legacy, whose 15-point scrollers would take a node's clip
    /// narrower and shorter than the box the kernel laid out.
    override var scrollerStyle: NSScroller.Style {
        get { super.scrollerStyle }
        set { super.scrollerStyle = .overlay }
    }

    /// Points per line for a wheel without precise deltas — the browser's
    /// tick.
    static let lineHeight: CGFloat = 40
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
        /// Hand the gesture to `NSScrollView` — a contained axis, whose
        /// scrolling, momentum, and rubber band are AppKit's to run.
        case appKit
        /// Scroll this view by hand: an `auto` axis with room to move.
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
    func routing(dx: CGFloat, dy: CGFloat, phased: Bool, in extent: Extent) -> Routing {
        // A gesture's phase transitions carry no delta — the lift that ends a
        // trackpad scroll is a zero-delta `.ended`, and momentum ends the
        // same way — and AppKit's elastic state machine needs them: without
        // the lift it never learns the gesture is over, so a stretched rubber
        // band stays stretched. A contained axis is AppKit's to drive, so it
        // sees every event of the gesture, delta or not. An `auto` axis is
        // driven here and has no use for them.
        if dx == 0 && dy == 0 {
            return (containX || containY) && phased ? .appKit : .drop
        }
        // A wheel with no phases is its own gesture and is not split (LLP
        // 1070 G2, Chrome's rule, measured in its §2): the deepest scroller
        // that can take any of its components takes the ones it can, and
        // the rest is dropped; one that can take none chains, unless the
        // tick's axis is contained here.
        if !phased {
            if take(dx, scrolls: scrollsX, limit: extent.maxX, at: extent.origin.x)
                || take(dy, scrolls: scrollsY, limit: extent.maxY, at: extent.origin.y) {
                return .here
            }
            return (dx != 0 && containX) || (dy != 0 && containY) ? .drop : .chain
        }
        // The dominant axis decides who owns a gesture: a gesture is one thing.
        let vertical = abs(dy) >= abs(dx)
        let room = vertical ? (scrollsY && extent.maxY > 0) : (scrollsX && extent.maxX > 0)
        // A contained axis never hands a gesture to an ancestor. Contained
        // with nowhere to go, it stops here: passing it on is exactly what
        // `contain` forbids.
        //
        // Only a *gesture* goes to AppKit. A wheel with no phases is a mouse
        // wheel, and a mouse wheel does not rubber-band on this platform —
        // elastic overscroll is a trackpad's. Clamping one here rather than
        // handing it over keeps it synchronous, and synchronous is what makes
        // it drivable: `super.scrollWheel` scrolls over several frames, so an
        // agent that wheels and then reads the offset races the animation.
        // A phased gesture gets AppKit, its momentum, and its rubber band.
        if vertical ? containY : containX {
            if !room { return .drop }
            return phased ? .appKit : .here
        }
        // Per axis: can this view move in the delta's direction? (Flipped
        // document: origin grows as content scrolls up; a negative delta
        // scrolls content up.)
        return take(vertical ? dy : dx,
                    scrolls: vertical ? scrollsY : scrollsX,
                    limit: vertical ? extent.maxY : extent.maxX,
                    at: vertical ? extent.origin.y : extent.origin.x) ? .here : .chain
    }

    private func take(_ delta: CGFloat, scrolls: Bool, limit: CGFloat, at origin: CGFloat) -> Bool {
        scrolls && delta != 0 && limit > 0 && ((delta < 0 && origin < limit) || (delta > 0 && origin > 0))
    }

    override func scrollWheel(with event: NSEvent) {
        // Precise deltas (a trackpad) are in points; a wheel's are in lines.
        let precise = event.hasPreciseScrollingDeltas
        var dx = precise ? event.scrollingDeltaX : event.deltaX * ChainingScrollView.lineHeight
        var dy = precise ? event.scrollingDeltaY : event.deltaY * ChainingScrollView.lineHeight
        let phased = !(event.phase.isEmpty && event.momentumPhase.isEmpty)
        let extent = self.extent
        switch routing(dx: dx, dy: dy, phased: phased, in: extent) {
        case .drop:
            return
        case .appKit:
            collectionWillScroll?()
            super.scrollWheel(with: event)
        case .chain:
            nextResponder?.scrollWheel(with: event)
        case .here:
            collectionWillScroll?()
            // What this view can take of it, it takes itself — never through
            // AppKit, whose nested-scroll routing may move the enclosing view
            // or animate later, doubling a delta applied here.
            if !take(dx, scrolls: scrollsX, limit: extent.maxX, at: extent.origin.x) { dx = 0 }
            if !take(dy, scrolls: scrollsY, limit: extent.maxY, at: extent.origin.y) { dy = 0 }
            let target = NSPoint(x: min(max(extent.origin.x - dx, 0), extent.maxX),
                                 y: min(max(extent.origin.y - dy, 0), extent.maxY))
            contentView.scroll(to: target)
            reflectScrolledClipView(contentView)
        }
    }
}

#endif
