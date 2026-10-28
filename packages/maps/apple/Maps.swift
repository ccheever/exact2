// @exact/maps on Apple (LLP 1133.021 D1-D5, §12): `exact-map` is an
// `MKMapView`, its markers MapKit's own `MKMarkerAnnotationView`s.
//
// - The camera is a centre and a Web Mercator zoom (D1): MapKit's world is
//   `MKMapSize.world.width` (2^28) map points wide, which is 256 × 2^20, so
//   at zoom z one point spans 2^(20 - z) map points.
// - A camera prop moves the map only when its value changes, and only that
//   prop: a new `zoom` zooms about the centre the map shows now (D1).
// - `moveend` is `mapView(_:regionDidChangeAnimated:)`, whoever moved the
//   map; `change` is a selection the person made, never one a prop asked for.
// - Under the agent (unless bound with `--bind maps`) the map's content is
//   replaced by plain tiles, an `MKTileOverlay` that `canReplaceMapContent`
//   (§12), so a screenshot is stable and offline; the markers stay MapKit's.
// - Each marker is a part named by its id (§9 item 3): the map answers
//   `exactPart:` with the marker's annotation view, where a drive's real
//   input lands.
// - The hatch (D5) is given the `MKMapView` after it is made and after each
//   props change; the module owns its delegate and the annotations it added.
import ExactPackage
import Foundation
import ImageIO
import MapKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public final class MapsModule: NSObject, ExactPackage {
    /// The app's access hatch (D5): the map, after it is made and after each
    /// props change.
    public static var hatch: ((MKMapView) -> Void)?

    let context: ExactPackageContext
    private var maps: [String: MapView] = [:]

    public init(context: ExactPackageContext) {
        self.context = context
        super.init()
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        reply(["error": "NotSupportedError"])
    }

    public func view(_ tag: String, id: String, props: [String: String], events: ExactPackageEvents) throws -> ExactPackageView {
        guard tag == "exact-map" else { throw ExactPackageRefusal("no view \(tag)") }
        let map = MapView(id: id, plain: context.agent, events: events, log: context.log)
        maps[id] = map
        try map.setProps(props)
        events.load()
        return map
    }

    public func destroy(_ done: @escaping () -> Void) {
        maps = [:]
        done()
    }
}

/// A marker the app gave: its id, the part's name.
final class Marker: MKPointAnnotation {
    let id: String
    init(id: String, title: String?, at coordinate: CLLocationCoordinate2D) {
        self.id = id
        super.init()
        self.title = title
        self.coordinate = coordinate
    }
}

/// The map view, answering a drive's parts.
final class PartedMap: MKMapView {
    var markers: () -> [Marker] = { [] }
    /// Told after each layout: a camera asked before the map had a size is placed then.
    var laidOut: () -> Void = {}
    #if os(macOS)
    override func layout() { super.layout(); laidOut() }
    #else
    override func layoutSubviews() { super.layoutSubviews(); laidOut() }
    #endif
    /// The annotation view of marker `name` (§9 item 3), the view a
    /// drive's tap aims at; nil when it is not on screen.
    @objc func exactPart(_ name: String) -> AnyObject? {
        guard let marker = markers().first(where: { $0.id == name }) else { return nil }
        return view(for: marker)
    }
    /// The marker whose view holds `at`, a point in the window: on macOS a
    /// click on a marker reaches the map view, not the marker's view, so a
    /// drive's tap asks the map which part it landed on.
    @objc func exactPartAt(_ at: NSValue) -> AnyObject? {
        #if os(macOS)
        let point = at.pointValue
        #else
        let point = at.cgPointValue
        #endif
        let hit = markers().first { marker in
            guard let v = view(for: marker), !v.isHidden else { return false }
            return v.bounds.contains(v.convert(point, from: nil))
        }
        return hit.map { $0.id as NSString }
    }
}

final class MapView: NSObject, ExactPackageView, MKMapViewDelegate {
    private let map = PartedMap(frame: .zero)
    private let id: String
    private let events: ExactPackageEvents
    private let log: (String) -> Void
    private var props: [String: String] = [:]
    /// The camera props asked for and not yet laid out: a map with no size
    /// yet cannot place a zoom.
    private var pending: (CLLocationCoordinate2D?, Double?)?
    private var applying = false
    private var first = true

    init(id: String, plain: Bool, events: ExactPackageEvents, log: @escaping (String) -> Void) {
        self.id = id; self.events = events; self.log = log
        super.init()
        map.delegate = self
        map.markers = { [weak self] in self?.markers ?? [] }
        map.laidOut = { [weak self] in
            guard let self, let (centre, zoom) = self.pending, self.map.bounds.width > 0 else { return }
            self.pending = nil
            self.move(centre, zoom, animated: false)
        }
        #if os(macOS)
        map.showsZoomControls = true
        #endif
        if plain {
            let tiles = PlainTiles()
            tiles.canReplaceMapContent = true
            map.addOverlay(tiles, level: .aboveLabels)
        }
        log("map \(id): \(plain ? "plain" : "live") tiles")
        MapsModule.hatch?(map)
    }

    var view: ExactPackagePlatformView { map }
    private var markers: [Marker] { map.annotations.compactMap { $0 as? Marker } }

    func setProps(_ next: [String: String]) throws {
        applying = true
        defer { applying = false }
        let changed = { (k: String) in self.first || next[k] != self.props[k] }
        var centre: CLLocationCoordinate2D?
        if changed("latitude") || changed("longitude"), let lat = Double(next["latitude"] ?? ""), let lon = Double(next["longitude"] ?? "") {
            centre = CLLocationCoordinate2D(latitude: lat, longitude: lon)
        }
        var zoom: Double?
        if changed("zoomLevel") { zoom = Double(next["zoomLevel"] ?? "") ?? (first ? 12 : nil) }
        if centre != nil || zoom != nil { move(centre, zoom, animated: !first) }
        if changed("markers") {
            map.removeAnnotations(markers)
            map.addAnnotations(Self.markers(next["markers"] ?? ""))
        }
        let selected = next["selected"] ?? ""
        if changed("selected") || changed("markers") {
            if let m = markers.first(where: { $0.id == selected }) {
                map.selectAnnotation(m, animated: !first)
            } else {
                for a in map.selectedAnnotations { map.deselectAnnotation(a, animated: false) }
            }
        }
        if let label = next["aria-label"] {
            #if os(macOS)
            map.setAccessibilityLabel(label)
            #else
            map.accessibilityLabel = label
            #endif
        }
        props = next
        first = false
        MapsModule.hatch?(map)
    }

    private static func markers(_ text: String) -> [Marker] {
        guard let data = text.data(using: .utf8), let list = (try? JSONSerialization.jsonObject(with: data)) as? [[String: Any]] else { return [] }
        return list.compactMap { m in
            guard let id = m["id"] as? String, let lat = m["latitude"] as? Double, let lon = m["longitude"] as? Double else { return nil }
            return Marker(id: id, title: m["title"] as? String, at: CLLocationCoordinate2D(latitude: lat, longitude: lon))
        }
    }

    // MARK: the camera (D1)

    /// Map points per screen point at zoom `z`.
    private static func scale(_ z: Double) -> Double { pow(2, 20 - z) }

    private var zoom: Double {
        let width = Double(map.bounds.width)
        guard width > 0, map.visibleMapRect.size.width > 0 else { return 12 }
        return 20 - log2(map.visibleMapRect.size.width / width)
    }

    /// Move to `centre` (or the current one) at `zoom` (or the current one).
    private func move(_ centre: CLLocationCoordinate2D?, _ zoom: Double?, animated: Bool) {
        let size = map.bounds.size
        guard size.width > 0, size.height > 0 else {
            pending = (centre ?? pending?.0, zoom ?? pending?.1)
            return
        }
        let at = MKMapPoint(centre ?? map.centerCoordinate)
        let k = Self.scale(zoom ?? self.zoom)
        let w = Double(size.width) * k, h = Double(size.height) * k
        map.setVisibleMapRect(MKMapRect(x: at.x - w / 2, y: at.y - h / 2, width: w, height: h), animated: animated)
    }

    private func camera() -> [String: Any] {
        let c = map.centerCoordinate
        let round = { (n: Double, places: Double) in (n * pow(10, places)).rounded() / pow(10, places) }
        return ["latitude": round(c.latitude, 6), "longitude": round(c.longitude, 6), "zoom": round(zoom, 3),
                "apple": ["heading": round(map.camera.heading, 2), "pitch": round(Double(map.camera.pitch), 2)]]
    }

    // MARK: MKMapViewDelegate

    func mapView(_ mapView: MKMapView, regionDidChangeAnimated animated: Bool) {
        guard case .none = pending, map.bounds.width > 0 else { return }
        let c = camera()
        log("map \(id): moveend \(c["latitude"]!) \(c["longitude"]!) \(c["zoom"]!)")
        events.emit("moveend", [c])
    }

    func mapView(_ mapView: MKMapView, didSelect view: MKAnnotationView) {
        guard !applying, let marker = view.annotation as? Marker, marker.id != props["selected"] else { return }
        log("map \(id): selected \(marker.id)")
        props["selected"] = marker.id
        events.change(marker.id)
    }

    func mapView(_ mapView: MKMapView, didDeselect view: MKAnnotationView) {
        guard !applying, view.annotation is Marker, !(props["selected"] ?? "").isEmpty else { return }
        DispatchQueue.main.async { [self] in
            // A deselection followed by another marker's selection is that selection.
            guard map.selectedAnnotations.isEmpty, !(props["selected"] ?? "").isEmpty else { return }
            log("map \(id): selected none")
            props["selected"] = ""
            events.change("")
        }
    }

    func mapView(_ mapView: MKMapView, viewFor annotation: MKAnnotation) -> MKAnnotationView? {
        guard let marker = annotation as? Marker else { return nil }
        let view = mapView.dequeueReusableAnnotationView(withIdentifier: "marker") as? MKMarkerAnnotationView
            ?? MKMarkerAnnotationView(annotation: marker, reuseIdentifier: "marker")
        view.annotation = marker
        view.canShowCallout = false
        #if os(macOS)
        view.setAccessibilityLabel(marker.title ?? marker.id)
        #else
        view.accessibilityLabel = marker.title ?? marker.id
        #endif
        return view
    }

    func mapView(_ mapView: MKMapView, rendererFor overlay: MKOverlay) -> MKOverlayRenderer {
        if let tiles = overlay as? MKTileOverlay { return MKTileOverlayRenderer(tileOverlay: tiles) }
        return MKOverlayRenderer(overlay: overlay)
    }

    func destroy() {
        map.delegate = nil
        map.removeAnnotations(map.annotations)
        map.removeOverlays(map.overlays)
    }
}

/// §12's plain tiles: a flat colour with each tile's edges, the same at
/// every zoom, so the map's content is stable and nothing is fetched.
final class PlainTiles: MKTileOverlay {
    private static let png: Data = {
        let side = 256
        let space = CGColorSpaceCreateDeviceRGB()
        guard let cg = CGContext(data: nil, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4, space: space,
                                 bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return Data() }
        cg.setFillColor(CGColor(red: 0.91, green: 0.925, blue: 0.937, alpha: 1))
        cg.fill(CGRect(x: 0, y: 0, width: side, height: side))
        cg.setFillColor(CGColor(red: 0.788, green: 0.82, blue: 0.847, alpha: 1))
        cg.fill(CGRect(x: side - 1, y: 0, width: 1, height: side))
        cg.fill(CGRect(x: 0, y: 0, width: side, height: 1))
        guard let image = cg.makeImage() else { return Data() }
        let out = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(out, "public.png" as CFString, 1, nil) else { return Data() }
        CGImageDestinationAddImage(dest, image, nil)
        CGImageDestinationFinalize(dest)
        return out as Data
    }()

    override func loadTile(at path: MKTileOverlayPath, result: @escaping (Data?, Error?) -> Void) {
        result(Self.png, nil)
    }
}
