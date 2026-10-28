// The wake entry points the host keeps, and its store of packages' wake
// requests (LLP 1133.017 D8). `BGTaskScheduler` takes a launch handler only
// before the app finishes launching, and a package's Swift loads with the
// app's module after its session boots, so the host registers the
// identifiers Info.plist permits (a grant line derived each, D14) and
// multiplexes every package's requests through them:
//
// - iOS: `<bundle id>.refresh`, one `BGAppRefreshTaskRequest` at the soonest
//   refresh request; `<bundle id>.processing.network` and
//   `.processing.network-power`, one `BGProcessingTaskRequest` each at the
//   soonest processing request of its constraints. Submitted again on each
//   change, on entering the background and at the end of each wake.
// - macOS: one `NSBackgroundActivityScheduler` a request, while the
//   process runs.
//
// The module side reaches the host through `NotificationCenter`, the one
// channel both share in the process (ExactExtensions.swift):
//
// - `exact.wakes.request` `{package, key, request}`: a request, or `null`
//   to cancel it.
// - `exact.wakes.due` `{token, package, kind, keys, deadline, task}`, an
//   `NSMutableDictionary` the module marks `taken`: the due requests of one
//   package, handed to its implementation.
// - `exact.wakes.done` `{token, package, success}`: that package is done.
// - `exact.wakes.expired` `{token}`: the platform expired the wake; each
//   event raised in it that has not ended expires (ExactPackage.swift).
// - `exact.wakes.ready`: a session's module was made; a wake held for one
//   is handed over, and at the first, the transfers that ended unretired
//   (Transfers.swift).
//
// A wake that arrives with no module ready is held, and `boot` (the app's
// host, ExactIOS/main.swift) boots a session for it with no view (D7).
import Foundation
import os
#if os(iOS)
import BackgroundTasks
import UIKit
#endif

public enum WakeStore {
    private struct Request: Codable {
        var package: String, key: String, kind: String, at: Double, network: Bool, power: Bool
    }

    /// A platform wake being handed to packages: the task, its due
    /// requests, and the packages not yet done.
    private final class Arrival {
        let token = UUID().uuidString, kind: String, due: [Request]
        let complete: (Bool) -> Void
        var waiting: Set<String> = [], success = true, offered = false, booted = false
        init(kind: String, due: [Request], complete: @escaping (Bool) -> Void) { self.kind = kind; self.due = due; self.complete = complete }
        #if os(iOS)
        var task: BGTask?
        #endif
    }

    private static let key = "exact.wakes"
    private static var requests: [String: Request] = [:]
    private static var arrivals: [Arrival] = []
    private static var installed = false, readied = false
    /// Boots a session with no view for a wake that found no module ready;
    /// the app's host sets it. Nil: the wake waits for a session.
    public static var boot: (() -> Void)?
    #if os(macOS)
    /// Each request's scheduler, with the time it was made for.
    private static var schedulers: [String: (NSBackgroundActivityScheduler, Double)] = [:]
    #endif

    private static var bundle: String { Bundle.main.bundleIdentifier ?? "app" }

    /// Into the live sessions' journals and the system's log (subsystem
    /// `dev.exact`, category `wakes`), which a wake with no session and no
    /// console reaches (`log show --predicate 'subsystem == "dev.exact"'`).
    static func log(_ line: String) {
        ExactApp.shared.sessions.filter { $0.state != .destroyed }.forEach { $0.log(line) }
        logger.notice("\(line, privacy: .public)")
    }
    private static let logger = Logger(subsystem: "dev.exact", category: "wakes")
    private static func now() -> Double { Date().timeIntervalSince1970 * 1000 }

    /// Install at launch, before `application(_:didFinishLaunchingWithOptions:)` returns.
    public static func install() {
        guard !installed else { return }
        installed = true
        if let data = UserDefaults.standard.data(forKey: key), let kept = try? JSONDecoder().decode([String: Request].self, from: data) {
            requests = kept
        }
        let center = NotificationCenter.default
        center.addObserver(forName: Notification.Name("exact.wakes.request"), object: nil, queue: .main) { note in
            guard let body = note.object as? [String: Any], let package = body["package"] as? String, let k = body["key"] as? String else { return }
            let id = package + " " + k
            if let r = body["request"] as? [String: Any] {
                requests[id] = Request(package: package, key: k, kind: r["kind"] as? String ?? "refresh", at: r["at"] as? Double ?? now(),
                                       network: r["network"] as? Bool ?? true, power: r["power"] as? Bool ?? false)
            } else {
                requests[id] = nil
            }
            keep()
            schedule()
        }
        center.addObserver(forName: Notification.Name("exact.wakes.done"), object: nil, queue: .main) { note in
            guard let body = note.object as? [String: Any], let token = body["token"] as? String, let package = body["package"] as? String,
                  let a = arrivals.first(where: { $0.token == token }) else { return }
            a.waiting.remove(package)
            if body["success"] as? Bool == false { a.success = false }
            if a.waiting.isEmpty { finish(a, a.success) }
        }
        center.addObserver(forName: Notification.Name("exact.wakes.ready"), object: nil, queue: .main) { _ in
            for a in arrivals where !a.offered { offer(a) }
            if !readied { readied = true; TransferCarrier.ready() }
            RegionCarrier.ready()
        }
        TransferCarrier.install()
        RegionCarrier.install()
        #if os(iOS)
        for suffix in ["refresh", "processing.network", "processing.network-power"] where permitted(suffix) {
            let registered = BGTaskScheduler.shared.register(forTaskWithIdentifier: "\(bundle).\(suffix)", using: .main) { task in
                arrived(task, suffix)
            }
            if !registered { WakeStore.log("wakes: \(bundle).\(suffix) was not registered") }
        }
        center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { _ in schedule() }
        #endif
        schedule()
    }

    private static func keep() {
        if let data = try? JSONEncoder().encode(requests) { UserDefaults.standard.set(data, forKey: key) }
    }

    /// The due requests of `kind` (and, for processing, of `power`), taken from the store.
    private static func take(kind: String, power: Bool?) -> [Request] {
        let at = now()
        let due = requests.filter { $0.value.kind == kind && $0.value.at <= at && (power == nil || $0.value.power == power) }
        for id in due.keys { requests[id] = nil }
        keep()
        return Array(due.values)
    }

    /// Hand `a`'s due requests to each package's implementation. None taken:
    /// no module is ready yet, so one is booted and `exact.wakes.ready` offers
    /// it again; taken by none after that, no module composes these packages,
    /// and the requests go back to the store.
    private static func offer(_ a: Arrival) {
        let packages = Dictionary(grouping: a.due, by: \.package)
        guard !packages.isEmpty else { return finish(a, true) }
        a.waiting = []
        for (package, due) in packages {
            let body = NSMutableDictionary(dictionary: ["token": a.token, "package": package, "kind": a.kind, "keys": due.map(\.key)])
            #if os(iOS)
            if let task = a.task { body["task"] = task }
            #endif
            NotificationCenter.default.post(name: Notification.Name("exact.wakes.due"), object: body)
            if body["taken"] as? Bool == true { a.waiting.insert(package) }
        }
        if !a.waiting.isEmpty { a.offered = true; return }
        if !a.booted, let boot {
            a.booted = true
            log("wakes: a \(a.kind) wake found no module ready; booting a session for it")
            return boot()
        }
        log("wakes: a \(a.kind) wake was taken by no package: \(packages.keys.sorted().joined(separator: ", "))")
        for r in a.due where r.kind != "transfer" && r.kind != "region" && requests[r.package + " " + r.key] == nil { requests[r.package + " " + r.key] = r }
        keep()
        finish(a, false)
    }

    /// Transfers' endings, by package and registration id: a wake of kind `transfer` (D10).
    static func transfer(_ due: [(String, String)], complete: @escaping (Bool) -> Void) {
        let a = Arrival(kind: "transfer", due: due.map { Request(package: $0.0, key: $0.1, kind: "transfer", at: now(), network: false, power: false) },
                        complete: complete)
        arrivals.append(a)
        offer(a)
    }

    /// Regions' crossings, by package and region name: a wake of kind `region` (LLP 1133.021 D14).
    static func region(_ due: [(String, String)], complete: @escaping (Bool) -> Void) {
        let a = Arrival(kind: "region", due: due.map { Request(package: $0.0, key: $0.1, kind: "region", at: now(), network: false, power: false) },
                        complete: complete)
        arrivals.append(a)
        offer(a)
    }

    private static func finish(_ a: Arrival, _ success: Bool) {
        guard let i = arrivals.firstIndex(where: { $0 === a }) else { return }
        arrivals.remove(at: i)
        a.complete(success)
        schedule()
    }

    #if os(iOS)
    private static func permitted(_ suffix: String) -> Bool {
        let ids = Bundle.main.object(forInfoDictionaryKey: "BGTaskSchedulerPermittedIdentifiers") as? [String] ?? []
        return ids.contains("\(bundle).\(suffix)")
    }

    private static func arrived(_ task: BGTask, _ suffix: String) {
        let due = suffix == "refresh" ? take(kind: "refresh", power: nil) : take(kind: "processing", power: suffix.hasSuffix("power"))
        let a = Arrival(kind: suffix == "refresh" ? "refresh" : "processing", due: due) { success in task.setTaskCompleted(success: success) }
        a.task = task
        // The platform's time is up: each package's events still running
        // expire (`exact.wakes.expired`, ExactWake), which ends their
        // invocations and fails their requests, and the packages are done in
        // a turn or two. The task is completed then, or after a second
        // regardless, since iOS terminates an app that holds an expired task.
        task.expirationHandler = { [weak a] in
            DispatchQueue.main.async {
                guard let a, arrivals.contains(where: { $0 === a }) else { return }
                WakeStore.log("wakes: \(task.identifier) expired")
                a.success = false
                NotificationCenter.default.post(name: Notification.Name("exact.wakes.expired"), object: ["token": a.token] as NSDictionary)
                DispatchQueue.main.asyncAfter(deadline: .now() + 1) { finish(a, false) }
            }
        }
        WakeStore.log("wakes: \(task.identifier): \(due.count) due")
        arrivals.append(a)
        offer(a)
    }

    /// One request an identifier: at the soonest due time of the requests it carries, or none.
    public static func schedule() {
        let all = Array(requests.values)
        let soonest = { (match: (Request) -> Bool) in all.filter(match).map(\.at).min() }
        submit("refresh", at: soonest { $0.kind == "refresh" || !permitted("processing.network") && $0.kind == "processing" }) { BGAppRefreshTaskRequest(identifier: $0) }
        for power in [false, true] {
            let suffix = power ? "processing.network-power" : "processing.network"
            submit(suffix, at: soonest { $0.kind == "processing" && $0.power == power }) { id in
                let r = BGProcessingTaskRequest(identifier: id)
                r.requiresNetworkConnectivity = true
                r.requiresExternalPower = power
                return r
            }
        }
    }

    private static func submit(_ suffix: String, at: Double?, _ make: (String) -> BGTaskRequest) {
        guard permitted(suffix) else { return }
        let id = "\(bundle).\(suffix)"
        guard let at else { return BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: id) }
        let request = make(id)
        request.earliestBeginDate = Date(timeIntervalSince1970: at / 1000)
        do { try BGTaskScheduler.shared.submit(request) } catch { WakeStore.log("wakes: \(id) not submitted: \(error)") }
    }
    #endif

    #if os(macOS)
    /// One scheduler a request, non-repeating. A scheduler runs its block
    /// from `interval - tolerance` on (seen: 628 s into a 900 s interval with
    /// 450 s of tolerance), so the window is the request's time to half
    /// again as long.
    public static func schedule() {
        for (id, (s, at)) in schedulers where requests[id]?.at != at { s.invalidate(); schedulers[id] = nil }
        for (id, r) in requests where schedulers[id] == nil {
            let s = NSBackgroundActivityScheduler(identifier: "\(bundle).wake.\(r.package).\(r.key)")
            s.repeats = false
            let wait = max(1, (r.at - now()) / 1000)
            s.interval = wait * 1.5
            s.tolerance = wait / 2
            s.schedule { done in
                DispatchQueue.main.async {
                    schedulers[id] = nil
                    guard let r = requests.removeValue(forKey: id) else { return done(.finished) }
                    keep()
                    let a = Arrival(kind: r.kind, due: [r]) { _ in done(.finished) }
                    arrivals.append(a)
                    offer(a)
                }
            }
            schedulers[id] = (s, r.at)
        }
    }
    #endif

    #if !os(iOS) && !os(macOS)
    /// tvOS's refresh is later (D17): its requests are kept, never submitted.
    public static func schedule() {}
    #endif
}

extension ExactApp {
    /// Install at launch, before the app finishes launching (LLP 1133.017
    /// D8). `boot` boots a session with no view for a wake that launched the
    /// app with none (D7).
    public func installWakes(boot: (() -> Void)? = nil) {
        WakeStore.boot = boot
        WakeStore.install()
    }
}
