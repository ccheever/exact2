// @exact/background on Apple (LLP 1133.017 D2, D8, D12, D17): the
// registrations kept in `UserDefaults`; a `sync` fired while the path is
// satisfied (`NWPathMonitor`) and the app runs; every other run asked of the
// host's wake requests (`context.wakes`), which the host submits as iOS's
// refresh and processing tasks and the Mac's schedulers. The app's composed
// module calls it outside the agent; under the agent the runner's substitute
// answers, unless the drive binds it (`--bind background`).
//
// - Each tag fired is its own invocation, raised through `context.wake` or a
//   host wake's `raise`, and told its outcome.
// - Retrying is D1's: a handler that registers its tag again keeps it,
//   eligible after 5 minutes, doubling to an hour; the third attempt in a row
//   is `lastChance`, after which it is retired unless registered again.
// - iOS has no wake for the network's return: a `sync` waiting asks for the
//   next refresh wake and a processing wake with a network connection, and
//   fires at whichever comes first, or at the next launch (D2).
// - `periodicsync` asks for a wake at its due time, the refresh kind unless
//   `apple.task` is `processing`; `apple.requiresExternalPower` asks a
//   processing wake for power.
import ExactPackage
import Foundation
import Network
#if os(iOS)
import UIKit
#endif

public final class BackgroundModule: NSObject, ExactPackage {
    private struct Registration: Codable {
        var registered: Double
        var attempts = 0
        var notBefore: Double?
        var minInterval: Double?
        var lastRun: Double?
        var task: String?
        var power: Bool?
    }

    private static let backoff = 5 * 60_000.0, longest = 60 * 60_000.0, attempts = 3
    private static let key = "exact.background"

    private let context: ExactPackageContext
    private let epochAtZero: Double
    /// By kind (`sync`, `periodicsync`), then tag.
    private var tags: [String: [String: Registration]] = [:]
    /// Platform tasks waiting for each invocation, by `kind tag`.
    private var raised: [String: [(Bool) -> Void]] = [:]
    /// Tags registered again during their own invocation, as `kind tag`.
    private var again: Set<String> = []
    private var satisfied = false
    private let monitor = NWPathMonitor()
    private var timer: Timer?

    public required init(context: ExactPackageContext) {
        self.context = context
        epochAtZero = Date().timeIntervalSince1970 * 1000 - context.now()
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
        ask()
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            let tag = args["tag"] as? String ?? ""
            let bad = tag.isEmpty || tag.utf8.count > 64
            let apple = args["apple"] as? [String: Any] ?? [:]
            switch name {
            case "registerSync":
                if bad { return reply(["error": "TypeError"]) }
                if denied { return reply(["error": "NotAllowedError"]) }
                register("sync", tag, interval: nil, task: nil, power: apple["requiresExternalPower"] as? Bool)
                reply([:])
            case "registerPeriodicSync":
                if bad { return reply(["error": "TypeError"]) }
                if denied { return reply(["error": "NotAllowedError"]) }
                let interval = max(args["minInterval"] as? Double ?? 900_000, 900_000)
                register("periodicsync", tag, interval: interval, task: apple["task"] as? String, power: apple["requiresExternalPower"] as? Bool)
                reply([:])
            case "unregisterPeriodicSync":
                tags["periodicsync"]?[tag] = nil
                keep("periodicsync")
                reply([:])
            case "syncTags": reply(["exact:answer": listed("sync")])
            case "periodicSyncTags": reply(["exact:answer": listed("periodicsync")])
            case "backgroundSyncStatus": reply(status())
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func foreground(_ shown: Bool) {
        if shown { context.changed("background.backgroundSyncStatus") }
    }

    // MARK: the fact

    private var denied: Bool { status()["state"] as? String == "denied" }

    private func status() -> [String: Any] {
        #if os(iOS)
        let refresh = UIApplication.shared.backgroundRefreshStatus
        let word = refresh == .available ? "available" : refresh == .denied ? "denied" : "restricted"
        let apple: [String: Any] = ["backgroundRefreshStatus": word, "lowPowerMode": ProcessInfo.processInfo.isLowPowerModeEnabled]
        return ["available": true, "state": refresh == .available ? "granted" : "denied", "periodic": true, "sync": true, "apple": apple]
        #else
        return ["available": true, "state": "granted", "periodic": true, "sync": true]
        #endif
    }

    private func listed(_ kind: String) -> [[String: Any]] {
        (tags[kind] ?? [:]).sorted { $0.key < $1.key }.map { tag, r in
            ["tag": tag, "minInterval": r.minInterval.map { $0 as Any } ?? NSNull(), "lastRun": r.lastRun.map { $0 as Any } ?? NSNull()]
        }
    }

    // MARK: registrations

    private func register(_ kind: String, _ tag: String, interval: Double?, task: String?, power: Bool?) {
        if raised[kind + " " + tag] != nil { again.insert(kind + " " + tag) }
        var r = tags[kind]?[tag] ?? Registration(registered: now())
        r.minInterval = interval
        r.task = task
        r.power = power
        tags[kind, default: [:]][tag] = r
        keep(kind)
        fire()
    }

    private func keep(_ kind: String) {
        if let data = try? JSONEncoder().encode(tags) { UserDefaults.standard.set(data, forKey: Self.key) }
        context.changed(kind == "sync" ? "background.syncTags" : "background.periodicSyncTags")
        ask()
    }

    /// The host's wakes this module needs now: a `sync` waiting (iOS), each periodic tag's due time.
    private func ask() {
        guard !context.agent else { return }
        let wakes = context.wakes
        #if os(iOS)
        let waiting = (tags["sync"] ?? [:]).filter { raised["sync " + $0.key] == nil }
        if let soonest = waiting.values.map({ $0.notBefore ?? 0 }).min() {
            let at = max(soonest, now()) - epochAtZero
            wakes.request(kind: "refresh", at: at, key: "sync")
            wakes.request(kind: "processing", at: at, key: "sync processing", power: waiting.values.contains { $0.power == true })
        } else {
            wakes.cancel(key: "sync")
            wakes.cancel(key: "sync processing")
        }
        #endif
        for (tag, r) in tags["periodicsync"] ?? [:] {
            wakes.request(kind: r.task == "processing" ? "processing" : "refresh", at: due(r) - epochAtZero, key: "periodicsync " + tag, power: r.power == true)
        }
    }

    // Persist epoch times: the session clock starts over on each launch.
    private func now() -> Double { epochAtZero + context.now() }

    private func due(_ r: Registration) -> Double { (r.lastRun ?? r.registered) + (r.minInterval ?? 900_000) }

    private func path(_ ok: Bool) {
        satisfied = ok
        if ok { fire() }
    }

    /// Each `sync` tag eligible while the path is satisfied, as an
    /// invocation of its own; a look again at the soonest backoff.
    private func fire() {
        guard satisfied, !context.agent else { return }
        timer?.invalidate()
        let now = now()
        var soonest = Double.infinity
        for (tag, r) in tags["sync"] ?? [:] where raised["sync " + tag] == nil {
            if let at = r.notBefore, at > now { soonest = min(soonest, at); continue }
            raise("sync", tag) { event, args, ended in self.context.wake(event, args, id: "sync \(tag) \(now)", ended: ended) }
        }
        if soonest < .infinity {
            timer = Timer.scheduledTimer(withTimeInterval: (soonest - now) / 1000, repeats: false) { [weak self] _ in self?.fire() }
        }
    }

    /// Raise `tag`'s event through `via`; `done` hears whether it completed.
    private func raise(_ kind: String, _ tag: String, done: ((Bool) -> Void)? = nil,
                       via: (String, [Any], @escaping (String) -> Void) -> Void) {
        let key = kind + " " + tag
        if raised[key] != nil {
            if let done { raised[key]?.append(done) }
            return
        }
        let r = tags[kind]?[tag]
        let args: [Any] = kind == "sync" ? [tag, ["tag": tag, "lastChance": (r?.attempts ?? 0) + 1 >= Self.attempts]] : [tag]
        raised[key] = done.map { [$0] } ?? []
        again.remove(kind + " " + tag)
        via(kind, args) { [weak self] outcome in
            DispatchQueue.main.async {
                guard let self else { return }
                let waiting = self.raised.removeValue(forKey: key) ?? []
                let registered = self.again.remove(kind + " " + tag) != nil
                self.ended(kind, tag, outcome, registered: registered)
                waiting.forEach { $0(outcome == "completed") }
                self.fire()
            }
        }
    }

    /// The host ran the app for due requests: each tag they cover, then done.
    public func woke(_ wake: ExactWake) {
        DispatchQueue.main.async { [self] in
            let now = now()
            var due: [(String, String)] = []
            for key in wake.keys {
                if key.hasPrefix("sync") {
                    due += (tags["sync"] ?? [:]).filter { ($0.value.notBefore ?? 0) <= now }.keys.map { ("sync", $0) }
                } else if key.hasPrefix("periodicsync "), let tag = Optional(String(key.dropFirst(13))), tags["periodicsync"]?[tag] != nil {
                    due.append(("periodicsync", tag))
                }
            }
            var unique: [(String, String)] = []
            for d in due where !unique.contains(where: { $0 == d }) { unique.append(d) }
            context.log("background: a \(wake.kind) wake: \(unique.map { "\($0.0) \($0.1)" }.joined(separator: ", "))")
            guard !unique.isEmpty else { ask(); return wake.done(success: true) }
            var left = unique.count, success = true
            for (kind, tag) in unique {
                raise(kind, tag, done: { completed in
                    success = success && completed
                    left -= 1
                    if left == 0 { wake.done(success: success) }
                }, via: { event, args, ended in wake.raise(event, args, ended: ended) })
            }
        }
    }

    /// D1's rules for an invocation's end.
    private func ended(_ kind: String, _ tag: String, _ outcome: String, registered: Bool) {
        guard var r = tags[kind]?[tag] else { return }
        if kind == "periodicsync" {
            if outcome == "completed" { r.lastRun = now(); r.attempts = 0 }
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
            r.notBefore = now() + Self.longest
        } else {
            r.notBefore = now() + min(Self.backoff * pow(2, Double(r.attempts - 1)), Self.longest)
        }
        tags[kind]?[tag] = r
        keep(kind)
    }
}
