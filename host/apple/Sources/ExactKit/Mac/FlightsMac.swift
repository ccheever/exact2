// Shared-element flights on AppKit (LLP 1013.000 D4, D5): as on UIKit
// (`FlightsIOS.swift`), in the window's content view. The leaver is captured
// before the batch's destroys; once it is applied the arriver's place is
// scrolled into view, the live arriver is lifted into a pass-through layer
// over the content, shown each frame between the captured rectangle and its
// slot (re-read every frame), with its radius and an image's crop
// interpolated, and put back at `land`.
#if os(macOS)
import AppKit

/// The look an arriver shows while it flies: where its image is drawn,
/// in its own bounds (`applyImageLayer` defers to it).
struct FlightLook {
    var image: CGRect
}

/// Where a leaver was shown when its name moved on.
struct FlightSource {
    var rect: NSRect // the window's content view coordinates
    var radius: CGFloat
    /// An image's fitted rectangle as a fraction of its box.
    var fit: CGRect?
    var natural: CGSize?
}

final class Flight {
    let id: UInt32
    let source: FlightSource
    var progress: CGFloat = 0
    weak var view: NodeView?
    var slot: NSView?
    var container: NSView?
    var frame: NSRect?
    var saved: (radius: CGFloat, clips: Bool)?
    init(id: UInt32, source: FlightSource) { self.id = id; self.source = source }
}

/// The flight layer: draws its flights, takes no clicks.
final class FlightLayer: NSView {
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
        var source = FlightSource(rect: leaver.convert(leaver.bounds, to: content), radius: leaver.cornerRadii(in: leaver.bounds).max() ?? 0)
        if let flying = flights.values.first(where: { $0.view === leaver }), let look = leaver.flightLook {
            let b = leaver.bounds
            source.fit = CGRect(x: look.image.minX / max(b.width, 1), y: look.image.minY / max(b.height, 1),
                                width: look.image.width / max(b.width, 1), height: look.image.height / max(b.height, 1))
            source.natural = flying.source.natural ?? leaver.raster?.image.naturalSize
            source.radius = leaver.layer?.cornerRadius ?? 0
        } else if leaver.kind == "image", let natural = leaver.raster?.image.naturalSize {
            source.fit = Self.fitFraction(natural: natural, box: leaver.bounds.size, fit: leaver.style["object_fit"]?.string ?? "fill")
            source.natural = natural
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
        for f in flights.values {
            if f.view == nil, f.slot == nil { liftFlight(f) }
            showFlight(f)
        }
    }

    func landFlight(_ f: Flight) {
        flights.removeValue(forKey: f.id)
        guard let view = f.view, let slot = f.slot, let parent = slot.superview else {
            f.slot?.removeFromSuperview(); f.container.map(Self.dropEmptyLayer); return
        }
        view.flightLook = nil
        view.removeFromSuperview()
        parent.addSubview(view, positioned: .above, relativeTo: slot)
        slot.removeFromSuperview()
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
        f.slot?.removeFromSuperview()
        f.container.map(Self.dropEmptyLayer)
    }

    private func liftFlight(_ f: Flight) {
        guard let view = views[f.id], let parent = view.superview, let content = view.window?.contentView, !f.source.rect.isNull,
              !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion else { flights.removeValue(forKey: f.id); return }
        let slot = FlightSlot(frame: view.frame)
        slot.isHidden = true
        parent.addSubview(slot, positioned: .below, relativeTo: view)
        scrollIntoView(slot)
        let layer = content.subviews.last as? FlightLayer ?? {
            let l = FlightLayer(frame: content.bounds)
            l.autoresizingMask = [.width, .height]
            content.addSubview(l)
            return l
        }()
        f.saved = (view.layer?.cornerRadius ?? 0, view.layer?.masksToBounds ?? false)
        f.view = view
        f.slot = slot
        f.container = layer
        view.removeFromSuperview()
        layer.addSubview(view)
        view.layer?.masksToBounds = true
        view.flightLook = FlightLook(image: CGRect(origin: .zero, size: view.bounds.size))
    }

    private func showFlight(_ f: Flight) {
        guard let view = f.view, let slot = f.slot, let layer = f.container, let content = layer.superview else { return }
        let p = f.progress
        let from = layer.convert(f.source.rect, from: content)
        let to = slot.convert(slot.bounds, to: layer)
        func mix(_ a: CGFloat, _ b: CGFloat) -> CGFloat { a + (b - a) * p }
        let shown = NSRect(x: mix(from.minX, to.minX), y: mix(from.minY, to.minY),
                           width: max(0, mix(from.width, to.width)), height: max(0, mix(from.height, to.height)))
        CATransaction.begin(); CATransaction.setDisableActions(true)
        view.frame = shown
        view.layer?.cornerRadius = mix(f.source.radius, view.cornerRadii(in: NSRect(origin: .zero, size: to.size)).max() ?? 0)
        if view.kind == "image" {
            let natural = view.raster?.image.naturalSize ?? f.source.natural ?? .zero
            let end = Self.fitFraction(natural: natural, box: to.size, fit: view.style["object_fit"]?.string ?? "fill")
            let start = f.source.fit ?? end
            let unit = CGRect(x: mix(start.minX, end.minX), y: mix(start.minY, end.minY),
                              width: mix(start.width, end.width), height: mix(start.height, end.height))
            view.flightLook = FlightLook(image: CGRect(x: unit.minX * shown.width, y: unit.minY * shown.height,
                                                       width: unit.width * shown.width, height: unit.height * shown.height))
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

    static func fitFraction(natural: CGSize, box: CGSize, fit: String) -> CGRect {
        guard box.width > 0, box.height > 0, natural.width > 0, natural.height > 0 else { return CGRect(x: 0, y: 0, width: 1, height: 1) }
        let r = RasterGeometry.rect(natural: natural, content: CGRect(origin: .zero, size: box), fit: fit)
        return CGRect(x: r.minX / box.width, y: r.minY / box.height, width: r.width / box.width, height: r.height / box.height)
    }
}
#endif
