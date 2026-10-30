// @exact/pedometer on Apple (LLP 1133.026 D12, D15, D16, D17): Core Motion's
// `CMPedometer`, answered in the interface's shapes. Under the agent the
// runner's declared substitute answers and nothing here runs; a bound run
// (`--bind pedometer`) reaches it on a simulator made for the drive.
//
// - A reader group is a `start` (D12): its boundary is fixed at its first
//   ask, the later of `start` and seven days before then, since Core Motion
//   keeps only the past seven days. The oldest group takes the module's one
//   update stream (`startUpdates(from:)`); any other group is answered by a
//   query from its boundary to now. `since` is the answering data's
//   `startDate`, which follows Core Motion if it moves.
// - `CMPedometer` has no request call: the first query prompts (D12). The
//   request queries the last second while `notDetermined` and answers the
//   status its handler leaves.
// - A reader never starts updates while the state is `prompt` or `denied`.
// - Delivery stops while the app is suspended and the next update is
//   cumulative again, so nothing stops here in the background, and a poll
//   while hidden answers the count held (D12).
// - Tier 3 (D17): `PedometerModule.hatch` is given the pedometer once, before
//   its first start; `forward` hears every update the module receives.
// - The Mac answers `available` false (D21): Core Motion's step counting is
//   the iPhone's.
import CoreMotion
import ExactPackage
import Foundation

#if os(iOS)
public final class PedometerModule: ExactPackage {
    /// The app's access hatch (D17): given the pedometer once, before its first start.
    public static var hatch: ((CMPedometer) -> Void)?
    /// Every update the module receives, on the main queue, after the module's own handling.
    public static var forward: ((CMPedometerData) -> Void)?

    /// What Core Motion keeps (D12): "only the past seven days".
    private static let kept = 7 * 86_400_000.0

    private final class Group {
        let boundary: Date
        var data: CMPedometerData?
        var waiting: [() -> Void] = []
        init(boundary: Date) { self.boundary = boundary }
    }

    private let context: ExactPackageContext
    private lazy var pedometer: CMPedometer = {
        let p = CMPedometer()
        Self.hatch?(p)
        return p
    }()
    /// Reader groups by their `start` (or the first ask's moment).
    private var groups: [String: Group] = [:]
    /// The group the update stream serves.
    private var streaming: String?
    private var lastStatus: [String: Any]?

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            // Under the agent the declared substitute answers; nothing here may reach Core Motion (D23 item 1).
            guard !context.agent else { return reply(["error": "NotSupportedError"]) }
            switch name {
            case "pedometerStatus": reply(status())
            case "requestPedometerPermission": request(reply)
            case "pedometer": read(args, reply)
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        guard !context.agent else { return ["error": "NotSupportedError"] }
        return name == "pedometerStatusSync" ? status() : ["error": "NotSupportedError"]
    }

    static func word(_ s: CMAuthorizationStatus) -> String {
        switch s {
        case .notDetermined: return "notDetermined"
        case .restricted: return "restricted"
        case .denied: return "denied"
        default: return "authorized"
        }
    }

    /// D15's table: `notDetermined` is `prompt`, `restricted` and `denied` are `denied`.
    static func state(_ s: CMAuthorizationStatus) -> String {
        switch s {
        case .notDetermined: return "prompt"
        case .authorized: return "granted"
        default: return "denied"
        }
    }

    private func status() -> [String: Any] {
        let available = CMPedometer.isStepCountingAvailable(), auth = CMPedometer.authorizationStatus()
        let s: [String: Any] = [
            "available": available, "state": available ? Self.state(auth) : "unknown",
            "apple": ["authorizationStatus": Self.word(auth), "isDistanceAvailable": CMPedometer.isDistanceAvailable(),
                      "isFloorCountingAvailable": CMPedometer.isFloorCountingAvailable(), "isPaceAvailable": CMPedometer.isPaceAvailable(),
                      "isCadenceAvailable": CMPedometer.isCadenceAvailable()] as [String: Any],
            "android": NSNull(),
        ]
        lastStatus = s
        return s
    }

    private func request(_ reply: @escaping ([String: Any]) -> Void) {
        guard CMPedometer.isStepCountingAvailable(), CMPedometer.authorizationStatus() == .notDetermined else { return reply(status()) }
        let end = Date()
        pedometer.queryPedometerData(from: end.addingTimeInterval(-1), to: end) { [weak self] _, _ in
            DispatchQueue.main.async {
                guard let self else { return }
                let s = self.status()
                self.context.changed("pedometer.pedometerStatus")
                reply(s)
            }
        }
    }

    // MARK: readers

    private func read(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard CMPedometer.isStepCountingAvailable() else { return reply(["error": "NotSupportedError"]) }
        switch CMPedometer.authorizationStatus() {
        case .notDetermined: return reply(["error": "NotAllowedError", "message": "requestPedometerPermission"])
        case .authorized: break
        default: return reply(["error": "NotAllowedError"])
        }
        let start = args["start"] as? Double, key = start.map { "\($0)" } ?? "first"
        let group: Group
        if let g = groups[key] {
            group = g
        } else {
            let asked = context.now()
            group = Group(boundary: Date(timeIntervalSince1970: max(start ?? asked, asked - Self.kept) / 1000))
            groups[key] = group
            if streaming == nil { stream(key, group) }
        }
        if streaming != key {
            // A group the stream does not serve reads its count by a query.
            pedometer.queryPedometerData(from: group.boundary, to: Date()) { [weak self] data, error in
                DispatchQueue.main.async {
                    guard let self else { return }
                    if let data { group.data = data }
                    reply(group.data.map(self.answer) ?? Self.failure(error))
                }
            }
            return
        }
        if let data = group.data { return reply(answer(data)) }
        // The first answer waits for the stream's first update, two seconds at most.
        var answered = false
        let once: () -> Void = { [weak self] in
            guard !answered, let self else { return }
            answered = true
            reply(group.data.map(self.answer) ?? ["error": "NotReadableError"])
        }
        group.waiting.append(once)
        DispatchQueue.main.asyncAfter(deadline: .now() + 2, execute: once)
    }

    private func stream(_ key: String, _ group: Group) {
        streaming = key
        context.log("pedometer: updates from \(Int(group.boundary.timeIntervalSince1970 * 1000))")
        pedometer.startUpdates(from: group.boundary) { [weak self] data, _ in
            DispatchQueue.main.async {
                guard let self, let data, self.streaming == key else { return }
                group.data = data
                let waiting = group.waiting
                group.waiting = []
                waiting.forEach { $0() }
                Self.forward?(data)
            }
        }
    }

    private static func failure(_ error: Error?) -> [String: Any] {
        let code = (error as NSError?).map { $0.domain == CMErrorDomain ? $0.code : -1 }
        if code == Int(CMErrorMotionActivityNotAuthorized.rawValue) { return ["error": "NotAllowedError"] }
        return ["error": "NotReadableError"]
    }

    private func answer(_ d: CMPedometerData) -> [String: Any] {
        let apple: [String: Any] = [
            "distance": d.distance ?? NSNull(), "floorsAscended": d.floorsAscended ?? NSNull(), "floorsDescended": d.floorsDescended ?? NSNull(),
            "currentPace": d.currentPace ?? NSNull(), "currentCadence": d.currentCadence ?? NSNull(), "averageActivePace": d.averageActivePace ?? NSNull(),
        ]
        return ["steps": d.numberOfSteps.doubleValue, "since": d.startDate.timeIntervalSince1970 * 1000,
                "timestamp": d.endDate.timeIntervalSince1970 * 1000, "apple": apple, "android": NSNull()]
    }

    public func release(_ name: String) {
        guard name == "pedometer" else { return }
        groups = [:]
        if streaming != nil { pedometer.stopUpdates() }
        streaming = nil
    }

    public func release(_ name: String, args: [String: Any]) {
        guard name == "pedometer" else { return }
        let key = (args["start"] as? Double).map { "\($0)" } ?? "first"
        groups[key] = nil
        guard streaming == key else { return }
        pedometer.stopUpdates()
        streaming = nil
        // The oldest group left takes the stream.
        if let (next, group) = groups.min(by: { $0.value.boundary < $1.value.boundary }) { stream(next, group) }
    }

    public func foreground(_ shown: Bool) {
        // The person may have changed Motion & Fitness in Settings while away.
        guard shown, let before = lastStatus else { return }
        let now = status()
        if (now["state"] as? String) != (before["state"] as? String) { context.changed("pedometer.pedometerStatus") }
    }

    public func destroy(_ done: @escaping () -> Void) {
        if streaming != nil { pedometer.stopUpdates() }
        streaming = nil
        groups = [:]
        done()
    }
}
#else
public final class PedometerModule: ExactPackage {
    private static let status: [String: Any] = ["available": false, "state": "unknown", "apple": NSNull(), "android": NSNull()]

    public init(context: ExactPackageContext) {}

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        reply(name == "pedometerStatus" || name == "requestPedometerPermission" ? Self.status : ["error": "NotSupportedError"])
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "pedometerStatusSync" ? Self.status : ["error": "NotSupportedError"]
    }
}
#endif
