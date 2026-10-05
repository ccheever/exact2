// Shared-element flights on UIKit (LLP 1013.000 D4, D5).
//
// A `flight` op names a view whose shared-element name arrived from
// another. It comes before the batch's destroys, so the leaver is still
// here: where it is shown (window coordinates: its ancestors' transforms,
// scroll offsets and a drag in progress included), its radius and an
// image's fitted rectangle are captured. When the batch is applied, the
// arriver's place is scrolled into view, then the arriver itself — live,
// not a snapshot — is lifted into a pass-through layer at the top of its
// presentation's root view, leaving an empty slot where it lays out. Each
// frame the host presents the curve's progress; the view is shown at the
// interpolation between the captured rectangle and its slot (re-read every
// frame, so a scroll that settles moves the landing), with its radius and an
// image's crop interpolated too. `land` puts it back.
#if os(iOS) || os(tvOS)
import UIKit

/// The look an arriver shows while it flies: where its image is drawn,
/// in its own bounds (`applyImageLayer` defers to it).
struct FlightLook {
    var image: CGRect
    /// The leaver's decoded image, drawn while the arriver's own is still
    /// loading: a new node's raster lands a turn or more after the commit
    /// that hides the leaver, and a flight drawing nothing until then showed
    /// no photo at all for a frame or two on a device (LLP 1013.000 D4).
    var stand: NativeRasterLease? = nil
}

/// Where a leaver was shown when its name moved on.
struct FlightSource {
    var rect: CGRect // window coordinates
    var radius: CGFloat
    /// An image's fitted rectangle as a fraction of its box.
    var fit: CGRect?
    var natural: CGSize?
    /// The leaver's decoded image (`FlightLook.stand`), held for the flight.
    var raster: NativeRasterLease?
    /// The root of the presentation the leaver was in: a flight from an
    /// overlay over the routes into a route flies over that overlay.
    weak var root: UIView?
}

final class Flight {
    let id: UInt32
    let source: FlightSource
    var progress: CGFloat = 0
    weak var view: NodeView?
    var slot: UIView?
    var container: UIView?
    var geometry: BatchOp?
    var saved: (radius: CGFloat, clips: Bool, interaction: Bool, hidden: Bool)?
    init(id: UInt32, source: FlightSource) { self.id = id; self.source = source }
}

/// The flight layer: draws its flights, takes no touches.
final class FlightLayer: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}

extension Presenter {
    /// The `flight` op: capture the leaver, before any destroy.
    func beginFlight(_ op: BatchOp) {
        let id = op.id
        guard let from = (op.payload["from"] as? NSNumber)?.uint32Value,
              let leaver = views[from] ?? leaving[from]?.view, leaver.window != nil else {
            flights[id] = Flight(id: id, source: FlightSource(rect: .null, radius: 0))
            return
        }
        var source = FlightSource(rect: leaver.convert(leaver.bounds, to: nil), radius: leaver.cornerRadii(in: leaver.bounds).max() ?? 0)
        if let flying = flights.values.first(where: { $0.view === leaver }), let look = leaver.flightLook {
            // A flight interrupted: from where it is now.
            source.fit = CGRect(x: look.image.minX / max(leaver.bounds.width, 1), y: look.image.minY / max(leaver.bounds.height, 1),
                                width: look.image.width / max(leaver.bounds.width, 1), height: look.image.height / max(leaver.bounds.height, 1))
            source.natural = flying.source.natural ?? leaver.raster?.image.naturalSize
            source.radius = leaver.layer.cornerRadius
            source.raster = leaver.raster ?? look.stand
            // The size of the image the stand draws, with it: after the
            // flying view's own raster landed, that is the replacement's.
            source.natural = source.raster?.image.naturalSize ?? source.natural
        } else if leaver.kind == "image", let natural = leaver.raster?.image.naturalSize {
            source.fit = Self.fitFraction(natural: natural, box: leaver.bounds.size, fit: leaver.style["object_fit"]?.string ?? "fill")
            source.natural = natural
            source.raster = leaver.raster
        }
        source.root = presentationRoot(of: leaver)
        if let old = flights[id] { landFlight(old) }
        flights[id] = Flight(id: id, source: source)
    }

    /// The `flight` progress the host presents this frame.
    func presentFlight(_ id: UInt32, _ progress: CGFloat) {
        flights[id]?.progress = progress
    }

    /// A flying view's frame op waits in its slot until it lands.
    func flightFrame(_ op: BatchOp) -> Bool {
        guard op.op == .frame, let f = flights[op.id], f.view != nil else { return false }
        f.geometry = op
        f.slot?.frame = CGRect(x: op.x, y: op.y, width: op.w, height: op.h)
        return true
    }

    func isFlying(_ view: NodeView) -> Bool { flights[view.id]?.view === view && view.flightLook != nil }

    /// After the batch: lift new flights, show every flight at its progress.
    func flightsBatchApplied() {
        for f in Array(flights.values) {
            if f.view == nil, f.slot == nil { liftFlight(f) }
            showFlight(f)
        }
    }

    /// The `land` op, or a flight replaced or destroyed.
    func landFlight(_ f: Flight) {
        flights.removeValue(forKey: f.id)
        guard let view = f.view, let slot = f.slot, let parent = slot.superview else {
            // Its place went (a roots change): the view is wherever that put
            // it; it gets its own look, input and accessibility back.
            if let view = f.view { restore(view, f) }
            f.slot?.removeFromSuperview(); f.container.map(Self.dropEmptyLayer); return
        }
        view.flightLook = nil
        parent.insertSubview(view, aboveSubview: slot)
        slot.removeFromSuperview()
        restore(view, f)
        let op = f.geometry ?? {
            var op = BatchOp(op: .frame, nodeID: f.id)
            op.x = slot.frame.minX; op.y = slot.frame.minY; op.w = slot.frame.width; op.h = slot.frame.height
            return op
        }()
        applyGeometry(op)
        view.setNeedsLayout()
        if view.kind == "image" { view.applyImageLayer() }
        f.container.map(Self.dropEmptyLayer)
    }

    private func restore(_ view: NodeView, _ f: Flight) {
        view.flightLook = nil
        if let s = f.saved {
            view.layer.cornerRadius = s.radius
            view.layer.masksToBounds = s.clips
            view.isUserInteractionEnabled = s.interaction
            view.accessibilityElementsHidden = s.hidden
        }
    }

    /// A destroyed arriver's flight ends with it.
    func forgetFlight(_ id: UInt32) {
        guard let f = flights.removeValue(forKey: id) else { return }
        // The look holds the leaver's lease (`stand`): it goes with the flight.
        f.view?.flightLook = nil
        f.view?.removeFromSuperview()
        f.slot?.removeFromSuperview()
        f.container.map(Self.dropEmptyLayer)
    }

    /// Flights whose place is inside `view` land now: it is leaving with an
    /// exit, and they leave with it.
    func landFlights(inside view: UIView) {
        for f in flights.values where f.slot?.isDescendant(of: view) == true { landFlight(f) }
    }

    /// A reset ends every flight, its view and layer with it.
    func resetFlights() {
        for f in flights.values {
            f.view?.flightLook = nil
            f.view?.removeFromSuperview(); f.slot?.removeFromSuperview(); f.container?.removeFromSuperview()
        }
        flights = [:]
    }

    private func liftFlight(_ f: Flight) {
        // A document root is placed by `roots`, not by a parent: it lands in place.
        guard let view = views[f.id], let parent = view.superview, parent !== root, view.window != nil, !f.source.rect.isNull,
              !DisplayPreferences.reducedMotion else {
            flights.removeValue(forKey: f.id)
            // Not flying (reduced motion, nothing captured): its place still
            // comes into view (D5).
            if let view = views[f.id], view.window != nil { afterBatch { [weak self, weak view] in if let view { self?.scrollIntoView(view) } } }
            return
        }
        let size = view.bounds.size
        let origin = CGPoint(x: view.layer.position.x - view.layer.anchorPoint.x * size.width,
                             y: view.layer.position.y - view.layer.anchorPoint.y * size.height)
        let slot = UIView(frame: CGRect(origin: origin, size: size))
        slot.isUserInteractionEnabled = false
        slot.isHidden = true
        parent.insertSubview(slot, belowSubview: view)
        // Once the batch is done: a virtualized list hears a scroll made
        // inside one as no move of its own (LLP 1013.000 D5). The flight
        // re-reads its slot each frame, so it follows.
        afterBatch { [weak self, weak slot, weak f] in
            guard let self, let slot, let f else { return }
            self.scrollIntoView(slot)
            self.showFlight(f)
        }
        guard let own = presentationRoot(of: parent) else { slot.removeFromSuperview(); flights.removeValue(forKey: f.id); return }
        // The flight goes in B's presentation (D4 step 3), unless A's encloses
        // it: then in A's, so a photo closing from an overlay above the routes
        // into a thumbnail in a route stays above that overlay as it fades,
        // as UIKit's and Signal's zooms keep the image above their backdrops.
        // A flight inside one modal still stays inside it.
        let root = f.source.root.flatMap { outer in own !== outer && own.isDescendant(of: outer) ? outer : nil } ?? own
        let layer = root.subviews.last as? FlightLayer ?? {
            let l = FlightLayer(frame: root.bounds)
            l.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            root.addSubview(l)
            // Above every ranked sibling, as a transition snapshot is: a
            // route's document plane sits at rank ½ (LLP 1083.000 D4), and a
            // flight layer left at depth 0 drew under it, so a flight landing
            // in a route was never seen.
            l.setPaintForeground()
            return l
        }()
        f.saved = (view.layer.cornerRadius, view.layer.masksToBounds, view.isUserInteractionEnabled, view.accessibilityElementsHidden)
        f.view = view
        f.slot = slot
        f.container = layer
        view.transform = .identity
        layer.addSubview(view)
        view.isUserInteractionEnabled = false
        view.accessibilityElementsHidden = true
        view.layer.masksToBounds = true
        view.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner, .layerMaxXMaxYCorner, .layerMinXMaxYCorner]
        view.flightLook = FlightLook(image: CGRect(origin: .zero, size: size))
    }

    private func showFlight(_ f: Flight) {
        guard let view = f.view, let slot = f.slot, let layer = f.container else { return }
        let p = f.progress
        let from = layer.convert(f.source.rect, from: nil)
        let to = slot.convert(slot.bounds, to: layer)
        func mix(_ a: CGFloat, _ b: CGFloat) -> CGFloat { a + (b - a) * p }
        let shown = CGRect(x: mix(from.minX, to.minX), y: mix(from.minY, to.minY),
                           width: max(0, mix(from.width, to.width)), height: max(0, mix(from.height, to.height)))
        CATransaction.begin(); CATransaction.setDisableActions(true)
        view.transform = .identity
        view.frame = shown
        view.layer.cornerRadius = mix(f.source.radius, view.cornerRadii(in: CGRect(origin: .zero, size: to.size)).max() ?? 0)
        if view.kind == "image" {
            let natural = view.raster?.image.naturalSize ?? f.source.raster?.image.naturalSize ?? f.source.natural ?? .zero
            let end = Self.fitFraction(natural: natural, box: to.size, fit: view.style["object_fit"]?.string ?? "fill")
            let start = f.source.fit ?? end
            view.flightLook = FlightLook(image: Self.flightImage(from: from.size, fit: start, to: to.size, fit: end, progress: p),
                                         stand: view.raster == nil ? f.source.raster : nil)
            view.applyImageLayer()
        } else {
            view.flightLook = FlightLook(image: CGRect(origin: .zero, size: shown.size))
        }
        CATransaction.commit()
    }

    /// Every scroller between the slot and its presentation scrolls by the
    /// least that shows it whole (`scrollIntoView`'s `nearest`), innermost
    /// first, at once (LLP 1013.000 D5).
    private func scrollIntoView(_ slot: UIView) {
        var ancestor = slot.superview
        while let current = ancestor {
            if let sv = current as? UIScrollView, sv.isScrollEnabled || sv === viewport {
                let r = sv.convert(slot.bounds, from: slot)
                let inset = sv.adjustedContentInset
                let visible = sv.bounds.inset(by: inset)
                var offset = sv.contentOffset
                func nearest(_ lo: CGFloat, _ hi: CGFloat, _ vlo: CGFloat, _ vhi: CGFloat) -> CGFloat {
                    if lo >= vlo && hi <= vhi { return 0 }
                    if hi - lo > vhi - vlo { return lo - vlo }
                    return lo < vlo ? lo - vlo : hi - vhi
                }
                offset.x += nearest(r.minX, r.maxX, visible.minX, visible.maxX)
                offset.y += nearest(r.minY, r.maxY, visible.minY, visible.maxY)
                offset.x = min(max(offset.x, -inset.left), max(-inset.left, sv.contentSize.width - sv.bounds.width + inset.right))
                offset.y = min(max(offset.y, -inset.top), max(-inset.top, sv.contentSize.height - sv.bounds.height + inset.bottom))
                if offset != sv.contentOffset { sv.setContentOffset(offset, animated: false) }
            }
            if let vc = current.next as? UIViewController, vc.view === current, Self.presents(vc) { break }
            ancestor = current.superview
        }
    }

    /// The root view of the controller presenting `view`: a route's, a
    /// sheet's, a fullscreen presentation's, else the window.
    private func presentationRoot(of view: UIView) -> UIView? {
        var ancestor: UIView? = view
        while let current = ancestor {
            if let vc = current.next as? UIViewController, vc.view === current, Self.presents(vc) { return current }
            ancestor = current.superview
        }
        return view.window
    }

    /// A controller whose view is a presentation's root: a route in a
    /// navigation or tab container, or one with no parent (a window's root,
    /// a sheet, a fullscreen presentation).
    private static func presents(_ vc: UIViewController) -> Bool {
        vc.parent == nil || vc.parent is UINavigationController || vc.parent is UITabBarController
    }

    private static func dropEmptyLayer(_ layer: UIView) {
        if layer.subviews.isEmpty { layer.removeFromSuperview() }
    }

    /// Where a flying image is drawn in its shown box, at `progress`: the
    /// image moves in points from where A drew it (`fit` of A's box) to where
    /// B draws it (`fit` of B's), as UIKit's zoom moves it, so its offset in
    /// the box and its size are each a mix of the two ends' own. A mix of the
    /// two fractions times the mixed box is not linear: a cover thumbnail
    /// opening into a contain photo grew 25% taller than either end.
    static func flightImage(from: CGSize, fit start: CGRect, to: CGSize, fit end: CGRect, progress p: CGFloat) -> CGRect {
        func mix(_ a: CGFloat, _ b: CGFloat) -> CGFloat { a + (b - a) * p }
        return CGRect(x: mix(start.minX * from.width, end.minX * to.width), y: mix(start.minY * from.height, end.minY * to.height),
                      width: mix(start.width * from.width, end.width * to.width), height: mix(start.height * from.height, end.height * to.height))
    }

    /// An image's fitted rectangle in a box, as a fraction of the box.
    static func fitFraction(natural: CGSize, box: CGSize, fit: String) -> CGRect {
        guard box.width > 0, box.height > 0, natural.width > 0, natural.height > 0 else { return CGRect(x: 0, y: 0, width: 1, height: 1) }
        let r = RasterGeometry.rect(natural: natural, content: CGRect(origin: .zero, size: box), fit: fit)
        return CGRect(x: r.minX / box.width, y: r.minY / box.height, width: r.width / box.width, height: r.height / box.height)
    }
}
#endif
