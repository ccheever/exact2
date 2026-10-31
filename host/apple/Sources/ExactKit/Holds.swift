// The app's lifecycle holds for work an answer started that it must finish
// (`Runner::must_finish`): the module's storage, which lands after the
// answer that started it (LLP 1097 D10), and a request with effects, whose
// server may have acted on it, spending a rotating refresh token say, so its
// reply must land and what its turn writes be kept (LLP 1146 D2). iOS and
// tvOS hold a background task from the batch that says there is such work
// to the one that says it is done, or until the system's time runs out
// (`FinishHold`). macOS waits to terminate while there is, and while a
// native module's quit work runs (LLP 1069.010 Q4), five seconds at most for
// all together, and the agent's exit 1.5 (`QuitHold`). What the store
// already has completes on disk regardless.
import Foundation
#if canImport(UIKit)
import UIKit
#endif

public extension ExactSession {
    /// Whether the app must finish work an answer started, asked of the
    /// runner now: what a quit waits for.
    var mustFinish: Bool {
        guard let data = agent("{\"op\":\"background\"}").data(using: .utf8),
              let reply = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
        return reply["mustFinish"] as? Bool == true
    }

    /// The quit is decided (macOS): the session's native module starts the
    /// work the quit waits for (`ExactModule.willQuit`), once.
    func willQuit() { natives.willQuit() }
    /// Whether that work is still running: until the module says it is
    /// done or the session ends.
    var quitWorkPending: Bool { natives.quitWorkPending }
}

/// A session's background task while its batches say it must finish work an
/// answer started. It begins at the word, as UIKit asks of work that must
/// finish: "Don't wait until your app moves to the background" ("Extending
/// your app's background execution time"). The system may take it back
/// first (its expiration); while the work is still wanted, the app's return
/// to the foreground begins another, as it does after the system refused one.
/// The begin and end are the platform's, injected, so the rule is tested
/// without one (`FinishHoldTests`).
package final class FinishHold {
    let begin: (@escaping () -> Void) -> Bool
    let end: () -> Void
    /// What the last word said.
    private(set) var wanted = false
    /// Whether a task is held: while wanted, unless the system refused it or
    /// took it back.
    private(set) var held = false

    /// `begin` takes the expiration to call if the platform takes the hold
    /// back first, and answers whether the platform gave one; `end` lets it go.
    init(begin: @escaping (@escaping () -> Void) -> Bool, end: @escaping () -> Void) {
        self.begin = begin
        self.end = end
    }

    /// A batch said whether the session must finish work.
    func moved(_ must: Bool) {
        wanted = must
        if must { take() } else { release() }
    }

    /// The app is in the foreground again: a task the system took back is
    /// begun again while the work is still wanted.
    func becameActive() {
        if wanted { take() }
    }

    private func take() {
        guard !held else { return }
        held = begin { [weak self] in self?.release() }
    }

    private func release() {
        guard held else { return }
        held = false
        end()
    }

    /// A UIKit background task on iOS and tvOS; nothing on the Mac, whose quit asks `mustFinish`.
    static func platform() -> FinishHold {
        #if canImport(UIKit)
        // `.invalid` when the app may not run in the background
        var task = UIBackgroundTaskIdentifier.invalid
        return FinishHold(begin: { expired in
                              task = UIApplication.shared.beginBackgroundTask(withName: "finishing an answer's work", expirationHandler: expired)
                              return task != .invalid
                          },
                          end: { UIApplication.shared.endBackgroundTask(task); task = .invalid })
        #else
        return FinishHold(begin: { _ in true }, end: {})
        #endif
    }
}

/// Holds something (a deferred quit, the agent's exit) while `pending`
/// says work is landing, checking at `every`, and lets it go when none
/// is or `bound` passes. The end is the platform's, injected, so the rule
/// is tested without one (`QuitHoldTests`).
public final class QuitHold {
    let pending: () -> Bool
    let end: () -> Void
    let every: TimeInterval
    let bound: TimeInterval?
    private var timer: Timer?
    private var started: Date?

    /// `end` lets the held thing go. `bound` is the longest to hold.
    public init(every: TimeInterval = 0.02, bound: TimeInterval? = nil, pending: @escaping () -> Bool, end: @escaping () -> Void) {
        self.pending = pending
        self.end = end
        self.every = every
        self.bound = bound
    }

    /// Whether a hold is taken.
    public var holding: Bool { started != nil }

    /// Take the hold if work is landing; whether it was taken.
    @discardableResult
    public func hold() -> Bool {
        guard started == nil, pending() else { return false }
        started = Date()
        let t = Timer(timeInterval: every, repeats: true) { [weak self] _ in self?.check() }
        // Common modes: a deferred quit runs the loop in the modal panel mode.
        RunLoop.main.add(t, forMode: .common)
        timer = t
        return true
    }

    /// Let the hold go now if the work has landed or the bound has passed.
    public func check() {
        guard let started else { return }
        if !pending() || bound.map({ Date().timeIntervalSince(started) >= $0 }) == true { release() }
    }

    /// Let the hold go, once.
    public func release() {
        guard started != nil else { return }
        started = nil
        timer?.invalidate()
        timer = nil
        end()
    }
}
