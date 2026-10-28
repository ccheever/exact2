// The region carrier (LLP 1133.021 D14): one `CLMonitor` named
// `<bundle id>.regions` and the store of regions and their undelivered
// transitions. On iOS a relaunch for a region event must "recreate the
// monitor with the same identifier" before anything else, which is before a
// package's Swift loads with the app's module, so the carrier is the
// host's, made at install when the store holds a region, and a package
// reaches it through `context.regions`.
//
// The module side reaches it through `NotificationCenter` (`exact.regions`,
// an `NSMutableDictionary` whose `reply` the carrier sets): `{op: add,
// package, name, latitude, longitude, radius}`, `{op: remove, package,
// name}`, `{op: list, package}`, `{op: pending, package, name}`, `{op:
// delivered, package, name}`.
//
// The store is the truth (D14): a crossing is written to it first, the
// region's `inside` set and its one undelivered transition replaced, then
// offered to the package that added it as a wake of kind `region`, its keys
// the regions' names. A transition stays until the package says it was
// delivered, and is offered again whenever a module becomes ready, which a
// later launch's is after its first pixel.
import Foundation
#if os(iOS) || os(macOS)
import CoreLocation
#endif
#if os(iOS)
import UIKit
#endif

public enum RegionCarrier {
    struct Region: Codable {
        var package: String, name: String, latitude: Double, longitude: Double, radius: Double
        var inside: Bool?
        /// The undelivered transition, the latest: `enter` or `leave`, and when.
        var pending: String?
        var at: Double?
    }

    private static var regions: [String: Region] = [:]
    private static var installed = false
    /// Names in a wake now, not offered again until it ends.
    private static var offering: Set<String> = []

    /// Under the app's support directory, not purgeable; on the Mac under its bundle id.
    static let file: URL = {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        #if os(macOS)
        let dir = support.appendingPathComponent(Bundle.main.bundleIdentifier ?? "app").appendingPathComponent("regions")
        #else
        let dir = support.appendingPathComponent("exact-regions")
        #endif
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("store.json")
    }()

    private static func keep() {
        if let data = try? JSONEncoder().encode(Array(regions.values)) { try? data.write(to: file, options: .atomic) }
    }

    /// At launch: the store read back, and the monitor made again if it holds a region.
    static func install() {
        guard !installed else { return }
        installed = true
        if let data = try? Data(contentsOf: file), let kept = try? JSONDecoder().decode([Region].self, from: data) {
            regions = Dictionary(kept.map { ($0.name, $0) }, uniquingKeysWith: { $1 })
        }
        NotificationCenter.default.addObserver(forName: Notification.Name("exact.regions"), object: nil, queue: nil) { note in
            guard let box = note.object as? NSMutableDictionary else { return }
            let run = { box["reply"] = handle(box as? [String: Any] ?? [:]) }
            if Thread.isMainThread { run() } else { DispatchQueue.main.sync(execute: run) }
        }
        #if os(iOS) || os(macOS)
        if !regions.isEmpty { Monitor.start(regions.values.map { $0 }) }
        #endif
    }

    private static func handle(_ body: [String: Any]) -> Any? {
        let package = body["package"] as? String ?? "", name = body["name"] as? String ?? ""
        switch body["op"] as? String {
        case "add":
            #if os(iOS) || os(macOS)
            let r = Region(package: package, name: name, latitude: body["latitude"] as? Double ?? 0, longitude: body["longitude"] as? Double ?? 0,
                           radius: body["radius"] as? Double ?? 100)
            regions[name] = r
            keep()
            WakeStore.log("regions: added \(name)")
            Monitor.add(r)
            return nil
            #else
            return "NotSupportedError"
            #endif
        case "remove":
            guard regions.removeValue(forKey: name) != nil else { return nil }
            keep()
            WakeStore.log("regions: removed \(name)")
            #if os(iOS) || os(macOS)
            Monitor.remove(name)
            #endif
            return nil
        case "list":
            return regions.values.filter { $0.package == package }.sorted { $0.name < $1.name }.map { r -> [String: Any] in
                var out: [String: Any] = ["name": r.name, "latitude": r.latitude, "longitude": r.longitude, "radius": r.radius]
                if let inside = r.inside { out["inside"] = inside }
                return out
            }
        case "pending":
            guard let r = regions[name], r.package == package, let event = r.pending else { return nil }
            return ["event": event, "timestamp": r.at ?? 0]
        case "delivered":
            regions[name]?.pending = nil
            regions[name]?.at = nil
            keep()
            return nil
        default:
            return nil
        }
    }

    /// The platform reported region `name` `inside` or not at `at` (ms):
    /// written to the store, then offered as a wake.
    static func crossed(_ name: String, inside: Bool, at: Double) {
        guard var r = regions[name], r.inside != inside else { return }
        r.inside = inside
        r.pending = inside ? "enter" : "leave"
        r.at = at
        regions[name] = r
        keep()
        WakeStore.log("regions: \(name) \(inside ? "entered" : "left")")
        offer([name])
    }

    /// A module is ready: the transitions kept undelivered are offered.
    static func ready() {
        offer(regions.values.filter { $0.pending != nil }.map(\.name))
    }

    private static func offer(_ names: [String]) {
        let fresh = names.filter { !offering.contains($0) }
        guard !fresh.isEmpty else { return }
        offering.formUnion(fresh)
        #if os(iOS)
        // Its time bounds the wake (D14 step 1).
        var task = UIBackgroundTaskIdentifier.invalid
        task = UIApplication.shared.beginBackgroundTask(withName: "exact.regions") {
            UIApplication.shared.endBackgroundTask(task)
            task = .invalid
        }
        #endif
        WakeStore.region(fresh.compactMap { n in regions[n].map { ($0.package, n) } }) { _ in
            offering.subtract(fresh)
            #if os(iOS)
            if task != .invalid { UIApplication.shared.endBackgroundTask(task) }
            #endif
        }
    }

    #if os(iOS) || os(macOS)
    /// The monitor and, from iOS 18, the service session that keeps Always
    /// in force while the app runs in the background.
    private enum Monitor {
        nonisolated(unsafe) static var monitor: CLMonitor?
        nonisolated(unsafe) static var session: AnyObject?
        nonisolated(unsafe) static var starting: Task<CLMonitor, Never>?

        private static var name: String { (Bundle.main.bundleIdentifier ?? "app") + ".regions" }

        /// Made once; its events iterated for the life of the process. The
        /// store's regions the monitor lacks are added, and only those, since
        /// adding a condition again starts its state from `unknown`.
        static func start(_ kept: [Region]) {
            guard starting == nil else { return }
            #if os(iOS)
            if #available(iOS 18, *) { session = CLServiceSession(authorization: .always) }
            #endif
            starting = Task { @MainActor in
                let m = await CLMonitor(name)
                monitor = m
                let have = Set(await m.identifiers)
                for r in kept where !have.contains(r.name) { await m.add(condition(r), identifier: r.name) }
                Task { @MainActor in
                    do {
                        for try await event in await m.events {
                            let at = event.date.timeIntervalSince1970 * 1000
                            switch event.state {
                            case .satisfied: RegionCarrier.crossed(event.identifier, inside: true, at: at)
                            case .unsatisfied: RegionCarrier.crossed(event.identifier, inside: false, at: at)
                            default: break
                            }
                        }
                    } catch {
                        WakeStore.log("regions: the monitor's events ended: \(error)")
                    }
                }
                WakeStore.log("regions: monitoring \(kept.count)")
                return m
            }
        }

        static func condition(_ r: Region) -> CLMonitor.CircularGeographicCondition {
            CLMonitor.CircularGeographicCondition(center: CLLocationCoordinate2D(latitude: r.latitude, longitude: r.longitude), radius: r.radius)
        }

        static func add(_ r: Region) {
            if starting == nil { return start([r]) }
            Task { @MainActor in
                let m = await starting!.value
                await m.remove(r.name)
                await m.add(condition(r), identifier: r.name)
            }
        }

        static func remove(_ name: String) {
            guard let starting else { return }
            Task { @MainActor in await starting.value.remove(name) }
        }
    }
    #endif
}
