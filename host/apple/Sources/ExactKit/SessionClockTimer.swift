import Foundation

/// Native wakes never request display frames; frame tasks have their own demand.
enum SessionClockTimer {
    enum Wake: Equatable { case none, timeout(Double) }
    static func wake(due: Double?, now: Double, agent: Bool = false) -> Wake {
        guard !agent, let due else { return .none }
        return .timeout(max(0, due - now))
    }
    static func schedule(after seconds: TimeInterval, tolerance: TimeInterval = 0, _ fire: @escaping @Sendable (Timer) -> Void) -> Timer {
        schedule(at: Date().addingTimeInterval(seconds), tolerance: tolerance, fire)
    }
    static func schedule(at date: Date, every interval: TimeInterval = 0, tolerance: TimeInterval = 0, _ fire: @escaping @Sendable (Timer) -> Void) -> Timer {
        precondition(Thread.isMainThread)
        let timer = Timer(fire: date, interval: interval, repeats: interval > 0, block: fire)
        timer.tolerance = max(0, tolerance)
        RunLoop.main.add(timer, forMode: .common)
        return timer
    }
}
