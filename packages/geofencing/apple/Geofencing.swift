// @exact/geofencing on Apple (LLP 1133.021 D10-D14): the regions are the
// host's region carrier's (`context.regions`, a `CLMonitor` the host makes
// again at launch), and the module asks for Always and reads it back.
//
// - Ask (D13): one call, `requestAlwaysAuthorization()`, whatever the When
//   In Use state; the request is remembered in the defaults, since a second
//   call does nothing, and When In Use after it reads `denied`.
// - Twenty regions an app (D11): the twenty-first fails `QuotaExceededError`;
//   a radius under 100 m is raised to 100 with a journal line.
// - A crossing reaches the module as a wake of kind `region` whose keys are
//   the regions crossed: each undelivered transition is raised, one at a
//   time, as `geofenceenter` or `geofenceleave`, and is forgotten only when
//   its invocation completes (D14 step 3).
import CoreLocation
import ExactPackage
import Foundation

public final class GeofencingModule: NSObject, ExactPackage, CLLocationManagerDelegate {
    private let context: ExactPackageContext
    private lazy var manager: CLLocationManager = {
        let m = CLLocationManager()
        m.delegate = self
        return m
    }()
    /// What waits on the person's answer to the Always prompt.
    private var authorizing: [() -> Void] = []
    private var said: (CLAuthorizationStatus, CLAccuracyAuthorization)?
    private static let requestedKey = "exact.geofencing.alwaysRequested"
    private var alwaysRequested: Bool {
        get { UserDefaults.standard.bool(forKey: Self.requestedKey) }
        set { UserDefaults.standard.set(newValue, forKey: Self.requestedKey) }
    }

    public init(context: ExactPackageContext) {
        self.context = context
        super.init()
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            switch name {
            case "geofencingStatus": reply(status())
            case "geofences": reply(["exact:answer": context.regions.list()])
            case "addGeofence": whenAlways(reply) { self.add(args, reply) }
            case "removeGeofence":
                context.regions.remove(name: args["name"] as? String ?? "")
                context.changed("geofencing.geofences")
                reply([:])
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    // MARK: permission (D13)

    private var authorization: CLAuthorizationStatus { manager.authorizationStatus }

    private func state() -> String {
        switch authorization {
        case .notDetermined: return "prompt"
        #if os(iOS)
        case .authorizedWhenInUse: return alwaysRequested ? "denied" : "prompt"
        #endif
        case .authorizedAlways: return manager.accuracyAuthorization == .fullAccuracy ? "granted" : "denied"
        default: return "denied"
        }
    }

    private func status() -> [String: Any] {
        let names: [CLAuthorizationStatus: String] = [.notDetermined: "notDetermined", .restricted: "restricted", .denied: "denied", .authorizedAlways: "authorizedAlways"]
        let authorizationName = names[authorization] ?? "authorizedWhenInUse"
        return ["available": CLLocationManager.isMonitoringAvailable(for: CLCircularRegion.self), "state": state(),
                "apple": ["authorization": authorizationName,
                          "accuracyAuthorization": manager.accuracyAuthorization == .fullAccuracy ? "fullAccuracy" : "reducedAccuracy",
                          "alwaysRequested": alwaysRequested]]
    }

    private func whenAlways(_ reply: @escaping ([String: Any]) -> Void, then: @escaping () -> Void) {
        switch state() {
        case "granted": then()
        case "prompt":
            authorizing.append { [self] in state() == "granted" ? then() : reply(["error": "PERMISSION_DENIED"]) }
            if authorizing.count == 1 {
                alwaysRequested = true
                manager.requestAlwaysAuthorization()
                // From When In Use the answer may be no change at all (a
                // prompt deferred, or none shown): read again when the app
                // is next active rather than wait forever.
                DispatchQueue.main.asyncAfter(deadline: .now() + 30) { [weak self] in self?.settle() }
            }
        default: reply(["error": "PERMISSION_DENIED"])
        }
    }

    private func settle() {
        let waiting = authorizing
        authorizing = []
        waiting.forEach { $0() }
    }

    public func locationManagerDidChangeAuthorization(_ m: CLLocationManager) {
        let now = (authorization, m.accuracyAuthorization)
        if let said, said != now { context.changed("geofencing.geofencingStatus") }
        said = now
        if authorization != .notDetermined, !authorizing.isEmpty, state() != "prompt" || alwaysRequested { settle() }
    }

    // MARK: regions (D10, D11)

    private func add(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let name = args["name"] as? String ?? ""
        let latitude = args["latitude"] as? Double ?? 0, longitude = args["longitude"] as? Double ?? 0
        var radius = args["radius"] as? Double ?? 100
        if radius < 100 {
            context.log("addGeofence \(name): radius \(radius) raised to 100 (LLP 1133.021 D11)")
            radius = 100
        }
        let others = context.regions.list().filter { $0["name"] as? String != name }
        guard others.count < 20 else { return reply(["error": "QuotaExceededError"]) }
        if let error = context.regions.add(name: name, latitude: latitude, longitude: longitude, radius: radius) {
            return reply(["error": error])
        }
        context.changed("geofencing.geofences")
        reply(["name": name, "latitude": latitude, "longitude": longitude, "radius": radius, "inside": NSNull()])
    }

    // MARK: crossings (D14)

    public func woke(_ wake: ExactWake) {
        guard wake.kind == "region" else { return wake.done(success: true) }
        deliver(wake.keys[...], wake)
    }

    /// The transitions of `names`, raised one at a time.
    private func deliver(_ names: ArraySlice<String>, _ wake: ExactWake) {
        guard let name = names.first else {
            context.changed("geofencing.geofences")
            return wake.done(success: true)
        }
        let region = context.regions.list().first { $0["name"] as? String == name }
        guard let region, let pending = context.regions.pending(name: name), let kind = pending["event"] as? String else {
            return deliver(names.dropFirst(), wake)
        }
        var geofence = region
        geofence["inside"] = kind == "enter"
        let event: [String: Any] = ["geofence": geofence, "timestamp": pending["timestamp"] ?? 0]
        wake.raise(kind == "enter" ? "geofenceenter" : "geofenceleave", [event]) { [self] outcome in
            context.log("geofencing: \(name) \(kind) \(outcome)")
            if outcome == "completed" || outcome == "unheard" { context.regions.delivered(name: name) }
            deliver(names.dropFirst(), wake)
        }
    }
}
