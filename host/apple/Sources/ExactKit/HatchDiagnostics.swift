// What Exact measures of each hatch call, and what hatch code says of itself
// (@ref LLP 1075.003.000.001 §3.1–3.3). One store a session, owned by the
// main thread. Development only, as `perf` is (LLP 1079 D1): a production
// bake hands the module no entry to call and times nothing.
//
// The module records through one entry of the host's table (NativeHatches.swift):
//
//   record(host, kind, node, scope, scopeLen, name, nameLen, value, text, textLen) → u64
//     kind 0 log (text)        1 count (value: by)      2 measure (value: ms)
//          3 begin → span id   4 end (value: span id)   5 publish (text: JSON)
//          6 saveTrace
//
// `scope` is `module` or `element <word>`; `node` is the element's node, 0
// for the module, so a node's open spans end with it. Any thread may call:
// off the main thread a count is added to a pending sum and a snapshot
// replaces a pending one, both exact, and the rest go through a ring of 1,024
// records that overwrites its oldest; the main thread drains them each turn.
import CExact
import Foundation
import QuartzCore

final class HatchDiagnostics {
    weak var session: ExactSession?
    static let measuring = !ExactEnv.productionBake

    // The live sessions' stores by runtime handle, for a call on any thread
    // (`ExactSession.session(for:)` is the main thread's).
    private final class Weak { weak var store: HatchDiagnostics?; init(_ s: HatchDiagnostics) { store = s } }
    private static let storesLock = NSLock()
    private static var stores: [ExactRuntime: Weak] = [:]
    /// The session whose store this is: its clock, its journal, its handle.
    /// An entry whose store has gone is dropped as the next one is added.
    func attach(_ session: ExactSession?) {
        self.session = session
        guard let rt = session?.runtime.rt else { return }
        Self.storesLock.lock(); defer { Self.storesLock.unlock() }
        Self.stores = Self.stores.filter { $0.value.store != nil }
        Self.stores[rt] = Weak(self)
    }
    static func store(for rt: ExactRuntime) -> HatchDiagnostics? {
        storesLock.lock(); defer { storesLock.unlock() }
        return stores[rt]?.store
    }

    // §3.2's bounds, fixed.
    static let names = 64, samples = 256, openSpans = 32, snapshotNames = 16, snapshotBytes = 4096
    static let lineBytes = 256, linesPerSecond = 20, ringRecords = 1024, recentCalls = 1024, replyBytes = 65536

    // MARK: What Exact times (§3.1)

    private struct Timed { let hatch: String, site: Int?, moment: String; var calls = 0, ms = 0.0, worst = 0.0 }
    private var timing: [String: Timed] = [:]
    private var timingOrder: [String] = []
    /// The time spent in calls nested in each open one: a call's time is its own.
    private var inner: [Double] = []
    /// The last 1,024 calls (start and end on the media clock, the hatch), for
    /// a late frame's join.
    private(set) var recent: [(start: Double, end: Double, hatch: String)] = []
    private var recentAt = 0
    /// The calls of the scopes that are not nodes, by moment.
    private var scopeCalls: [String: [String: Int]] = [:]

    /// One hatch call, timed. `scope` is `element <word>` or a container's
    /// name; a node's calls are counted by ElementHatches, a container's here.
    func timed<T>(_ scope: String, _ moment: String, site: Int? = nil, counts: Bool = true, _ body: () -> T) -> T {
        guard Self.measuring else { return body() }
        if counts { scopeCalls[scope, default: [:]][moment, default: 0] += 1 }
        inner.append(0)
        let start = CACurrentMediaTime()
        let out = body()
        let end = CACurrentMediaTime(), whole = (end - start) * 1000, own = max(0, whole - inner.removeLast())
        if !inner.isEmpty { inner[inner.count - 1] += whole }
        let key = "\(scope)\n\(site.map(String.init) ?? "")\n\(moment)"
        if timing[key] == nil { timing[key] = Timed(hatch: scope, site: site, moment: moment); timingOrder.append(key) }
        timing[key]!.calls += 1
        timing[key]!.ms += own
        timing[key]!.worst = max(timing[key]!.worst, own)
        if recent.count < Self.recentCalls { recent.append((start, end, scope)) } else {
            recent[recentAt] = (start, end, scope)
            recentAt = (recentAt + 1) % Self.recentCalls
        }
        return out
    }

    /// A late frame's `hatches` (§3.1): the calls that overlapped its window
    /// `(from, to]`, seconds on the media clock, each charged its overlap;
    /// `coverage: partial` when the window reaches past the oldest call kept.
    /// Nil when none did. A join by window, not a causal trace.
    func window(from: Double, to: Double) -> [String: Any]? {
        var by: [String: (calls: Int, ms: Double)] = [:], order: [String] = []
        for call in recent where call.end > from && call.start <= to {
            if by[call.hatch] == nil { order.append(call.hatch) }
            by[call.hatch, default: (0, 0)].calls += 1
            by[call.hatch]!.ms += max(0, min(call.end, to) - max(call.start, from)) * 1000
        }
        guard !order.isEmpty else { return nil }
        var out: [String: Any] = ["hatches": order.map { ["hatch": $0, "calls": by[$0]!.calls, "ms": (by[$0]!.ms * 100).rounded() / 100] as [String: Any] }]
        if recent.count == Self.recentCalls, let oldest = recent.map(\.start).min(), oldest > from { out["coverage"] = "partial" }
        return out
    }

    // MARK: What hatch code records (§3.2)

    private struct Timing { var count = 0, sum = 0.0, max = 0.0, ring: [Double] = [], at = 0, dropped = 0, measured = false }
    private struct Span { let scope: String, name: String, from: Double, node: UInt32 }
    private struct Record { let kind: UInt32, node: UInt32, scope: String, name: String, value: Double, text: Data }
    private var counters: [String: UInt64] = [:], counterOrder: [String] = []
    private var timings: [String: Timing] = [:], timingNames: [String] = []
    private var snapshots: [String: Data] = [:], snapshotOrder: [String] = []
    private var spans: [UInt64: Span] = [:]
    private var buckets: [String: (from: Double, lines: Int, more: Int)] = [:]
    private var badNames: Set<String> = []
    private var rejected = 0, abandoned = 0, limited = 0
    private var lastTrace: Double?, tracePending = false

    // What other threads left for the main thread, under `lock`.
    private let lock = NSLock()
    private var nextSpan: UInt64 = 1
    private var pending: [Record] = []
    private var pendingCounts: [String: (scope: String, name: String, by: UInt64)] = [:]
    private var pendingSnapshots: [String: Record] = [:]
    private var dropped = 0, lateRejected = 0, draining = false

    /// The module's call, on any thread. A span's id for `begin`, else 0.
    func record(kind: UInt32, node: UInt32, scope: String, name: String, value: Double, text: Data) -> UInt64 {
        guard Self.measuring else { return 0 }
        var id: UInt64 = 0
        if kind == 3 { lock.lock(); id = nextSpan; nextSpan += 1; lock.unlock() }
        let record = Record(kind: kind, node: node, scope: scope, name: name, value: kind == 3 ? Double(id) : value, text: text)
        if Thread.isMainThread {
            drain()
            apply(record)
            return id
        }
        lock.lock()
        let key = "\(scope)\n\(name)"
        switch kind {
        case 1 where value >= 0 && value <= 9_007_199_254_740_992 && value == value.rounded() && Self.valid(name):
            if let had = pendingCounts[key] { pendingCounts[key] = (scope, name, had.by + UInt64(value)) }
            else if pendingCounts.count < Self.names { pendingCounts[key] = (scope, name, UInt64(value)) }
            else { dropped += 1 }
        case 5 where Self.valid(name):
            // The whole snapshot or none of it: never cut, never through the ring.
            if text.count > Self.snapshotBytes || (pendingSnapshots[key] == nil && pendingSnapshots.count >= Self.snapshotNames) { lateRejected += 1 }
            else { pendingSnapshots[key] = record }
        default:
            if pending.count >= Self.ringRecords { pending.removeFirst(); dropped += 1 }
            pending.append(record)
        }
        let wake = !draining
        draining = true
        lock.unlock()
        if wake { DispatchQueue.main.async { [weak self] in self?.drain() } }
        return id
    }

    /// The main thread takes what other threads recorded, in their order.
    func drain() {
        lock.lock()
        let records = pending, counts = pendingCounts, shots = pendingSnapshots
        pending = []; pendingCounts = [:]; pendingSnapshots = [:]
        rejected += lateRejected
        lateRejected = 0
        draining = false
        lock.unlock()
        for (_, c) in counts.sorted(by: { $0.key < $1.key }) { apply(Record(kind: 1, node: 0, scope: c.scope, name: c.name, value: Double(c.by), text: Data())) }
        for (_, shot) in shots.sorted(by: { $0.key < $1.key }) { apply(shot) }
        for record in records { apply(record) }
    }

    private static func valid(_ name: String) -> Bool {
        let bytes = name.utf8
        return !bytes.isEmpty && bytes.count <= 64 && bytes.allSatisfy { ($0 >= 97 && $0 <= 122) || ($0 >= 48 && $0 <= 57) || $0 == 45 || $0 == 46 }
    }

    /// `text` in at most `max` bytes of UTF-8, cut at a character with `…`.
    private static func cut(_ text: String, _ max: Int) -> String {
        guard text.utf8.count > max else { return text }
        var out = "", size = 0
        for ch in text {
            let n = String(ch).utf8.count
            if size + n > max - 3 { break }
            out.append(ch)
            size += n
        }
        return out + "…"
    }

    private func say(_ line: String) { session?.log("hatch \(line)") }

    private func named(_ r: Record) -> Bool {
        if Self.valid(r.name) { return true }
        rejected += 1
        let shown = Self.cut(r.name, 64)
        if badNames.count < 8, badNames.insert(shown).inserted {
            let quoted = (try? JSONSerialization.data(withJSONObject: shown, options: .fragmentsAllowed)).map { String(decoding: $0, as: UTF8.self) } ?? "\"\""
            say("\(r.scope): diagnostics refused the name \(quoted) (lowercase letters, digits, \"-\" and \".\", at most 64 bytes)")
        }
        return false
    }

    private func sample(_ scope: String, _ name: String, _ ms: Double, measured: Bool) {
        guard ms >= 0, ms.isFinite else { rejected += 1; return }
        let key = "\(scope)\n\(name)"
        if timings[key] == nil {
            guard timings.count < Self.names else { rejected += 1; return }
            timings[key] = Timing()
            timingNames.append(key)
        }
        var t = timings[key]!
        t.count += 1; t.sum += ms; t.max = max(t.max, ms)
        if measured { t.measured = true }
        if t.ring.count < Self.samples { t.ring.append(ms) } else { t.ring[t.at] = ms; t.at = (t.at + 1) % Self.samples; t.dropped += 1 }
        timings[key] = t
    }

    private func apply(_ r: Record) {
        let now = session?.now() ?? 0
        switch r.kind {
        case 0:
            // 20 lines a second of session clock a scope, then one line for the rest.
            var b = buckets[r.scope] ?? (from: now, lines: 0, more: 0)
            if now - b.from >= 1000 || now < b.from {
                if b.more > 0 { say("\(r.scope): … \(b.more) more") }
                b = (from: now, lines: 0, more: 0)
            }
            if b.lines >= Self.linesPerSecond { b.more += 1; limited += 1 } else {
                b.lines += 1
                say("\(r.scope): \(Self.cut(String(decoding: r.text, as: UTF8.self), Self.lineBytes))")
            }
            buckets[r.scope] = b
        case 1:
            guard named(r) else { return }
            guard r.value >= 0, r.value <= 9_007_199_254_740_992, r.value == r.value.rounded() else { rejected += 1; return }
            let key = "\(r.scope)\n\(r.name)"
            if counters[key] == nil {
                guard counters.count < Self.names else { rejected += 1; return }
                counterOrder.append(key)
            }
            counters[key, default: 0] += UInt64(r.value)
        case 2:
            if named(r) { sample(r.scope, r.name, r.value, measured: true) }
        case 3:
            guard named(r) else { return }
            guard spans.count < Self.openSpans else { rejected += 1; return }
            spans[UInt64(r.value)] = Span(scope: r.scope, name: r.name, from: now, node: r.node)
        case 4:
            // An id the store does not hold is an inert span's, or one ended already.
            guard r.value >= 0, let span = spans.removeValue(forKey: UInt64(r.value)) else { return }
            sample(span.scope, span.name, max(0, now - span.from), measured: false)
        case 5:
            guard named(r) else { return }
            let key = "\(r.scope)\n\(r.name)"
            guard r.text.count <= Self.snapshotBytes, (try? JSONSerialization.jsonObject(with: r.text, options: .fragmentsAllowed)) != nil,
                  snapshots[key] != nil || snapshots.count < Self.snapshotNames else { rejected += 1; return }
            if snapshots[key] == nil { snapshotOrder.append(key) }
            snapshots[key] = r.text
        case 6:
            // At most one a second of wall time; a sooner ask joins the pending one.
            let wall = CACurrentMediaTime()
            if let last = lastTrace, wall - last < 1 {
                guard !tracePending else { return }
                tracePending = true
                DispatchQueue.main.asyncAfter(deadline: .now() + (1 - (wall - last))) { [weak self] in
                    guard let self else { return }
                    self.tracePending = false
                    self.lastTrace = CACurrentMediaTime()
                    _ = self.session?.saveTrace()
                }
            } else {
                lastTrace = wall
                _ = session?.saveTrace()
            }
        default: rejected += 1
        }
    }

    /// A node ended: its open spans are closed as abandoned, counted, not timed.
    func ended(node: UInt32) {
        guard Self.measuring, !spans.isEmpty else { return }
        for (id, span) in spans where span.node == node && node != 0 {
            spans.removeValue(forKey: id)
            abandoned += 1
        }
    }

    /// A reload or a new module: a new incarnation's counts start at nothing.
    func reset() {
        drain()
        timing = [:]; timingOrder = []; inner = []; recent = []; recentAt = 0; scopeCalls = [:]
        counters = [:]; counterOrder = []; timings = [:]; timingNames = []; snapshots = [:]; snapshotOrder = []
        spans = [:]; buckets = [:]; badNames = []; rejected = 0; abandoned = 0; limited = 0
        lock.lock(); dropped = 0; lock.unlock()
    }

    // MARK: Reads, which change nothing (§3.3)

    private func grouped<V>(_ order: [String], _ value: (String) -> V) -> [String: [String: V]] {
        var out: [String: [String: V]] = [:]
        for key in order {
            let parts = key.split(separator: "\n", maxSplits: 1).map(String.init)
            if parts.count == 2 { out[parts[0], default: [:]][parts[1]] = value(key) }
        }
        return out
    }

    private var refusals: [String: Any] {
        lock.lock(); let d = dropped; lock.unlock()
        return ["measuring": Self.measuring, "rejected": rejected, "abandoned": abandoned, "limited": limited, "dropped": d]
    }

    /// A reply of at most 64 KB: the longest of `lists` is halved until it fits.
    private static func fit(_ reply: [String: Any], _ lists: [String]) -> [String: Any] {
        var reply = reply
        let size = { (v: Any) in (try? JSONSerialization.data(withJSONObject: v, options: .fragmentsAllowed))?.count ?? 0 }
        while size(reply) > replyBytes {
            guard let longest = lists.filter({ ((reply[$0] as? [Any])?.count ?? (reply[$0] as? [String: Any])?.count ?? 0) > 0 }).max(by: { size(reply[$0]!) < size(reply[$1]!) }) else { break }
            if let list = reply[longest] as? [Any] { reply[longest] = Array(list.prefix(list.count / 2)) }
            else if let map = reply[longest] as? [String: Any] { reply[longest] = Dictionary(uniqueKeysWithValues: map.sorted { $0.key < $1.key }.prefix(map.count / 2).map { ($0.key, $0.value) }) }
            reply["truncated"] = true
        }
        return reply
    }

    /// `state.hatches`: `words` as ElementHatches counts them, each with what
    /// its hatch counted and published, and the same for the other scopes.
    func state(words: [String: Any]) -> [String: Any] {
        drain()
        let counted = grouped(counterOrder) { counters[$0] ?? 0 }
        let published = grouped(snapshotOrder) { (key: String) -> Any in snapshots[key].flatMap { try? JSONSerialization.jsonObject(with: $0, options: .fragmentsAllowed) } ?? NSNull() }
        let add = { (entry: [String: Any], scope: String) -> [String: Any] in
            var entry = entry
            if let c = counted[scope] { entry["counters"] = c }
            if let p = published[scope] { entry["published"] = p }
            return entry
        }
        var outWords: [String: Any] = [:], others: [String: Any] = [:]
        for (word, entry) in words { outWords[word] = add(entry as? [String: Any] ?? [:], "element \(word)") }
        for scope in Set(scopeCalls.keys).union(counted.keys).union(published.keys) {
            if scope.hasPrefix("element ") {
                let word = String(scope.dropFirst(8))
                if outWords[word] == nil { outWords[word] = add(["live": 0, "reusable": 0, "lost": [String](), "calls": [String: Int]()], scope) }
            } else {
                others[scope] = add(scopeCalls[scope].map { ["calls": $0] } ?? [:], scope)
            }
        }
        var reply: [String: Any] = ["words": outWords, "scopes": others]
        reply.merge(refusals) { a, _ in a }
        return Self.fit(reply, ["words", "scopes"])
    }

    /// `perf <target>`'s row for a hatched site: its hatch's calls and time.
    func site(_ site: Int) -> (calls: Int, ms: Double)? {
        let rows = timing.values.filter { $0.site == site }
        return rows.isEmpty ? nil : (rows.reduce(0) { $0 + $1.calls }, rows.reduce(0) { $0 + $1.ms })
    }

    /// The runner's `perf <target>` reply with each hatched site's row naming
    /// its hatch's calls and time, as the other hosts' rows do.
    func joined(perf reply: String) -> String {
        guard timing.values.contains(where: { $0.site != nil }),
              var perf = try? JSONSerialization.jsonObject(with: Data(reply.utf8)) as? [String: Any],
              var sites = perf["sites"] as? [[String: Any]] else { return reply }
        var found = false
        for index in sites.indices {
            guard let at = (sites[index]["site"] as? NSNumber)?.intValue, let row = site(at) else { continue }
            sites[index]["hatch"] = ["calls": row.calls, "ms": row.ms]
            found = true
        }
        guard found else { return reply }
        perf["sites"] = sites
        return (try? JSONSerialization.data(withJSONObject: perf)).map { String(decoding: $0, as: UTF8.self) } ?? reply
    }

    /// `perf hatches`: every call Exact timed, by hatch and by moment, and the
    /// hatches' counters and timings. Cumulative; a difference is two reads.
    func perf(tags: [String: Any]) -> [String: Any] {
        drain()
        var by: [String: [String: Any]] = [:]
        let calls: [[String: Any]] = timingOrder.compactMap { key in
            guard let t = timing[key] else { return nil }
            var sum = by[t.hatch] ?? ["calls": 0, "ms": 0.0, "worst": 0.0]
            sum["calls"] = (sum["calls"] as? Int ?? 0) + t.calls
            sum["ms"] = (sum["ms"] as? Double ?? 0) + t.ms
            sum["worst"] = max(sum["worst"] as? Double ?? 0, t.worst)
            by[t.hatch] = sum
            var row: [String: Any] = ["hatch": t.hatch, "moment": t.moment, "calls": t.calls, "ms": t.ms, "worst": t.worst]
            if let site = t.site { row["site"] = site }
            return row
        }
        let timed = grouped(timingNames) { (key: String) -> [String: Any] in
            guard let t = timings[key] else { return [:] }
            let sorted = t.ring.sorted()
            let q = { (f: Double) -> Double in sorted.isEmpty ? 0 : sorted[min(sorted.count - 1, Int(f * Double(sorted.count)))] }
            var out: [String: Any] = ["count": t.count, "sum": t.sum, "max": t.max, "p50": q(0.5), "p95": q(0.95), "samples": sorted.count, "dropped": t.dropped]
            if t.measured { out["measured"] = true }
            return out
        }
        var reply: [String: Any] = tags
        reply["hatches"] = by
        reply["calls"] = calls
        reply["tickets"] = session?.natives.hatchClock.liveTickets ?? 0
        reply["counters"] = grouped(counterOrder) { counters[$0] ?? 0 }
        reply["timings"] = timed
        reply.merge(refusals) { a, _ in a }
        return Self.fit(reply, ["calls", "hatches", "counters", "timings"])
    }
}

extension ExactSession {
    var hatchDiagnostics: HatchDiagnostics { natives.hatchDiagnostics }

    /// `perf hatches` with `perf`'s own tags (`seq`, `plan`, `incarnation`,
    /// `clock`), read from the runner as Save Trace reads them.
    func hatchPerf() -> [String: Any] {
        let perf = (try? JSONSerialization.jsonObject(with: Data(agent("{\"op\":\"perf\"}").utf8))) as? [String: Any] ?? [:]
        var tags: [String: Any] = [:]
        for key in ["seq", "plan", "incarnation", "clock", "epoch"] { if let v = perf[key] { tags[key] = v } }
        return hatchDiagnostics.perf(tags: tags)
    }
}
