import Foundation

/// Holds the snapshot answer's watched topics while one of its reads is in
/// flight (lane r3-protocol). Exact asks an answer again when a topic it
/// watches changes; asking `snapshot` again while its previous answer still
/// awaits a native reply drops that reply ("a reply for an answer not in
/// flight") and the data resource keeps its old value. The TypeScript read
/// tags its requests `reader: n` and ends with `readEnd`; until then, and until
/// the read's last network reply, the topics it watches are held. Drawing the
/// read's result mounts hooks whose native views report back (`t3.status`), so
/// a short quiet window after each read keeps holding (each held topic extends
/// it, up to `quietCap`) and replays everything once. A read that stops making
/// requests (Exact let its answer go) is released after `idle` and asked again,
/// so the window never stays stale.
final class T3ReadGate: @unchecked Sendable {
    static let held: Set<String> = ["t3.status", "t3.events", "t3.fleet"]
    private let lock = NSLock()
    private let forward: (String) -> Void
    private let queue: DispatchQueue
    private let idle: TimeInterval, limit: TimeInterval, settle: TimeInterval, quietCap: TimeInterval
    private var session = ""
    private var reader = 0, ended = 0, outstanding = 0
    private var ending = false
    private var began = Date.distantPast, active = Date.distantPast
    /// After a read: hold until `quiet` (extended per held topic, never past `quietLimit`).
    private var quiet = Date.distantPast, quietLimit = Date.distantPast
    private var topics = Set<String>()
    private var timer: DispatchSourceTimer?

    init(changed: @escaping (String) -> Void, idle: TimeInterval = 1.0, limit: TimeInterval = 20, settle: TimeInterval = 0.04,
         quietCap: TimeInterval = 0.12, queue: DispatchQueue = DispatchQueue(label: "com.exact.t3code.read-gate")) {
        forward = changed; self.idle = idle; self.limit = limit; self.settle = settle; self.quietCap = quietCap; self.queue = queue
    }

    /// The `changed` every component reports through.
    func changed(_ topic: String) {
        guard Self.held.contains(topic) else { return forward(topic) }
        let now = Date()
        lock.lock()
        if reader != 0 || now < quiet {
            topics.insert(topic)
            if reader == 0 { quiet = min(quietLimit, now.addingTimeInterval(settle)) }
            lock.unlock()
            return
        }
        // Idle: ask at once, then coalesce the burst that follows into one replay,
        // so a second change cannot drop the read the first one just started.
        quiet = now.addingTimeInterval(settle); quietLimit = now.addingTimeInterval(quietCap)
        lock.unlock()
        forward(topic)
        replaySoon()
    }

    /// Notes a request; true when it was the gate's own `readEnd`, already answered through `answer`.
    func began(_ request: [String: Any], answer: ([String: Any]) -> Void) -> Bool {
        let op = request["op"] as? String ?? ""
        var release = false
        if let id = request["reader"] as? Int, id > 0 {
            let now = Date()
            lock.lock()
            let name = request["readerSession"] as? String ?? ""
            if name != session { session = name; ended = 0; reader = 0; outstanding = 0; ending = false }
            if op != "readEnd" && id > reader && id > ended {
                // A new read; a newer one supersedes a read still open.
                if reader != 0 { ended = reader }
                reader = id; outstanding = 0; ending = false; began = now; startTimer()
            }
            if id == reader {
                active = now
                if op == "readEnd" { ending = true; release = outstanding == 0 }
                if release { finish() }
            }
            lock.unlock()
        }
        guard op == "readEnd" else { return false }
        answer(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": [:]])
        if release { replaySoon() }
        return true
    }

    /// A request of the current read now waits on the network (transport or fleet).
    func sent(_ request: [String: Any]) {
        guard let id = request["reader"] as? Int, id > 0 else { return }
        lock.lock()
        if id == reader { outstanding += 1; active = Date() }
        lock.unlock()
    }

    /// The reply to a request `sent` counted.
    func answered(_ request: [String: Any]) {
        guard let id = request["reader"] as? Int, id > 0 else { return }
        lock.lock()
        var release = false
        if id == reader {
            outstanding = max(0, outstanding - 1); active = Date()
            if ending && outstanding == 0 { finish(); release = true }
        }
        lock.unlock()
        if release { replaySoon() }
    }

    var reading: Bool { lock.lock(); defer { lock.unlock() }; return reader != 0 }

    /// Under the lock.
    private func finish() {
        ended = reader; reader = 0; outstanding = 0; ending = false
        let now = Date(); quiet = now.addingTimeInterval(settle); quietLimit = now.addingTimeInterval(quietCap)
    }

    /// The read's answer completes as its last reply lands; replay once the quiet window ends.
    private func replaySoon() { queue.asyncAfter(deadline: .now() + settle) { [weak self] in self?.flush(force: false) } }

    /// Under the lock.
    private func startTimer() {
        guard timer == nil else { return }
        let source = DispatchSource.makeTimerSource(queue: queue)
        source.schedule(deadline: .now() + 0.25, repeating: 0.25)
        source.setEventHandler { [weak self] in self?.tick() }
        timer = source; source.resume()
    }

    private func tick() {
        let now = Date()
        lock.lock()
        let stalled = reader != 0 && ((outstanding == 0 && now.timeIntervalSince(active) >= idle) || now.timeIntervalSince(began) >= limit)
        if stalled { finish() }
        if reader == 0 { timer?.cancel(); timer = nil }
        lock.unlock()
        // Exact let the read go mid-way: ask again so the window shows what it applied.
        if stalled { flush(force: true) }
    }

    private func flush(force: Bool) {
        lock.lock()
        guard reader == 0 else { lock.unlock(); return }
        let wait = quiet.timeIntervalSinceNow
        if !force && wait > 0 {
            lock.unlock()
            queue.asyncAfter(deadline: .now() + wait) { [weak self] in self?.flush(force: false) }
            return
        }
        quiet = .distantPast
        var due = topics; topics.removeAll()
        lock.unlock()
        if force { due.insert("t3.status") }
        for topic in due.sorted() { forward(topic) }
    }
}
