// @exact/geolocation on Apple (LLP 1133.002 D3, D5, D10, D15): CoreLocation
// through one `CLLocationManager`, answered in the interface's shapes. The
// app's composed module calls it outside the agent, through `ExactPackage`'s
// shape; under the agent the runner's substitute answers and no manager is
// created, unless the drive binds it (`--bind geolocation`).
//
// - An option the call omits stays CoreLocation's (D2): `desiredAccuracy`
//   is set only from `apple.desiredAccuracy` or a written
//   `enableHighAccuracy` (WebKit's mapping: `Best`, `HundredMeters`).
// - The approximate line is a ceiling (D7): iOS 27 opens the prompt with
//   Precise on despite the line's `NSLocationDefaultAccuracyReduced`, so
//   under the line every request asks for `kCLLocationAccuracyReduced`,
//   which CoreLocation honours whatever the person granted.
// - The state is a fact (D8): `authorizationStatus` and
//   `accuracyAuthorization`, announced when they change.
// - A request while `notDetermined` asks first (D9); a source asks only for
//   `watchPosition` (`request: when-prompt`).
// - One watch (`startUpdatingLocation`) serves every reader; it stops at
//   `release`, when the runner's last reader goes (D5). A one-shot during a
//   watch takes the next update, since `requestLocation` does nothing then.
// - Tier 3 (D4): `GeolocationModule.hatch` receives the manager once made,
//   and the delegate it returns hears every delegate call, after the
//   module's own handling of the three it handles. The module owns the
//   delegate and, while a reader runs, the options it writes; a development
//   build names a change to them (`log`).
import CoreLocation
import ExactPackage
import Foundation

public final class GeolocationModule: NSObject, CLLocationManagerDelegate, ExactPackage {
    /// The app's access hatch (D4): given the manager once, before its first
    /// request; a delegate returned hears every delegate call after the module.
    public static var hatch: ((CLLocationManager) -> CLLocationManagerDelegate?)?

    private let changed: (String) -> Void
    private let log: (String) -> Void
    private var forward: CLLocationManagerDelegate?
    /// What `configure` last wrote, to name a hatch's change to it.
    private var wrote: (accuracy: CLLocationAccuracy, filter: CLLocationDistance, activity: CLActivityType)?
    private lazy var manager: CLLocationManager = {
        let m = CLLocationManager()
        // The hatch first: a delegate's answers to `responds(to:)` may be read when it is set.
        forward = Self.hatch?(m)
        m.delegate = self
        return m
    }()
    /// What waits on the person's answer to the prompt.
    private var authorizing: [() -> Void] = []
    /// One-shots waiting on a fix, each with its deadline's token.
    private var oneShots: [(id: Int, reply: ([String: Any]) -> Void)] = []
    private var nextShot = 0
    /// One-shots asked again after a transient `locationUnknown`.
    private var unknown = 0
    private var watching = false
    /// The last fix delivered, which under the ceiling is the cached one a
    /// read may take: `manager.location` can be a precise fix.
    private var delivered: CLLocation?
    private var last: [String: Any]?
    private var watchers: [([String: Any]) -> Void] = []
    /// The authorization last answered: CoreLocation also calls
    /// `locationManagerDidChangeAuthorization` when a manager is made.
    private var said: (CLAuthorizationStatus, CLAccuracyAuthorization)?
    /// Whether the build carries the approximate line (D7); macOS has no such key.
    private let approximate = Bundle.main.object(forInfoDictionaryKey: "NSLocationDefaultAccuracyReduced") as? Bool == true

    public init(context: ExactPackageContext) {
        changed = context.changed
        log = context.log
        super.init()
    }

    // Every delegate call the module does not handle goes to the hatch's delegate.
    override public func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || forward?.responds(to: selector) == true
    }

    override public func forwardingTarget(for selector: Selector!) -> Any? {
        forward?.responds(to: selector) == true ? forward : super.forwardingTarget(for: selector)
    }

    /// A development build's line for each change to what the module owns (D4).
    private func checkOwned() {
        if manager.delegate !== self { log("geolocation: the locationManager hatch replaced the delegate, which the module owns; its answers stop (LLP 1133.002 D4)") }
        guard watching, let w = wrote else { return }
        let changes = [("desiredAccuracy", manager.desiredAccuracy != w.accuracy), ("distanceFilter", manager.distanceFilter != w.filter), ("activityType", manager.activityType != w.activity)]
        for (name, changed) in changes where changed {
            log("geolocation: the locationManager hatch changed \(name), which the module owns while a reader runs (LLP 1133.002 D4)")
        }
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            checkOwned()
            switch name {
            case "geolocationStatus": reply(status())
            case "getCurrentPosition": whenAllowed(reply) { self.oneShot(args, reply) }
            case "watchPosition": whenAllowed(reply) { self.watch(args, reply) }
            case "requestPreciseLocation": whenAllowed(reply) { self.precise(args, reply) }
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    /// The synchronous forms (LLP 1133 D6 rule 4): a cheap read, never a prompt.
    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        switch name {
        case "geolocationStatusSync": return status()
        case "getCurrentPositionSync":
            if authorization != .notDetermined, !granted { return ["error": "PERMISSION_DENIED"] }
            guard granted, let fix = cached, eligible(fix, args) else { return ["error": "POSITION_UNAVAILABLE"] }
            return position(fix)
        default: return ["error": "NotSupportedError"]
        }
    }

    public func release(_ name: String) {
        guard name == "watchPosition", watching else { return }
        manager.stopUpdatingLocation()
        watching = false
        last = nil
    }

    // MARK: the fact

    private var authorization: CLAuthorizationStatus { manager.authorizationStatus }
    private var granted: Bool {
        #if os(macOS)
        authorization == .authorizedAlways
        #else
        authorization == .authorizedAlways || authorization == .authorizedWhenInUse
        #endif
    }

    /// `authorizationStatus` as Apple names it.
    private var authorizationWord: String {
        switch authorization {
        case .notDetermined: "notDetermined"
        case .restricted: "restricted"
        case .denied: "denied"
        case .authorizedAlways: "authorizedAlways"
        default: "authorizedWhenInUse"
        }
    }

    private func status() -> [String: Any] {
        let state = granted ? "granted" : authorization == .notDetermined ? "prompt" : "denied"
        let full = manager.accuracyAuthorization == .fullAccuracy
        said = (authorization, manager.accuracyAuthorization)
        return [
            "available": CLLocationManager.locationServicesEnabled(),
            "state": state,
            "accuracy": state == "granted" ? (full && !approximate ? "precise" : "approximate") : "unknown",
            "apple": ["authorizationStatus": authorizationWord, "accuracyAuthorization": full ? "fullAccuracy" : "reducedAccuracy"],
            "android": NSNull(),
        ]
    }

    /// `then`, once the platform allows it: asked first while undetermined
    /// (D9); refused while denied.
    private func whenAllowed(_ reply: @escaping ([String: Any]) -> Void, then: @escaping () -> Void) {
        if granted {
            then()
        } else if authorization == .notDetermined {
            authorizing.append { [self] in granted ? then() : reply(["error": "PERMISSION_DENIED"]) }
            if authorizing.count == 1 {
                #if os(macOS)
                manager.requestAlwaysAuthorization()
                #else
                manager.requestWhenInUseAuthorization()
                #endif
            }
        } else {
            reply(["error": "PERMISSION_DENIED"])
        }
    }

    public func locationManagerDidChangeAuthorization(_ m: CLLocationManager) {
        if let said, said != (authorization, m.accuracyAuthorization) {
            changed("geolocation.geolocationStatus")
            // A watch's readers too: denied, they fail; granted again, they resume (D5).
            if watching {
                last = nil
                changed("geolocation.watchPosition")
            }
        }
        if authorization != .notDetermined {
            let waiting = authorizing
            authorizing = []
            waiting.forEach { $0() }
        }
        forward?.locationManagerDidChangeAuthorization?(m)
    }

    // MARK: positions

    /// Apple's own word, else the written web option (D2); unsaid, CoreLocation's.
    private func configure(_ args: [String: Any]) {
        let apple = args["apple"] as? [String: Any] ?? [:]
        let accuracies: [String: CLLocationAccuracy] = [
            "bestForNavigation": kCLLocationAccuracyBestForNavigation, "best": kCLLocationAccuracyBest,
            "nearestTenMeters": kCLLocationAccuracyNearestTenMeters, "hundredMeters": kCLLocationAccuracyHundredMeters,
            "kilometer": kCLLocationAccuracyKilometer, "threeKilometers": kCLLocationAccuracyThreeKilometers,
            "reduced": kCLLocationAccuracyReduced,
        ]
        var accuracy = manager.desiredAccuracy
        if let word = apple["desiredAccuracy"] as? String, let a = accuracies[word] {
            accuracy = a
        } else if let high = args["enableHighAccuracy"] as? Bool {
            accuracy = high ? kCLLocationAccuracyBest : kCLLocationAccuracyHundredMeters
        }
        if approximate { accuracy = kCLLocationAccuracyReduced }
        // Each change makes CoreLocation deliver again, and a delivery asks
        // the watch's readers again, which configure again.
        if accuracy != manager.desiredAccuracy { manager.desiredAccuracy = accuracy }
        if let d = apple["distanceFilter"] as? Double { manager.distanceFilter = d }
        let activities: [String: CLActivityType] = ["other": .other, "automotiveNavigation": .automotiveNavigation, "fitness": .fitness, "otherNavigation": .otherNavigation, "airborne": .airborne]
        if let word = apple["activityType"] as? String, let a = activities[word] { manager.activityType = a }
        #if os(iOS)
        if let p = apple["pausesLocationUpdatesAutomatically"] as? Bool { manager.pausesLocationUpdatesAutomatically = p }
        #endif
        wrote = (manager.desiredAccuracy, manager.distanceFilter, manager.activityType)
    }

    private var cached: CLLocation? { approximate ? delivered : manager.location }

    /// A cached fix is eligible only when `maximumAge` is above 0 and it is
    /// younger (D2).
    private func eligible(_ fix: CLLocation, _ args: [String: Any]) -> Bool {
        guard let age = args["maximumAge"] as? Double, age > 0 else { return false }
        return -fix.timestamp.timeIntervalSinceNow * 1000 <= age
    }

    private func oneShot(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        if let fix = cached, eligible(fix, args) { return reply(position(fix)) }
        let id = nextShot
        nextShot += 1
        oneShots.append((id, reply))
        if let timeout = args["timeout"] as? Double {
            DispatchQueue.main.asyncAfter(deadline: .now() + timeout / 1000) { [self] in
                guard let at = oneShots.firstIndex(where: { $0.id == id }) else { return }
                oneShots.remove(at: at).reply(["error": "TIMEOUT"])
            }
        }
        configure(args)
        // During a watch the next update answers it (`requestLocation` does nothing then).
        if !watching { manager.requestLocation() }
    }

    private func watch(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        configure(args)
        // Started again with no fix in hand: after a denial CoreLocation need not resume on its own.
        if !watching || last == nil {
            watching = true
            manager.startUpdatingLocation()
        }
        if let last { reply(last) } else { watchers.append(reply) }
    }

    private func precise(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        // Under the ceiling there is no precise to ask for, as on Android.
        guard !approximate, manager.accuracyAuthorization == .reducedAccuracy, let key = args["purposeKey"] as? String else {
            return reply(status())
        }
        // Declined or dismissed is no failure: the call answers the status (D10).
        manager.requestTemporaryFullAccuracyAuthorization(withPurposeKey: key) { [self] _ in
            DispatchQueue.main.async { reply(self.status()) }
        }
    }

    public func locationManager(_ m: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        checkOwned()
        unknown = 0
        if let fix = locations.last {
            delivered = fix
            let p = position(fix)
            let shots = oneShots
            oneShots = []
            shots.forEach { $0.reply(p) }
            if watching {
                last = p
                let waiting = watchers
                watchers = []
                waiting.forEach { $0(p) }
                changed("geolocation.watchPosition")
            }
        }
        forward?.locationManager?(m, didUpdateLocations: locations)
    }

    public func locationManager(_ m: CLLocationManager, didFailWithError error: Error) {
        let cl = (error as? CLError)?.code
        // `locationUnknown` is transient: CoreLocation has no fix yet, as
        // right after a grant. A watch keeps trying, and a one-shot asks
        // again, a few times, before it fails.
        if cl == .locationUnknown {
            if !oneShots.isEmpty && !watching {
                if unknown < 3 {
                    unknown += 1
                    manager.requestLocation()
                } else {
                    fail(shots: true, watch: false, "POSITION_UNAVAILABLE")
                }
            }
        } else {
            fail(shots: true, watch: true, cl == .denied ? "PERMISSION_DENIED" : "POSITION_UNAVAILABLE")
        }
        forward?.locationManager?(m, didFailWithError: error)
    }

    private func fail(shots: Bool, watch: Bool, _ code: String) {
        unknown = 0
        if shots {
            let waiting = oneShots
            oneShots = []
            waiting.forEach { $0.reply(["error": code]) }
        }
        if watch {
            let waiting = watchers
            watchers = []
            waiting.forEach { $0(["error": code]) }
        }
    }

    /// A fix in the interface's `GeolocationPosition` (D1): the ellipsoidal
    /// altitude, as the web's; mean sea level under `apple`.
    private func position(_ fix: CLLocation) -> [String: Any] {
        let vertical = fix.verticalAccuracy >= 0
        var apple: [String: Any] = [
            "altitude": vertical ? fix.altitude as Any : NSNull(),
            "floor": fix.floor.map { $0.level as Any } ?? NSNull(),
            "sourceInformation": NSNull(),
        ]
        if #available(iOS 15, macOS 12, *), let info = fix.sourceInformation {
            apple["sourceInformation"] = ["isSimulatedBySoftware": info.isSimulatedBySoftware, "isProducedByAccessory": info.isProducedByAccessory]
        }
        var ellipsoidal: Any = NSNull()
        if #available(iOS 15, macOS 12, *), vertical { ellipsoidal = fix.ellipsoidalAltitude }
        return [
            "coords": [
                "latitude": fix.coordinate.latitude, "longitude": fix.coordinate.longitude,
                "accuracy": fix.horizontalAccuracy,
                "altitude": ellipsoidal,
                "altitudeAccuracy": vertical ? fix.verticalAccuracy as Any : NSNull(),
                "heading": fix.course >= 0 ? fix.course as Any : NSNull(),
                "speed": fix.speed >= 0 ? fix.speed as Any : NSNull(),
            ],
            "timestamp": (fix.timestamp.timeIntervalSince1970 * 1000).rounded(),
            "apple": apple,
            "android": NSNull(),
        ]
    }
}
