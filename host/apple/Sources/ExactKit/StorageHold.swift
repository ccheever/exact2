// The app's lifecycle holds for the module's storage (LLP 1097 D10): an
// answer replies before the write it started lands, so a quit or a
// suspension can come between them. macOS waits to terminate while the
// write lands (five seconds at most); iOS and tvOS hold a background task
// while one is in flight, ended when it lands or when the system's time
// runs out. What the store already has completes on disk regardless.
import Foundation
#if canImport(UIKit)
import UIKit
#endif

public extension ExactSession {
    /// The module's storage operations queued or in flight.
    var storageOperations: Int {
        guard let data = agent("{\"op\":\"background\"}").data(using: .utf8),
              let reply = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return 0 }
        return reply["operations"] as? Int ?? 0
    }
}

#if canImport(UIKit)
public extension StorageHold {
    /// A background task held while `pending`, ended when it is not or when
    /// the system's time runs out; `nil` when nothing was landing.
    static func backgroundTask(_ pending: @escaping () -> Bool) -> StorageHold? {
        var task = UIBackgroundTaskIdentifier.invalid
        let hold = StorageHold(pending: pending,
                               begin: { expired in task = UIApplication.shared.beginBackgroundTask(withName: "storage", expirationHandler: expired) },
                               end: { UIApplication.shared.endBackgroundTask(task); task = .invalid })
        return hold.hold() ? hold : nil
    }
}
#endif

/// Holds something (a background task, a deferred quit) while `pending`
/// says storage is landing, checking at `every`, and lets it go when none
/// is or `bound` passes. The begin and end are the platform's, injected,
/// so the rule is tested without one (`StorageHoldTests`).
public final class StorageHold {
    let pending: () -> Bool
    let begin: (@escaping () -> Void) -> Void
    let end: () -> Void
    let every: TimeInterval
    let bound: TimeInterval?
    private var timer: Timer?
    private var started: Date?

    /// `begin` takes the expiration to call if the platform takes the hold
    /// back first; `end` lets it go. `bound` is the longest to hold.
    public init(every: TimeInterval = 0.02, bound: TimeInterval? = nil, pending: @escaping () -> Bool,
                begin: @escaping (@escaping () -> Void) -> Void, end: @escaping () -> Void) {
        self.pending = pending
        self.begin = begin
        self.end = end
        self.every = every
        self.bound = bound
    }

    /// Whether a hold is taken.
    public var holding: Bool { started != nil }

    /// Take the hold if storage is landing; whether it was taken.
    @discardableResult
    public func hold() -> Bool {
        guard started == nil, pending() else { return false }
        started = Date()
        begin { [weak self] in self?.release() }
        let t = Timer(timeInterval: every, repeats: true) { [weak self] _ in self?.check() }
        // Common modes: a deferred quit runs the loop in the modal panel mode.
        RunLoop.main.add(t, forMode: .common)
        timer = t
        return true
    }

    /// Let the hold go now if storage has landed or the bound has passed.
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
