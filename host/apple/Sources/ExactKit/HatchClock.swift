// The hatches' frame clock (@ref LLP 1075.003.000.001 §2.4): logical time,
// for behaviour that keeps step with the app. A module's `context.frames`
// ticket ticks at the instants LLP 1073's frame tasks fire, after that
// frame's tasks and timers: each presented frame on the wall, and under the
// agent the virtual display, `base + k·1000/60`. `context.after` waits on the
// session clock.
//
// Under the agent a seek stops at every hatch instant (Agent.swift
// `advanceStepped`): the runner is advanced to the instant and its batch
// applied, each due tick is called in registration order and each due
// `after` in due order, and what they asked of elements is drained there,
// act after act, with the moments those commits cause, until nothing is
// queued. So a callback sees its own instant's state, and `clock +1000` is
// ten `clock +100`. Ticks and `after`s are not commits: 4,096 of them, and
// 4,096 drained acts, are the most one agent command runs.
//
//   host table   64  frames(host, token, on)     a ticket begins or stops
//                72  after(host, token, ms)      ms < 0 cancels
//   module table 200 tick(module, kind, token, now, seq)   kind 0 tick, 1 after
import Foundation

typealias HatchTickFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt64, Double, UInt64) -> Void

final class HatchClock {
    weak var natives: NativeViews?
    var call: HatchTickFn?
    /// The last commit the session applied, for `frame.seq`.
    var seq: UInt64 = 0
    private var tickets: [UInt64] = []
    private var afters: [(due: Double, token: UInt64, order: Int)] = []
    /// The virtual display's frames after the first ticket began: `base + k·1000/60`.
    private var base = 0.0, k = 1, order = 0
    private var fires = 0, acts = 0
    private var timer: Timer?
    static let limit = 4096

    var liveTickets: Int { tickets.count }
    private var now: Double { natives?.session?.now() ?? 0 }
    private var frameDue: Double { base + Double(k) * 1000 / 60 }

    func frames(_ token: UInt64, on: Bool) {
        if on {
            guard !tickets.contains(token) else { return }
            if tickets.isEmpty { base = now; k = 1 }
            tickets.append(token)
        } else {
            tickets.removeAll { $0 == token }
        }
        natives?.session?.frames.hatches = !tickets.isEmpty
        if !tickets.isEmpty { natives?.session?.frames.run(true) }
    }

    func after(_ token: UInt64, ms: Double) {
        afters.removeAll { $0.token == token }
        if ms >= 0, ms.isFinite {
            order += 1
            afters.append((now + ms, token, order))
        }
        rearm()
    }

    /// A reload or the module's end: what was registered before it is dropped.
    func reset() {
        if !tickets.isEmpty || !afters.isEmpty {
            natives?.session?.log("hatch frames: \(tickets.count) ticket(s) and \(afters.count) after(s) from before the reload were dropped")
        }
        tickets = []; afters = []
        timer?.invalidate(); timer = nil
        natives?.session?.frames.hatches = false
    }

    /// An agent command begins: its caps start at nothing.
    func beginCommand() { fires = 0; acts = 0 }

    /// The next hatch instant, for a seek to stop at.
    var nextInstant: Double? {
        let after = afters.map(\.due).min()
        guard !tickets.isEmpty else { return after }
        return min(frameDue, after ?? .infinity)
    }

    private func tick(_ kind: UInt32, _ token: UInt64, at now: Double) -> Bool {
        fires += 1
        guard fires <= Self.limit, let natives, let instance = natives.instance, let call else { return fires <= Self.limit }
        natives.timedHatch(kind == 0 ? "frames" : "after", kind == 0 ? "tick" : "fired", counts: false) { call(instance, kind, token, now, seq) }
        return true
    }

    private func dueAfters(at now: Double) -> [UInt64] {
        let due = afters.filter { $0.due <= now }.sorted { ($0.due, $0.order) < ($1.due, $1.order) }
        afters.removeAll { $0.due <= now }
        return due.map(\.token)
    }

    /// Under the agent, at the instant `now`, its commits applied: the ticks,
    /// the `after`s, and what they asked, drained here. The limit's name when
    /// one is passed.
    func fire(at now: Double) -> String? {
        if !tickets.isEmpty, now >= frameDue {
            while frameDue <= now { k += 1 }
            for token in tickets where tickets.contains(token) { guard tick(0, token, at: now) else { return "HatchFireLimit" } }
        }
        while true {
            for token in dueAfters(at: now) { guard tick(1, token, at: now) else { return "HatchFireLimit" } }
            guard let elements = natives?.session?.presenter.elements, elements.inFlight > 0 else {
                if afters.contains(where: { $0.due <= now }) { continue }
                return nil
            }
            acts += elements.inFlight < ElementHatches.snapshot ? elements.inFlight : ElementHatches.snapshot
            guard acts <= Self.limit else { return "HatchActLimit" }
            elements.drain()
        }
    }

    /// A presented frame, after its tasks and timers: each ticket ticks.
    func presented(_ now: Double) {
        for token in tickets where tickets.contains(token) { _ = tick(0, token, at: now) }
    }

    /// On the wall the next `after` is a timer; under the agent the seek fires it.
    private func rearm() {
        timer?.invalidate(); timer = nil
        guard let session = natives?.session, session.clock == nil, let next = afters.map(\.due).min() else { return }
        timer = Timer.scheduledTimer(withTimeInterval: max(0, (next - now) / 1000), repeats: false) { [weak self] _ in
            guard let self else { return }
            let now = self.now
            for token in self.dueAfters(at: now) { _ = self.tick(1, token, at: now) }
            self.rearm()
        }
    }
}
