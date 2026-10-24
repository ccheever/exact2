// @exact/background on Apple (LLP 1133.017 D2, D8, D17): the registrations
// kept in `UserDefaults`, a `sync` fired while the path is satisfied
// (`NWPathMonitor`), a `periodicsync` by `NSBackgroundActivityScheduler` on
// the Mac and by the host's one `BGAppRefreshTask` on iOS. The app's composed
// module calls it outside the agent; under the agent the runner's substitute
// answers, unless the drive binds it (`--bind background`).
//
// - Each tag fired is its own wake, raised through `context.wake`; its end
//   is `woke(id, outcome)`, which completes the platform's task.
// - Retrying is D1's: a handler that registers its tag again keeps it,
//   eligible after 5 minutes, doubling to an hour; the third attempt in a row
//   is `lastChance`, after which it is retired unless registered again.
// - iOS has no wake for the network's return: a `sync` registered offline
//   fires at the next refresh wake or launch (D2). The host registers the
//   refresh identifier at launch and offers each task here (ExactKit's
//   BackgroundRefresh.swift, D8); one that arrives with no session booted
//   ends unsuccessful at its expiry, since the headless boot is not built.
import ExactPackage
import Foundation
import Network
#if os(iOS)
import BackgroundTasks
import UIKit
#endif

public final class BackgroundModule: NSObject, ExactPackage {
    private struct Registration: Codable {
        var registered: Double
        var attempts = 0
        var notBefore: Double?
        var minInterval: Double?
        var lastRun: Double?
    }

    private static let backoff = 5 * 60_000.0, longest = 60 * 60_000.0, attempts = 3
    private static let key = "exact.background"

    private let context: ExactPackageContext
    /// By kind (`sync`, `periodicsync`), then tag.
    private var tags: [String: [String: Registration]] = [:]
    /// Wakes raised and not ended: the wake's id → (kind, tag).
    private var raised: [String: (kind: String, tag: String)] = [:]
    /// Tags registered again during their own invocation.
    private var again: Set<String> = []
    private var satisfied = false
    private let monitor = NWPathMonitor()
    private var timer: Timer?
    #if os(macOS)
    private var schedulers: [String: NSBackgroundActivityScheduler] = [:]
    private var finishing: [String: (NSBackgroundActivityScheduler.Result) -> Void] = [:]
    #endif
    #if os(iOS)
    private var task: BGTask?
    #endif

    public required init(context: ExactPackageContext) {
        self.context = context
        super.init()
        if let data = UserDefaults.standard.data(forKey: Self.key),
           let kept = try? JSONDecoder().decode([String: [String: Registration]].self, from: data) {
            tags = kept
        }
        guard !context.agent else { return }
        monitor.pathUpdateHandler = { [weak self] path in
            DispatchQueue.main.async { self?.path(path.status == .satisfied) }
        }
        monitor.start(queue: .main)
        #if os(macOS)
        for (tag, r) in tags["periodicsync"] ?? [:] { schedule(tag, r.minInterval ?? 900_000) }
        #endif
        #if os(iOS)
        NotificationCenter.default.addObserver(forName: Self.offered, object: nil, queue: .main) { [weak self] note in
            guard let self, self.task == nil, let offer = note.object as? NSMutableDictionary,
                  offer["taken"] == nil, let task = offer["task"] as? BGTask else { return }
            offer["taken"] = true
            self.refresh(task)
        }
        NotificationCenter.default.post(name: Self.ask, object: nil)
        #endif
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            let tag = args["tag"] as? String ?? ""
            let bad = tag.isEmpty || tag.utf8.count > 64
            switch name {
            case "registerSync":
                if bad { return reply(["error": "TypeError"]) }
                if denied { return reply(["error": "NotAllowedError"]) }
                register("sync", tag, interval: nil)
                reply([:])
            case "registerPeriodicSync":
                if bad { return reply(["error": "TypeError"]) }
                if denied { return reply(["error": "NotAllowedError"]) }
                register("periodicsync", tag, interval: max(args["minInterval"] as? Double ?? 900_000, 900_000))
                reply([:])
            case "unregisterPeriodicSync":
                tags["periodicsync"]?[tag] = nil
                #if os(macOS)
                schedulers.removeValue(forKey: tag)?.invalidate()
                #endif
                keep("periodicsync")
                reply([:])
            case "syncTags": reply(["exact:answer": listed("sync")])
            case "periodicSyncTags": reply(["exact:answer": listed("periodicsync")])
            case "backgroundSyncStatus": reply(status())
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    // MARK: the fact

    private var denied: Bool { status()["state"] as? String == "denied" }

    private func status() -> [String: Any] {
        #if os(iOS)
        let state = UIApplication.shared.backgroundRefreshStatus == .available ? "granted" : "denied"
        #else
        let state = "granted"
        #endif
        return ["available": true, "state": state, "periodic": true, "sync": true]
    }

    private func listed(_ kind: String) -> [[String: Any]] {
        (tags[kind] ?? [:]).sorted { $0.key < $1.key }.map { tag, r in
            ["tag": tag, "minInterval": r.minInterval.map { $0 as Any } ?? NSNull(), "lastRun": r.lastRun.map { $0 as Any } ?? NSNull()]
        }
    }

    // MARK: registrations

    private func register(_ kind: String, _ tag: String, interval: Double?) {
        if raised.values.contains(where: { $0 == (kind, tag) }) { again.insert(kind + " " + tag) }
        var r = tags[kind]?[tag] ?? Registration(registered: context.now())
        r.minInterval = interval
        tags[kind, default: [:]][tag] = r
        keep(kind)
        #if os(macOS)
        if kind == "periodicsync" { schedule(tag, interval ?? 900_000) }
        #endif
        #if os(iOS)
        submit()
        #endif
        fire()
    }

    private func keep(_ kind: String) {
        if let data = try? JSONEncoder().encode(tags) { UserDefaults.standard.set(data, forKey: Self.key) }
        context.changed(kind == "sync" ? "background.syncTags" : "background.periodicSyncTags")
    }

    private func path(_ ok: Bool) {
        satisfied = ok
        if ok { fire() }
    }

    /// Each `sync` tag eligible while the path is satisfied, as its own wake; a look again at the soonest backoff.
    private func fire() {
        guard satisfied, !context.agent else { return }
        timer?.invalidate()
        let now = context.now()
        var soonest = Double.infinity
        for (tag, r) in tags["sync"] ?? [:] where !raised.values.contains(where: { $0 == ("sync", tag) }) {
            if (r.notBefore ?? 0) > now { soonest = min(soonest, r.notBefore!); continue }
            let lastChance = r.attempts + 1 >= Self.attempts
            raise("sync", tag, ["tag": tag, "lastChance": lastChance], id: "sync \(tag) \(r.attempts)")
        }
        if soonest < .infinity {
            timer = Timer.scheduledTimer(withTimeInterval: (soonest - now) / 1000, repeats: false) { [weak self] _ in self?.fire() }
        }
    }

    private func raise(_ kind: String, _ tag: String, _ event: [String: Any]?, id: String) {
        raised[id] = (kind, tag)
        again.remove(kind + " " + tag)
        context.wake(kind, event.map { [tag, $0] } ?? [tag], id: id)
    }

    public func woke(_ id: String, _ outcome: String) {
        DispatchQueue.main.async { [self] in
            guard let (kind, tag) = raised.removeValue(forKey: id) else { return }
            let registered = again.remove(kind + " " + tag) != nil
            ended(kind, tag, outcome, registered: registered)
            #if os(macOS)
            finishing.removeValue(forKey: id)?(.finished)
            #endif
            #if os(iOS)
            if raised.isEmpty, let task { self.task = nil; task.setTaskCompleted(success: outcome == "completed"); submit() }
            #endif
            fire()
        }
    }

    /// D1's rules for an invocation's end.
    private func ended(_ kind: String, _ tag: String, _ outcome: String, registered: Bool) {
        guard var r = tags[kind]?[tag] else { return }
        if kind == "periodicsync" {
            if outcome == "completed" { r.lastRun = context.now(); r.attempts = 0 }
            tags[kind]?[tag] = r
            return keep(kind)
        }
        if outcome == "completed" && !registered || outcome == "unheard" {
            tags[kind]?[tag] = nil
            context.log("background.sync \(tag): retired (\(outcome))")
            return keep(kind)
        }
        r.attempts += 1
        if r.attempts >= Self.attempts {
            guard registered else { tags[kind]?[tag] = nil; return keep(kind) }
            r.attempts = 0
            r.notBefore = context.now() + Self.longest
        } else {
            r.notBefore = context.now() + min(Self.backoff * pow(2, Double(r.attempts - 1)), Self.longest)
        }
        tags[kind]?[tag] = r
        keep(kind)
    }

    // MARK: the Mac's scheduler

    #if os(macOS)
    private func schedule(_ tag: String, _ interval: Double) {
        guard !context.agent else { return }
        schedulers.removeValue(forKey: tag)?.invalidate()
        let s = NSBackgroundActivityScheduler(identifier: "\(Bundle.main.bundleIdentifier ?? "app").periodicsync.\(tag)")
        s.repeats = true
        s.interval = interval / 1000
        s.schedule { [weak self] done in
            DispatchQueue.main.async {
                guard let self, self.satisfied, self.tags["periodicsync"]?[tag] != nil else { return done(.deferred) }
                let id = "periodicsync \(tag) \(self.context.now())"
                self.finishing[id] = done
                self.raise("periodicsync", tag, nil, id: id)
            }
        }
        schedulers[tag] = s
    }
    #endif

    // MARK: iOS's refresh task

    #if os(iOS)
    /// The host's offer of a refresh task, and a module's ask for one offered before it was made.
    static let offered = Notification.Name("exact.background.refresh"), ask = Notification.Name("exact.background.ask")

    /// A refresh wake: every due periodic tag and every eligible sync tag.
    private func refresh(_ task: BGTask) {
        self.task = task
        let now = context.now()
        for (tag, r) in tags["periodicsync"] ?? [:] where now >= (r.lastRun ?? r.registered) + (r.minInterval ?? 900_000) {
            raise("periodicsync", tag, nil, id: "periodicsync \(tag) \(now)")
        }
        fire()
        if raised.isEmpty { self.task = nil; task.setTaskCompleted(success: true); submit() }
    }

    /// The one refresh request, at the soonest due time of a periodic tag, or at once for a sync tag waiting.
    private func submit() {
        guard !context.agent else { return }
        let id = "\(Bundle.main.bundleIdentifier ?? "app").refresh"
        let due = (tags["periodicsync"] ?? [:]).values.map { ($0.lastRun ?? $0.registered) + ($0.minInterval ?? 900_000) }
        let soonest = (tags["sync"]?.isEmpty == false) ? context.now() : due.min()
        guard let soonest else { return BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: id) }
        let request = BGAppRefreshTaskRequest(identifier: id)
        request.earliestBeginDate = Date(timeIntervalSince1970: soonest / 1000)
        do { try BGTaskScheduler.shared.submit(request) } catch { context.log("background: refresh not submitted: \(error)") }
    }
    #endif
}
