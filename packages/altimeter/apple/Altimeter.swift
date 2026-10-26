// @exact/altimeter on Apple (LLP 1133.026 D13, D15, D16, D17): Core Motion's
// `CMAltimeter`, answered in the interface's shapes. Under the agent the
// runner's declared substitute answers and nothing here runs; a bound run
// (`--bind altimeter`) reaches it on a simulator made for the drive.
//
// - One relative-altitude stream serves every reader group (D13). Updates
//   come every few seconds whatever a reader asks, so `frequency` is inert.
//   Each group (a name with one `history`) keeps a ring of its own numbered
//   samples, answered from the cursor the runner passes as `after`, as
//   @exact/motion's do (LLP 1133.013 D5).
// - Pressure is Core Motion's kPa times ten: hectopascals.
// - A group's relative altitude is the stream's less the group's first
//   sample, and `since` is when that sample came.
// - Nothing stops in the background; the system suspends delivery. If the
//   first update after a return reads a new reference (an altitude near 0
//   that the change in pressure across the gap does not explain), the
//   module bridges it by that change in pressure, by the formula Android's
//   `SensorManager.getAltitude` uses, so each group's height stays
//   continuous. Each group's next answer reads `interrupted`.
// - The request starts updates while `notDetermined`, which shows the
//   prompt, and stops them at the first callback (D13).
// - Tier 3 (D17): `AltimeterModule.hatch` is given the altimeter once,
//   before its first start; `forward` hears every update it receives.
// - The Mac has no `CMAltimeter`; its module answers the status unavailable.
import CoreMotion
import ExactPackage
import Foundation

#if os(iOS)
public final class AltimeterModule: ExactPackage {
    /// The app's access hatch (D17): given the altimeter once, before its first start.
    public static var hatch: ((CMAltimeter) -> Void)?
    /// Every update the module receives, on the main queue, after the module's own handling.
    public static var forward: ((CMAltitudeData) -> Void)?

    private static let most = 256

    private final class Group {
        let name: String, history: Int
        var zero: Double?
        var since = 0.0
        var ring: [(sequence: Int, fields: [String: Any])] = []
        var sequence = 0
        var pauses: [Int] = []
        var waiting: [() -> Void] = []
        init(name: String, history: Int) { self.name = name; self.history = history }
    }

    private let context: ExactPackageContext
    private lazy var altimeter: CMAltimeter = {
        let a = CMAltimeter()
        Self.hatch?(a)
        return a
    }()
    private var groups: [String: Group] = [:]
    private var running = false
    private var shown = true
    /// The last update's pressure (hPa) and bridged altitude, and the
    /// offset added to Core Motion's altitude since a re-reference.
    private var last: (pressure: Double, altitude: Double)?
    private var offset = 0.0
    /// Whether the next update follows a return from the background.
    private var returned = false
    /// Requests waiting on the prompt's answer.
    private var asking: [([String: Any]) -> Void] = []
    private var lastStatus: [String: Any]?

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            // Under the agent the declared substitute answers; nothing here may reach Core Motion (D23 item 1).
            guard !context.agent else { return reply(["error": "NotSupportedError"]) }
            switch name {
            case "altimeterStatus": reply(status())
            case "requestAltimeterPermission": request(reply)
            case "barometer", "altimeter": read(name, args, reply)
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        guard !context.agent else { return ["error": "NotSupportedError"] }
        return name == "altimeterStatusSync" ? status() : ["error": "NotSupportedError"]
    }

    private static func word(_ s: CMAuthorizationStatus) -> String {
        switch s {
        case .notDetermined: return "notDetermined"
        case .restricted: return "restricted"
        case .denied: return "denied"
        default: return "authorized"
        }
    }

    private func status() -> [String: Any] {
        let available = CMAltimeter.isRelativeAltitudeAvailable(), auth = CMAltimeter.authorizationStatus()
        let state = !available ? "unknown" : auth == .notDetermined ? "prompt" : auth == .authorized ? "granted" : "denied"
        let s: [String: Any] = ["available": available, "state": state,
                                "apple": ["authorizationStatus": Self.word(auth), "absoluteAltitude": CMAltimeter.isAbsoluteAltitudeAvailable()]]
        lastStatus = s
        return s
    }

    private func request(_ reply: @escaping ([String: Any]) -> Void) {
        guard CMAltimeter.isRelativeAltitudeAvailable(), CMAltimeter.authorizationStatus() == .notDetermined else { return reply(status()) }
        asking.append(reply)
        restart()
    }

    // MARK: readers

    private static func key(_ name: String, _ args: [String: Any]) -> String {
        "\(name) \(Int(args["history"] as? Double ?? 0))"
    }

    private func read(_ name: String, _ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard CMAltimeter.isRelativeAltitudeAvailable() else { return reply(["error": "NotSupportedError"]) }
        switch CMAltimeter.authorizationStatus() {
        case .notDetermined: return reply(["error": "NotAllowedError", "message": "requestAltimeterPermission"])
        case .authorized: break
        default: return reply(["error": "NotAllowedError"])
        }
        let key = Self.key(name, args)
        let group = groups[key] ?? Group(name: name, history: min(Self.most, Int(args["history"] as? Double ?? 0)))
        groups[key] = group
        restart()
        guard group.ring.isEmpty else { return reply(answer(group, args)) }
        // A poll while hidden answers an empty batch, never an error (D13).
        guard shown else { return reply(["error": "NotReadableError"]) }
        // The first answer waits for the stream's first update: they come every few seconds.
        var answered = false
        let once: () -> Void = { [weak self] in
            guard !answered, let self else { return }
            answered = true
            reply(group.ring.isEmpty ? ["error": "NotReadableError"] : self.answer(group, args))
        }
        group.waiting.append(once)
        DispatchQueue.main.asyncAfter(deadline: .now() + 5, execute: once)
    }

    private func answer(_ group: Group, _ args: [String: Any]) -> [String: Any] {
        guard let newest = group.ring.last else { return ["error": "NotReadableError"] }
        var out = newest.fields
        out["sequence"] = newest.sequence
        out["samples"] = [[String: Any]]()
        out["dropped"] = 0
        out["interrupted"] = false
        if let after = (args["after"] as? Double).map(Int.init) {
            let fresh = group.ring.filter { $0.sequence > after }
            let kept = group.history > 0 ? Array(fresh.suffix(group.history)) : []
            out["samples"] = kept.map(\.fields)
            out["dropped"] = group.history > 0 ? max(0, newest.sequence - after - kept.count) : 0
            out["interrupted"] = group.pauses.contains { $0 >= after }
            if !shown { out["samples"] = [[String: Any]](); out["interrupted"] = true }
        }
        return out
    }

    public func release(_ name: String) {
        groups = groups.filter { $0.value.name != name }
        restart()
    }

    public func release(_ name: String, args: [String: Any]) {
        groups[Self.key(name, args)] = nil
        restart()
    }

    public func foreground(_ shown: Bool) {
        guard shown != self.shown else { return }
        self.shown = shown
        if !shown { for g in groups.values { g.pauses.append(g.sequence) } } else { returned = true }
        guard shown, let before = lastStatus else { return }
        if (status()["state"] as? String) != (before["state"] as? String) { context.changed("altimeter.altimeterStatus") }
    }

    public func destroy(_ done: @escaping () -> Void) {
        if running { altimeter.stopRelativeAltitudeUpdates() }
        running = false
        groups = [:]
        done()
    }

    private func restart() {
        let wanted = !groups.isEmpty || !asking.isEmpty
        guard wanted != running else { return }
        running = wanted
        if !wanted {
            altimeter.stopRelativeAltitudeUpdates()
            last = nil; offset = 0; returned = false
            context.log("altimeter: stopped")
            return
        }
        context.log("altimeter: relative updates")
        altimeter.startRelativeAltitudeUpdates(to: .main) { [weak self] data, error in
            guard let self else { return }
            if !self.asking.isEmpty {
                let replies = self.asking
                self.asking = []
                let s = self.status()
                self.context.changed("altimeter.altimeterStatus")
                replies.forEach { $0(s) }
                if self.groups.isEmpty { self.restart() }
            }
            if let data { self.take(data) } else if error != nil, self.groups.isEmpty { self.restart() }
        }
    }

    /// Height in metres for pressure `p` hPa in the standard atmosphere
    /// (`SensorManager.getAltitude`'s formula).
    private static func height(_ p: Double) -> Double { 44330 * (1 - pow(p / 1013.25, 1 / 5.255)) }

    private func take(_ d: CMAltitudeData) {
        let pressure = d.pressure.doubleValue * 10, raw = d.relativeAltitude.doubleValue
        if returned, let last {
            returned = false
            let expected = last.altitude + Self.height(pressure) - Self.height(last.pressure)
            // A new reference reads near 0 where the pressure says the height moved little.
            if abs(raw) < 0.5, abs(expected - (raw + offset)) > 3 {
                offset = expected - raw
                context.log("altimeter: a new reference after the return, bridged by pressure")
            }
        }
        let altitude = raw + offset, at = context.now() - (ProcessInfo.processInfo.systemUptime - d.timestamp) * 1000
        last = (pressure, altitude)
        for group in groups.values {
            group.sequence += 1
            let fields: [String: Any]
            if group.name == "barometer" {
                fields = ["pressure": pressure, "timestamp": at]
            } else {
                if group.zero == nil { group.zero = altitude; group.since = at }
                fields = ["relativeAltitude": altitude - (group.zero ?? altitude), "since": group.since, "timestamp": at]
            }
            group.ring.append((group.sequence, fields))
            let bound = max(1, group.history)
            if group.ring.count > bound { group.ring.removeFirst(group.ring.count - bound) }
            let waiting = group.waiting
            group.waiting = []
            waiting.forEach { $0() }
        }
        Self.forward?(d)
    }
}
#else
public final class AltimeterModule: ExactPackage {
    private static let status: [String: Any] = ["available": false, "state": "unknown", "apple": NSNull()]

    public init(context: ExactPackageContext) {}

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        reply(name == "altimeterStatus" || name == "requestAltimeterPermission" ? Self.status : ["error": "NotSupportedError"])
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "altimeterStatusSync" ? Self.status : ["error": "NotSupportedError"]
    }
}
#endif
