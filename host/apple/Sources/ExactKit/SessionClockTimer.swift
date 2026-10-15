import Foundation

/// @ref LLP 1043.000 §3 D8 — deadlines, in milliseconds, never a repeating poll.
enum SessionClockTimer {
    enum Wake: Equatable { case none, frame, timeout(Double) }
    static func wake(due: Double?, now: Double, agent: Bool = false) -> Wake {
        guard !agent, let due else { return .none }
        let delay = max(0, due - now)
        return delay <= 8 * 1000 / 60 ? .frame : .timeout(delay)
    }
    static func schedule(after seconds: TimeInterval, _ fire: @escaping @Sendable (Timer) -> Void) -> Timer {
        precondition(Thread.isMainThread)
        let timer = Timer(timeInterval: seconds, repeats: false, block: fire)
        RunLoop.main.add(timer, forMode: .common)
        return timer
    }
}
