// Shared-element flights on AppKit (LLP 1013.000 D4, D5): as on UIKit
// (`FlightsIOS.swift`), in the window's content view. The leaver is captured
// before the batch's destroys; once it is applied the arriver's place is
// scrolled into view, the live arriver is lifted into a pass-through layer
// over the content, shown each frame between the captured rectangle and its
// slot (re-read every frame), with its radius and an image's crop
// interpolated (any other view scaled whole in a clip that is the shown box,
// `FlightScale`), and put back at `land`.
#if os(macOS)
import AppKit

/// The look an arriver shows while it flies: where its image is drawn,
/// in its own bounds (`applyImageLayer` defers to it).
struct FlightLook {
    var image: CGRect
    /// The leaver's decoded image, drawn while the arriver's own is still
    /// loading: a new node's raster lands a turn or more after the commit
    /// that hides the leaver, and a flight drawing nothing until then showed
    /// no photo at all for a frame or two on a device (LLP 1013.000 D4).
    var stand: NativeRasterLease? = nil
    /// A view scaled whole in its clip (`FlightScale`): `applyTransform`
    /// keeps this scale while it flies, as a layout would otherwise undo it.
    var scale: CGFloat? = nil
}

/// Where a leaver was shown when its name moved on.
struct FlightSource {
    var rect: NSRect // the window's content view coordinates
    var radius: CGFloat
    /// An image's fitted rectangle as a fraction of its box.
    var fit: CGRect?
    var natural: CGSize?
    /// The leaver's decoded image (`FlightLook.stand`), held for the flight.
    var raster: NativeRasterLease?
}

final class Flight {
    let id: UInt32
    let source: FlightSource
    var progress: CGFloat = 0
    weak var view: NodeView?
    var slot: NSView?
    var container: NSView?
    /// A view that is not an image flies scaled inside this: the shown box,
    /// its radius and its clip (D4.4).
    var clip: NSView?
    var frame: NSRect?
    var saved: (radius: CGFloat, clips: Bool)?
    init(id: UInt32, source: FlightSource) { self.id = id; self.source = source }
}

/// The flight layer: draws its flights, takes no clicks.
final class FlightLayer: NSView {
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

/// A flight's clip: the shown box of a view flying scaled (D4.4).
final class FlightClip: NSView {
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

/// A flying view's empty place in its parent.
final class FlightSlot: NSView {
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

extension Presenter {
    func beginFlight(_ op: BatchOp) {
        let id = op.id
        guard let from = (op.payload["from"] as? NSNumber)?.uint32Value,
              let leaver = views[from] ?? leaving[from]?.view, let content = leaver.window?.contentView else {
            flights[id] = Flight(id: id, source: FlightSource(rect: .null, radius: 0))
            return
        }
        // Through the layers: AppKit's own conversion ignores the layer
        // transforms a drag or a scale presents.
        let shown = leaver.layer.flatMap { l in content.layer.map { l.convert(l.bounds, to: $0) } } ?? leaver.convert(leaver.bounds, to: content)
        var source = FlightSource(rect: shown, radius: leaver.cornerRadii(in: leaver.bounds).max() ?? 0)
        if let flying = flights.values.first(where: { $0.view === leaver }), let look = leaver.flightLook {
            let b = leaver.bounds
            source.fit = CGRect(x: look.image.minX / max(b.width, 1), y: look.image.minY / max(b.height, 1),
                                width: look.image.width / max(b.width, 1), height: look.image.height / max(b.height, 1))
            source.natural = flying.source.natural ?? leaver.raster?.image.naturalSize
            source.radius = leaver.layer?.cornerRadius ?? 0
            if let clip = flying.clip {
                // Scaled in its clip: where and how round the clip is shown.
                // The clip itself is unscaled, so AppKit's own conversion is
                // right (the content view may have no layer to convert to).
                source.rect = clip.convert(clip.bounds, to: content)
                source.radius = clip.layer?.cornerRadius ?? 0
            }
            source.raster = leaver.raster ?? look.stand
            // The size of the image the stand draws, with it: after the
            // flying view's own raster landed, that is the replacement's.
            source.natural = source.raster?.image.naturalSize ?? source.natural
        } else if leaver.kind == "image", let natural = leaver.raster?.image.naturalSize {
            source.fit = Self.fitFraction(natural: natural, box: leaver.bounds.size, fit: leaver.style["object_fit"]?.string ?? "fill")
            source.natural = natural
            source.raster = leaver.raster
        }
        if let old = flights[id] { landFlight(old) }
        flights[id] = Flight(id: id, source: source)
    }

    func presentFlight(_ id: UInt32, _ progress: CGFloat) {
        flights[id]?.progress = progress
    }

    /// A flying view's frame op waits in its slot until it lands.
    func flightFrame(_ id: UInt32, _ frame: NSRect) -> Bool {
        guard let f = flights[id], f.view != nil else { return false }
        f.frame = frame
        f.slot?.frame = frame
        return true
    }

    func isFlying(_ view: NodeView) -> Bool { flights[view.id]?.view === view && view.flightLook != nil }

    func flightsBatchApplied() {
        for f in Array(flights.values) {
            if f.view == nil, f.slot == nil { liftFlight(f) }
            showFlight(f)
        }
    }

    func landFlight(_ f: Flight) {
        flights.removeValue(forKey: f.id)
        guard let view = f.view, let slot = f.slot, let parent = slot.superview else {
            // Its place went (a roots change): it gets its own look back.
            // Still in its clip, it stands at the clip's top left, unscaled,
            // at its own size.
            if let view = f.view {
                view.flightLook = nil
                if let clip = f.clip, view.superview === clip, let layer = clip.superview {
                    view.removeFromSuperview()
                    layer.addSubview(view, positioned: .above, relativeTo: clip)
                    view.frame = NSRect(origin: clip.frame.origin, size: view.frame.size)
                }
                if let s = f.saved { view.layer?.cornerRadius = s.radius; view.layer?.masksToBounds = s.clips }
                view.applyTransform()
            }
            f.clip?.removeFromSuperview()
            f.slot?.removeFromSuperview(); f.container.map(Self.dropEmptyLayer); return
        }
        view.flightLook = nil
        view.removeFromSuperview()
        parent.addSubview(view, positioned: .above, relativeTo: slot)
        slot.removeFromSuperview()
        f.clip?.removeFromSuperview()
        view.frame = f.frame ?? slot.frame
        if let s = f.saved {
            view.layer?.cornerRadius = s.radius
            view.layer?.masksToBounds = s.clips
        }
        view.applyTransform()
        if view.kind == "image" { view.applyImageLayer() }
        view.needsDisplay = true
        f.container.map(Self.dropEmptyLayer)
    }

    func forgetFlight(_ id: UInt32) {
        guard let f = flights.removeValue(forKey: id) else { return }
        // The look holds the leaver's lease (`stand`): it goes with the flight.
        f.view?.flightLook = nil
        f.view?.removeFromSuperview()
        f.slot?.removeFromSuperview()
        f.clip?.removeFromSuperview()
        f.container.map(Self.dropEmptyLayer)
    }

    /// Flights whose place is inside `view` land now: it is leaving with an
    /// exit, and they leave with it.
    func landFlights(inside view: NSView) {
        for f in flights.values where f.slot?.isDescendant(of: view) == true { landFlight(f) }
    }

    /// A reset ends every flight, its view and layer with it.
    func resetFlights() {
        for f in flights.values {
            f.view?.flightLook = nil
            f.view?.removeFromSuperview(); f.slot?.removeFromSuperview(); f.clip?.removeFromSuperview(); f.container?.removeFromSuperview()
        }
        flights = [:]
    }

    private func liftFlight(_ f: Flight) {
        // A document root is placed by `roots`, not by a parent: it lands in place.
        guard let view = views[f.id], let parent = view.superview, parent !== root, let content = view.window?.contentView, !f.source.rect.isNull,
              !DisplayPreferences.reducedMotion else {
            flights.removeValue(forKey: f.id)
            // Not flying (reduced motion, nothing captured): its place still
            // comes into view (D5).
            if let view = views[f.id], view.window != nil { afterBatch { [weak self, weak view] in if let view { self?.scrollIntoView(view) } } }
            return
        }
        let slot = FlightSlot(frame: view.frame)
        slot.wantsLayer = true
        slot.isHidden = true
        parent.addSubview(slot, positioned: .below, relativeTo: view)
        // Once the batch is done: a virtualized list hears a scroll made
        // inside one as no move of its own (LLP 1013.000 D5). The flight
        // re-reads its slot each frame, so it follows.
        afterBatch { [weak self, weak slot, weak f] in
            guard let self, let slot, let f else { return }
            self.scrollIntoView(slot)
            self.showFlight(f)
        }
        let layer = content.subviews.last as? FlightLayer ?? {
            let l = FlightLayer(frame: content.bounds)
            l.wantsLayer = true
            l.autoresizingMask = [.width, .height]
            content.addSubview(l)
            // Above every ranked sibling (LLP 1083.000 D4), as on iOS.
            l.setPaintForeground()
            return l
        }()
        f.saved = (view.layer?.cornerRadius ?? 0, view.layer?.masksToBounds ?? false)
        f.view = view
        f.slot = slot
        f.container = layer
        view.removeFromSuperview()
        if view.kind == "image" {
            layer.addSubview(view)
            view.layer?.masksToBounds = true
            view.flightLook = FlightLook(image: CGRect(origin: .zero, size: view.bounds.size))
        } else {
            // Its own look, at its own size, scaled in a clip (D4.4).
            let clip = FlightClip(frame: .zero)
            clip.wantsLayer = true
            clip.layer?.masksToBounds = true
            layer.addSubview(clip)
            clip.addSubview(view)
            f.clip = clip
            view.flightLook = FlightLook(image: CGRect(origin: .zero, size: view.bounds.size), scale: 1)
        }
        // Ranked among the flights by its rank, a clip as the view it holds
        // (`PaintOrder`).
        PaintOrder.changed(layer)
    }

    private func showFlight(_ f: Flight) {
        guard let view = f.view, let slot = f.slot, let layer = f.container, let content = layer.superview else { return }
        let p = f.progress
        // Through the layers, as the source was: ancestors' transforms count.
        let from = layer.layer.flatMap { l in content.layer.map { l.convert(f.source.rect, from: $0) } } ?? layer.convert(f.source.rect, from: content)
        let to = slot.layer.flatMap { s in layer.layer.map { s.convert(s.bounds, to: $0) } } ?? slot.convert(slot.bounds, to: layer)
        func mix(_ a: CGFloat, _ b: CGFloat) -> CGFloat { a + (b - a) * p }
        let shown = NSRect(x: mix(from.minX, to.minX), y: mix(from.minY, to.minY),
                           width: max(0, mix(from.width, to.width)), height: max(0, mix(from.height, to.height)))
        CATransaction.begin(); CATransaction.setDisableActions(true)
        if let clip = f.clip {
            // Scaled whole at its own layout, the slot's size, in a clip that
            // is the shown box (D4.4). AppKit's geometry knows nothing of a
            // layer's scale, so the view keeps its frame at its own size at
            // the clip's top left and its layer is scaled about that origin.
            let layout = slot.bounds.size
            let s = FlightScale.of(shown: shown.size, layout: layout) ?? 1
            let ratio = to.width / max(layout.width, 1)
            clip.frame = shown
            clip.layer?.cornerRadius = mix(f.source.radius, (view.cornerRadii(in: NSRect(origin: .zero, size: layout)).max() ?? 0) * ratio)
            view.frame = NSRect(origin: .zero, size: layout)
            view.flightLook = FlightLook(image: CGRect(origin: .zero, size: layout), scale: s)
            view.layer?.setAffineTransform(CGAffineTransform(scaleX: s, y: s))
            CATransaction.commit()
            return
        }
        view.frame = shown
        view.layer?.cornerRadius = mix(f.source.radius, view.cornerRadii(in: NSRect(origin: .zero, size: to.size)).max() ?? 0)
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

    /// Every scroller between the slot and the window scrolls by the least
    /// that shows it whole, innermost first, at once (LLP 1013.000 D5).
    private func scrollIntoView(_ slot: NSView) {
        var inner: NSView = slot
        while let sv = inner.enclosingScrollView, let doc = sv.documentView {
            doc.scrollToVisible(doc.convert(slot.bounds, from: slot))
            inner = sv
        }
    }

    private static func dropEmptyLayer(_ layer: NSView) {
        if layer.subviews.isEmpty { layer.removeFromSuperview() }
    }

    /// Where a flying image is drawn in its shown box, at `progress`: the
    /// image moves in points from A's drawn rectangle to B's, as on iOS
    /// (`FlightsIOS.swift`).
    static func flightImage(from: CGSize, fit start: CGRect, to: CGSize, fit end: CGRect, progress p: CGFloat) -> CGRect {
        func mix(_ a: CGFloat, _ b: CGFloat) -> CGFloat { a + (b - a) * p }
        return CGRect(x: mix(start.minX * from.width, end.minX * to.width), y: mix(start.minY * from.height, end.minY * to.height),
                      width: mix(start.width * from.width, end.width * to.width), height: mix(start.height * from.height, end.height * to.height))
    }

    static func fitFraction(natural: CGSize, box: CGSize, fit: String) -> CGRect {
        guard box.width > 0, box.height > 0, natural.width > 0, natural.height > 0 else { return CGRect(x: 0, y: 0, width: 1, height: 1) }
        let r = RasterGeometry.rect(natural: natural, content: CGRect(origin: .zero, size: box), fit: fit)
        return CGRect(x: r.minX / box.width, y: r.minY / box.height, width: r.width / box.width, height: r.height / box.height)
    }
}
#endif
