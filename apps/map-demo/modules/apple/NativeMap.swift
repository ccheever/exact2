// `<native-map>` on Apple hosts (LLP 1024): MKMapView, the platform's own
// map and gestures. Props: latitude, longitude, span (degrees of latitude in
// view), pins (`id|lat|lon|title;…`), selected (an id). Events: `change`
// with the id of a pin the person selected on the map. A `selected` prop
// selects and centres without an event. MapKit draws with Metal, which the
// host's ordinary capture cannot see, so the tag answers snapshots
// (MKMapSnapshotter, the pins drawn on top).
//
// The tag opts into reuse in a list (LLP 1068 §4.8): creating an MKMapView
// costs 20–47 ms of main thread and its churn retains memory, so a retired
// row's map is reset and lent to the next map row. The reset leaves nothing
// a new map would not have — no pins, selection, camera, map type, user
// location or interaction flags of the last row — and the next props are a
// first mount, and `load` follows it. The reset empties MapKit's Metal layer,
// so until the new region draws the map shows its own blank ground, as a new
// map does, never the last row's tiles.
import Foundation
import MapKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class MapModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["native-map": ExactNativeFactory(snapshot: true, reuse: true) { props, events in NativeMap(props: props, events: events) }]
    }
}
let exactModule: ExactModule.Type = MapModule.self

private final class Pin: MKPointAnnotation {
    let id: String
    init(id: String, title: String, at coordinate: CLLocationCoordinate2D) {
        self.id = id
        super.init()
        self.title = title
        self.coordinate = coordinate
    }
}

/// MapKit's delegate is an NSObject; the instance is not.
private final class MapDelegate: NSObject, MKMapViewDelegate {
    var selected: ((MKAnnotation) -> Void)?
    var rendered: (() -> Void)?
    func mapView(_ mapView: MKMapView, didSelect view: MKAnnotationView) { if let a = view.annotation { selected?(a) } }
    func mapViewDidFinishRenderingMap(_ mapView: MKMapView, fullyRendered: Bool) { rendered?() }
}

final class NativeMap: ExactNativeInstance {
    private let map = MKMapView(frame: .zero)
    private let delegate = MapDelegate()
    private var props: [String: String] = [:]
    private var applying = false
    /// Reset for reuse: the next props are a first mount (LLP 1068 §4.8).
    private var fresh = false
    /// Reused and set to its new region: `load` waits for the first draw of it.
    private var awaiting = 0

    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        delegate.selected = { [weak self] annotation in self?.didSelect(annotation) }
        delegate.rendered = { [weak self] in self?.renderedAfterReuse() }
        map.delegate = delegate
        #if os(macOS)
        map.showsZoomControls = true
        map.setAccessibilityLabel("Map of stores")
        #else
        map.accessibilityLabel = "Map of stores"
        #endif
        apply(props, first: true)
        events.load()
    }

    override var view: ExactNativeView { map }

    override func setProps(_ props: [String: String]) throws {
        guard fresh else { return apply(props, first: false) }
        fresh = false
        apply(props, first: true)
        // The host keeps a reused view transparent until `load`, so the last
        // row's tiles never show; `load` comes with the new region's first
        // draw, or after half a second if MapKit reports none.
        awaiting += 1
        let token = awaiting
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
            guard let self, self.awaiting == token else { return }
            self.renderedAfterReuse()
        }
    }

    private func renderedAfterReuse() {
        guard awaiting != 0 else { return }
        awaiting = 0
        events.load()
    }

    /// As if created with no props: nothing of the last row's map stays.
    override func prepareForReuse() throws {
        applying = true
        defer { applying = false }
        for a in map.selectedAnnotations { map.deselectAnnotation(a, animated: false) }
        map.removeAnnotations(map.annotations)
        map.removeOverlays(map.overlays)
        map.showsUserLocation = false
        #if !os(macOS)
        if map.userTrackingMode != .none { map.setUserTrackingMode(.none, animated: false) }
        #endif
        map.mapType = .standard
        map.isScrollEnabled = true; map.isZoomEnabled = true; map.isRotateEnabled = true; map.isPitchEnabled = true
        #if os(macOS)
        map.setAccessibilityLabel("Map of stores")
        #else
        map.isUserInteractionEnabled = true
        map.accessibilityLabel = "Map of stores"
        #endif
        let camera = map.camera.copy() as! MKMapCamera
        camera.heading = 0; camera.pitch = 0
        map.setCamera(camera, animated: false)
        props = [:]
        fresh = true
    }

    private func apply(_ next: [String: String], first: Bool) {
        applying = true
        defer { applying = false }
        if first || next["latitude"] != props["latitude"] || next["longitude"] != props["longitude"] || next["span"] != props["span"] {
            let centre = CLLocationCoordinate2D(latitude: Double(next["latitude"] ?? "") ?? 0, longitude: Double(next["longitude"] ?? "") ?? 0)
            let span = Double(next["span"] ?? "") ?? 0.05
            map.setRegion(MKCoordinateRegion(center: centre, span: MKCoordinateSpan(latitudeDelta: span, longitudeDelta: span)), animated: !first)
        }
        if first || next["pins"] != props["pins"] {
            map.removeAnnotations(map.annotations)
            map.addAnnotations(pins(next["pins"] ?? ""))
        }
        let selected = next["selected"] ?? ""
        if first || selected != props["selected"] {
            if let pin = map.annotations.compactMap({ $0 as? Pin }).first(where: { $0.id == selected }) {
                map.selectAnnotation(pin, animated: !first)
                map.setCenter(pin.coordinate, animated: !first)
            } else {
                for a in map.selectedAnnotations { map.deselectAnnotation(a, animated: false) }
            }
        }
        props = next
    }

    private func pins(_ text: String) -> [Pin] {
        text.split(separator: ";").compactMap { row in
            let f = row.split(separator: "|", omittingEmptySubsequences: false).map(String.init)
            guard f.count >= 3, let lat = Double(f[1]), let lon = Double(f[2]) else { return nil }
            return Pin(id: f[0], title: f.count > 3 ? f[3] : f[0], at: CLLocationCoordinate2D(latitude: lat, longitude: lon))
        }
    }

    // A selection the person made — not one a prop asked for — is the event.
    private func didSelect(_ annotation: MKAnnotation) {
        guard !applying, let pin = annotation as? Pin, pin.id != props["selected"] else { return }
        events.change(pin.id)
    }

    override func snapshot() throws -> Data {
        let size = map.bounds.size
        guard size.width > 0, size.height > 0 else { throw ExactNativeRefusal("no bounds yet") }
        let options = MKMapSnapshotter.Options()
        options.region = map.region
        options.size = size
        let done = DispatchSemaphore(value: 0)
        var result: MKMapSnapshotter.Snapshot?
        var failure: Error?
        // The completion comes on a background queue: this call may wait for it.
        MKMapSnapshotter(options: options).start(with: .global()) { snap, error in
            result = snap; failure = error; done.signal()
        }
        guard done.wait(timeout: .now() + 4) == .success, let snap = result else {
            throw ExactNativeRefusal("map snapshot failed: \(failure.map { String(describing: $0) } ?? "timed out")")
        }
        let selected = props["selected"] ?? ""
        #if os(macOS)
        let image = NSImage(size: size)
        image.lockFocus()
        snap.image.draw(in: CGRect(origin: .zero, size: size))
        for pin in map.annotations.compactMap({ $0 as? Pin }) {
            let p = snap.point(for: pin.coordinate)
            let dot = CGRect(x: p.x - 7, y: p.y - 7, width: 14, height: 14)
            (pin.id == selected ? NSColor.systemRed : NSColor.systemBlue).setFill()
            NSBezierPath(ovalIn: dot).fill()
            NSColor.white.setStroke()
            let ring = NSBezierPath(ovalIn: dot); ring.lineWidth = 2; ring.stroke()
        }
        image.unlockFocus()
        guard let tiff = image.tiffRepresentation, let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else {
            throw ExactNativeRefusal("no PNG")
        }
        return png
        #else
        let png = UIGraphicsImageRenderer(size: size).pngData { _ in
            snap.image.draw(in: CGRect(origin: .zero, size: size))
            for pin in map.annotations.compactMap({ $0 as? Pin }) {
                let p = snap.point(for: pin.coordinate)
                let dot = UIBezierPath(ovalIn: CGRect(x: p.x - 7, y: p.y - 7, width: 14, height: 14))
                (pin.id == selected ? UIColor.systemRed : UIColor.systemBlue).setFill(); dot.fill()
                UIColor.white.setStroke(); dot.lineWidth = 2; dot.stroke()
            }
        }
        return png
        #endif
    }
}
