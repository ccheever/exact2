// @exact/health on Apple (LLP 1133.026 D1-D11): HealthKit through one
// `HKHealthStore`, answered in the interface's shapes. The implementation is
// its own substitute (§11): under the agent it makes no store and answers
// from `AgentHealth`, the fixture, applying the permission states and facts
// the runner tells it.
//
// - HealthKit never says a read is granted (D5): a type asked and answered
//   reads `unknown`, with `since` the limited window
//   `getEarliestAuthorizedSampleDate` names (iOS 27), and every answer's
//   `complete` is false. A type never asked reads `prompt`, and a source
//   fails `NotAllowedError` for it rather than read nothing (D6).
// - Totals are HealthKit's statistics (D2), which merge every source; a
//   bucket with no sample is `none`, never 0. `since` clamps every query.
// - Workouts are ordered by start, then id, and paged by a key cursor over
//   both (D8); a cursor whose workout is gone fails `cursor-stale`.
// - One `HKObserverQuery` per type a reader reads announces a change, at
//   most four times a second; a return to the foreground announces once
//   (D5, D8).
// - Tier 3 (D11): `HealthModule.hatch` is given the store once, when made.
// - The Mac calls only `isHealthDataAvailable()` and answers `available`
//   false whatever it returns (D23 item 2).
import ExactPackage
import Foundation
import HealthKit

#if os(iOS)
public final class HealthModule: ExactPackage {
    /// The app's access hatch (D11): given the store once, when it is made.
    public static var hatch: ((HKHealthStore) -> Void)?

    /// Tier 1's types (D1): HealthKit's type and the unit both Health apps show.
    static let quantities: [String: (HKQuantityTypeIdentifier, HKUnit)] = [
        "steps": (.stepCount, .count()), "distance": (.distanceWalkingRunning, .meter()),
        "activeEnergy": (.activeEnergyBurned, .kilocalorie()),
    ]
    static let unitWords = ["steps": "count", "distance": "m", "activeEnergy": "kcal"]
    static let names = ["healthStatus", "healthStatistics", "healthWorkouts"]

    private let context: ExactPackageContext
    private lazy var store: HKHealthStore = {
        let s = HKHealthStore()
        Self.hatch?(s)
        return s
    }()
    /// Under the agent: the fixture, and what the runner told.
    private lazy var agentStore = AgentHealth()
    private var told: [String: String] = [:]
    private var facts: [String: [String: Any]] = [:]
    private var observers: [String: HKObserverQuery] = [:]
    private var announcing = false
    private var shown = true

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        let main: ([String: Any]) -> Void = { out in DispatchQueue.main.async { reply(out) } }
        switch name {
        case "healthStatus": status(args, main)
        case "requestHealthAuthorization": request(args, main)
        case "healthStatistics": statistics(args, main)
        case "healthWorkouts": workouts(args, main)
        default: main(["error": "NotSupportedError"])
        }
    }

    public func permissions(_ states: [String: String]) { told = states }

    public func preferred(_ values: [String: Any]) {
        guard context.agent else { return }
        facts = (values["facts"] as? [String: Any] ?? [:]).compactMapValues { $0 as? [String: Any] }
        if let name = values["store"] as? String {
            let file = (name as NSString).lastPathComponent
            context.log(agentStore.load(file) ? "health: store \(file)" : "health: no fixture \(file)")
        }
        if let now = values["now"] as? String { agentStore.now = Self.local(now, agentStore.zone) }
        // On the next turn, not after the wall clock's coalescing: the drive's clock says when readers ask again.
        DispatchQueue.main.async { [self] in for name in Self.names { context.changed("health.\(name)") } }
    }

    public func foreground(_ shown: Bool) {
        defer { self.shown = shown }
        if shown, !self.shown { announce() }
    }

    public func release(_ name: String) {
        guard name != "healthStatus" else { return }
        for q in observers.values { store.stop(q) }
        observers = [:]
    }

    public func destroy(_ done: @escaping () -> Void) {
        release("healthStatistics")
        done()
    }

    /// Every reader asked again, coalesced (D8).
    private func announce() {
        guard !announcing else { return }
        announcing = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { [self] in
            announcing = false
            for name in Self.names { context.changed("health.\(name)") }
        }
    }

    /// `2026-10-25T09:00` in `zone`, as ms since the epoch.
    static func local(_ text: String, _ zone: TimeZone) -> Double? {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = zone
        for format in ["yyyy-MM-dd'T'HH:mm:ss", "yyyy-MM-dd'T'HH:mm", "yyyy-MM-dd"] {
            f.dateFormat = format
            if let d = f.date(from: text) { return d.timeIntervalSince1970 * 1000 }
        }
        return nil
    }

    private static func read(_ args: [String: Any]) -> [String] {
        (args["read"] as? [Any] ?? []).compactMap { $0 as? String }
    }

    private static func objectType(_ type: String) -> HKObjectType? {
        type == "workouts" ? HKObjectType.workoutType() : quantities[type].map { HKQuantityType($0.0) }
    }

    /// D5's summary over the types' `read` words.
    private static func summary(_ reads: [String]) -> String {
        reads.contains("prompt") ? "prompt" : reads.contains("unknown") ? "unknown" : !reads.isEmpty && reads.allSatisfy { $0 == "denied" } ? "denied" : "granted"
    }

    private static func answer(_ types: [[String: Any]]) -> [String: Any] {
        let reads = types.compactMap { $0["read"] as? String }
        return ["available": true, "state": summary(reads), "types": types, "android": NSNull(),
                "apple": ["requestStatus": reads.contains("prompt") ? "shouldRequest" : "unnecessary"]]
    }

    // MARK: status and the request

    /// Each type's `read` and `since` (D5).
    private func types(_ read: [String], _ done: @escaping ([[String: Any]]) -> Void) {
        if context.agent {
            return done(read.map { t in
                let state = told["health.\(t)"] ?? "prompt"
                return ["type": t, "read": state == "prompt" ? "prompt" : "unknown",
                        "since": state == "prompt" ? NSNull() : facts["health.\(t)"]?["since"] as? Double ?? NSNull() as Any, "complete": false]
            })
        }
        var reads: [String: String] = [:]
        let group = DispatchGroup(), lock = NSLock()
        for t in read {
            guard let type = Self.objectType(t) else { continue }
            group.enter()
            store.getRequestStatusForAuthorization(toShare: [], read: [type]) { status, _ in
                lock.lock(); reads[t] = status == .shouldRequest ? "prompt" : "unknown"; lock.unlock()
                group.leave()
            }
        }
        group.notify(queue: .main) { [self] in
            let asked = read.filter { reads[$0] == "unknown" }
            since(asked) { windows in
                done(read.map { t in ["type": t, "read": reads[t] ?? "unknown", "since": windows[t] ?? NSNull() as Any, "complete": false] })
            }
        }
    }

    /// The limited windows HealthKit names (iOS 27), by type.
    private func since(_ asked: [String], _ done: @escaping ([String: Double]) -> Void) {
        guard #available(iOS 27.0, *), !asked.isEmpty else { return done([:]) }
        let objects = Dictionary(uniqueKeysWithValues: asked.compactMap { t in Self.objectType(t).map { (t, $0) } })
        store.getEarliestAuthorizedSampleDate(for: Set(objects.values)) { dates, _ in
            let windows = objects.compactMapValues { dates?[$0].map { $0.timeIntervalSince1970 * 1000 } }
            DispatchQueue.main.async { done(windows) }
        }
    }

    private func status(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard context.agent || HKHealthStore.isHealthDataAvailable() else {
            return reply(["available": false, "state": "unknown", "types": [[String: Any]](), "apple": NSNull(), "android": NSNull()])
        }
        types(Self.read(args)) { reply(Self.answer($0)) }
    }

    private func request(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let read = Self.read(args)
        guard !context.agent else { return status(args, reply) }
        guard HKHealthStore.isHealthDataAvailable() else { return reply(["error": "NotSupportedError"]) }
        store.requestAuthorization(toShare: [], read: Set(read.compactMap(Self.objectType))) { [weak self] _, _ in
            DispatchQueue.main.async {
                self?.announce()
                self?.status(args, reply)
            }
        }
    }

    // MARK: reads

    /// The type a read names and its `read` state; a failure for `prompt`.
    private func admitted(_ type: String, _ done: @escaping (_ since: Double?, _ shared: Bool) -> Void, _ reply: @escaping ([String: Any]) -> Void) {
        types([type]) { [self] states in
            let state = states.first?["read"] as? String ?? "prompt"
            guard state != "prompt" else { return reply(["error": "NotAllowedError", "message": "requestHealthAuthorization"]) }
            let fact = facts["health.\(type)"]
            done(states.first?["since"] as? Double, (fact?["shared"] as? Bool) ?? (told["health.\(type)"] != "denied"))
        }
    }

    private static func failure(_ error: Error?) -> [String: Any] {
        switch (error as? HKError)?.code {
        case .errorDatabaseInaccessible: return ["error": "NotReadableError", "message": "the Health store is locked"]
        case .errorAuthorizationNotDetermined: return ["error": "NotAllowedError", "message": "requestHealthAuthorization"]
        case .errorHealthDataUnavailable: return ["error": "NotSupportedError"]
        default: return ["error": "NotReadableError", "message": error?.localizedDescription ?? "no answer"]
        }
    }

    private func statistics(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let apple = args["apple"] as? [String: Any] ?? [:]
        let quantity = apple["quantityType"] as? String, type = quantity ?? args["type"] as? String ?? "steps"
        guard let start = args["start"] as? Double, let end = args["end"] as? Double, end > start,
              let interval = args["interval"] as? String else { return reply(["error": "TypeError", "message": "start, end and interval"]) }
        let zone = context.agent ? agentStore.zone : TimeZone.current
        guard let edges = AgentHealth.edges(start, end, interval, zone) else { return reply(["error": "RangeError", "message": "more than 400 buckets"]) }
        let unit = apple["unit"] as? String ?? Self.unitWords[type] ?? agentStore.units[type] ?? "count"
        guard context.agent || HKHealthStore.isHealthDataAvailable() else { return reply(["error": "NotSupportedError"]) }
        admitted(type, { [self] since, shared in
            let answer: ([[String: Any]]) -> [String: Any] = { ["type": type, "unit": unit, "since": since ?? NSNull() as Any, "complete": false, "buckets": $0] }
            if context.agent {
                context.log("health: statistics \(type) \(edges.count - 1) buckets")
                return reply(answer(agentStore.buckets(type, edges, since: since, shared: shared)))
            }
            let identifier = quantity.map { HKQuantityTypeIdentifier(rawValue: $0) } ?? Self.quantities[type]?.0
            guard let identifier else { return reply(["error": "NotSupportedError"]) }
            let qt = HKQuantityType(identifier), hkUnit = apple["unit"] is String ? HKUnit(from: unit) : Self.quantities[type]?.1 ?? .count()
            let option = Self.option(apple["statistics"] as? String)
            var components = DateComponents()
            switch interval {
            case "PT1H": components.hour = 1
            case "P1D": components.day = 1
            case "P1W": components.day = 7
            default: components.month = 1
            }
            let from = Date(timeIntervalSince1970: max(start, since ?? start) / 1000), to = Date(timeIntervalSince1970: end / 1000)
            let q = HKStatisticsCollectionQuery(quantityType: qt, quantitySamplePredicate: HKQuery.predicateForSamples(withStart: from, end: to, options: []),
                                                options: option, anchorDate: Date(timeIntervalSince1970: start / 1000), intervalComponents: components)
            q.initialResultsHandler = { _, collection, error in
                guard let collection else { return reply(Self.failure(error)) }
                let buckets = zip(edges, edges.dropFirst()).map { s, e -> [String: Any] in
                    let stats = collection.statistics(for: Date(timeIntervalSince1970: s / 1000))
                    let value = stats.flatMap { Self.value($0, option)?.doubleValue(for: hkUnit) }
                    return ["start": s, "end": e, "value": value ?? NSNull() as Any]
                }
                reply(answer(buckets))
            }
            store.execute(q)
            observe(type, qt)
        }, reply)
    }

    private static func option(_ word: String?) -> HKStatisticsOptions {
        switch word {
        case "discreteAverage": return .discreteAverage
        case "discreteMin": return .discreteMin
        case "discreteMax": return .discreteMax
        case "mostRecent": return .mostRecent
        default: return .cumulativeSum
        }
    }

    private static func value(_ s: HKStatistics, _ option: HKStatisticsOptions) -> HKQuantity? {
        switch option {
        case .discreteAverage: return s.averageQuantity()
        case .discreteMin: return s.minimumQuantity()
        case .discreteMax: return s.maximumQuantity()
        case .mostRecent: return s.mostRecentQuantity()
        default: return s.sumQuantity()
        }
    }

    private func observe(_ key: String, _ type: HKSampleType) {
        guard observers[key] == nil else { return }
        let q = HKObserverQuery(sampleType: type, predicate: nil) { [weak self] _, completion, _ in
            DispatchQueue.main.async { self?.announce() }
            completion()
        }
        observers[key] = q
        store.execute(q)
    }

    private func workouts(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let start = args["start"] as? Double, let end = args["end"] as? Double else { return reply(["error": "TypeError", "message": "start and end"]) }
        let limit = max(1, min(500, Int(args["limit"] as? Double ?? 50))), after = args["after"] as? String
        guard context.agent || HKHealthStore.isHealthDataAvailable() else { return reply(["error": "NotSupportedError"]) }
        admitted("workouts", { [self] since, shared in
            let page: ([[String: Any]]) -> [String: Any] = { all in
                Self.page(all, after: after, limit: limit, since: since)
            }
            if context.agent {
                let all = shared ? agentStore.workouts(start, end, since: since) : []
                context.log("health: workouts \(all.count)")
                return reply(page(all))
            }
            let predicate = HKQuery.predicateForSamples(withStart: Date(timeIntervalSince1970: max(start, since ?? start) / 1000),
                                                        end: Date(timeIntervalSince1970: end / 1000), options: .strictStartDate)
            let q = HKSampleQuery(sampleType: .workoutType(), predicate: predicate, limit: HKObjectQueryNoLimit,
                                  sortDescriptors: [NSSortDescriptor(key: HKSampleSortIdentifierStartDate, ascending: true)]) { _, samples, error in
                guard let samples else { return reply(Self.failure(error)) }
                let all = samples.compactMap { $0 as? HKWorkout }.filter { $0.startDate.timeIntervalSince1970 * 1000 >= start }
                    .map(Self.workout).sorted(by: AgentHealth.ordered)
                reply(page(all))
            }
            store.execute(q)
            observe("workouts", .workoutType())
        }, reply)
    }

    /// One page after the key cursor `after` (`<start>:<id>`), D8.
    static func page(_ all: [[String: Any]], after: String?, limit: Int, since: Double?) -> [String: Any] {
        var rest = all[...]
        if let after {
            guard let cut = after.firstIndex(of: ":"), let at = Double(after[..<cut]),
                  all.contains(where: { $0["start"] as? Double == at && $0["id"] as? String == String(after[after.index(after: cut)...]) })
            else { return ["error": "InvalidStateError", "message": "cursor-stale"] }
            let id = String(after[after.index(after: cut)...])
            rest = all.drop { w in
                let s = w["start"] as? Double ?? 0
                return s < at || (s == at && (w["id"] as? String ?? "") <= id)
            }
        }
        let shown = Array(rest.prefix(limit))
        let next: Any = rest.count > limit ? shown.last.map { "\($0["start"] as? Double ?? 0):\($0["id"] as? String ?? "")" } ?? NSNull() : NSNull()
        return ["workouts": shown, "next": next, "since": since ?? NSNull() as Any, "complete": false]
    }

    private static let activities: [HKWorkoutActivityType: String] = [
        .walking: "walking", .running: "running", .hiking: "hiking", .cycling: "cycling", .swimming: "swimming",
    ]
    private static let activityNames: [HKWorkoutActivityType: String] = [
        .walking: "walking", .running: "running", .hiking: "hiking", .cycling: "cycling", .swimming: "swimming",
        .yoga: "yoga", .functionalStrengthTraining: "functionalStrengthTraining", .traditionalStrengthTraining: "traditionalStrengthTraining",
        .highIntensityIntervalTraining: "highIntensityIntervalTraining", .elliptical: "elliptical", .rowing: "rowing",
        .stairClimbing: "stairClimbing", .dance: "dance", .soccer: "soccer", .tennis: "tennis", .golf: "golf", .other: "other",
    ]

    private static func workout(_ w: HKWorkout) -> [String: Any] {
        let distanceType: HKQuantityTypeIdentifier = w.workoutActivityType == .cycling ? .distanceCycling
            : w.workoutActivityType == .swimming ? .distanceSwimming : .distanceWalkingRunning
        let distance = w.statistics(for: HKQuantityType(distanceType))?.sumQuantity()?.doubleValue(for: .meter())
        let energy = w.statistics(for: HKQuantityType(.activeEnergyBurned))?.sumQuantity()?.doubleValue(for: .kilocalorie())
        let source = w.sourceRevision.source
        return ["id": w.uuid.uuidString, "activity": activities[w.workoutActivityType] ?? "other",
                "start": w.startDate.timeIntervalSince1970 * 1000, "end": w.endDate.timeIntervalSince1970 * 1000, "duration": w.duration,
                "distance": distance ?? NSNull() as Any, "energy": energy ?? NSNull() as Any, "source": source.name,
                "apple": ["activityType": activityNames[w.workoutActivityType] ?? "\(w.workoutActivityType.rawValue)", "sourceBundleId": source.bundleIdentifier],
                "android": NSNull()]
    }
}
#else
/// The Mac (D21, D23 item 2): HealthKit's `isHealthDataAvailable()` is the
/// one call, and the answer is `available` false whatever it returns.
public final class HealthModule: ExactPackage {
    private static let status: [String: Any] = ["available": false, "state": "unknown", "types": [[String: Any]](), "apple": NSNull(), "android": NSNull()]
    private let context: ExactPackageContext

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        guard name == "healthStatus" else { return reply(["error": "NotSupportedError"]) }
        context.log("health: isHealthDataAvailable \(HKHealthStore.isHealthDataAvailable()); answered unavailable")
        reply(Self.status)
    }
}
#endif
