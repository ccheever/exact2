// @ref LLP 1012#summary — physical-device agent startup, before the token handshake.
import Foundation
import Darwin

/// One startup deadline, including inactive time. A returned socket belongs to
/// the caller; cancelling this object never closes a socket already handed off.
final class AgentSocketStartup: @unchecked Sendable {
    enum Failure: Error, CustomStringConvertible {
        case cancelled
        case timedOut(lastError: Int32)
        case system(Int32)

        var description: String {
            switch self {
            case .cancelled: return "cancelled before handshake"
            case .timedOut(let error):
                return "deadline expired before handshake" + (error == 0 ? " (waiting for foreground)" : ": \(String(cString: strerror(error)))")
            case .system(let error): return String(cString: strerror(error))
            }
        }
    }
    private let lock = NSLock()
    private let changed = DispatchSemaphore(value: 0)
    private let deadline: DispatchTime
    private var active: Bool
    private var cancelled = false
    private var finished = false

    init(active: Bool, timeout: TimeInterval = 15) {
        self.active = active
        deadline = .now() + timeout
    }
    func setActive(_ value: Bool) {
        lock.lock()
        if !finished { active = value }
        lock.unlock()
        changed.signal()
    }
    func cancel() {
        lock.lock()
        if !finished { cancelled = true }
        lock.unlock()
        changed.signal()
    }
    private var state: (active: Bool, cancelled: Bool) {
        lock.lock(); defer { lock.unlock() }
        return (active, cancelled)
    }
    private var remaining: UInt64 {
        let now = DispatchTime.now().uptimeNanoseconds
        return deadline.uptimeNanoseconds > now ? deadline.uptimeNanoseconds - now : 0
    }
    private func pause() {
        let delay = min(remaining, 100_000_000)
        _ = changed.wait(timeout: .now() + .nanoseconds(Int(delay)))
    }
    private func finish() {
        lock.lock(); finished = true; lock.unlock()
    }

    /// `failedAttempt` observes a closed attempt, allowing a delayed-listener
    /// test to advance on the actual refusal instead of sleeping.
    func connect(to address: sockaddr_in, waitingForForeground: (() -> Void)? = nil, failedAttempt: ((Int32) -> Void)? = nil) throws -> Int32 {
        defer { finish() }
        var lastError: Int32 = 0
        while remaining > 0 {
            let before = state
            if before.cancelled { throw Failure.cancelled }
            if !before.active {
                waitingForForeground?()
                _ = changed.wait(timeout: deadline)
                continue
            }
            let fd = socket(AF_INET, SOCK_STREAM, 0)
            guard fd >= 0 else { throw Failure.system(errno) }
            var handedOff = false
            defer { if !handedOff { Darwin.close(fd) } }
            let flags = fcntl(fd, F_GETFL)
            guard flags >= 0, fcntl(fd, F_SETFL, flags | O_NONBLOCK) == 0 else { throw Failure.system(errno) }
            var address = address
            let result = withUnsafePointer(to: &address) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                    Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
                }
            }
            var error: Int32 = result == 0 ? 0 : errno
            if error == EINPROGRESS || error == EINTR {
                while remaining > 0 {
                    let current = state
                    if current.cancelled { throw Failure.cancelled }
                    if !current.active { error = EINTR; break }
                    var descriptor = pollfd(fd: fd, events: Int16(POLLOUT), revents: 0)
                    let ms = Int32(max(1, min(100, (remaining + 999_999) / 1_000_000)))
                    let polled = poll(&descriptor, 1, ms)
                    if polled == 0 { continue }
                    if polled < 0 {
                        if errno == EINTR { continue }
                        throw Failure.system(errno)
                    }
                    var length = socklen_t(MemoryLayout<Int32>.size)
                    guard descriptor.revents & Int16(POLLNVAL) == 0 else { throw Failure.system(EBADF) }
                    guard getsockopt(fd, SOL_SOCKET, SO_ERROR, &error, &length) == 0 else { throw Failure.system(errno) }
                    break
                }
            }
            if error == 0 {
                // Cancellation and the final ownership transfer have one order.
                lock.lock()
                if !cancelled && active && remaining > 0 {
                    if fcntl(fd, F_SETFL, flags) == 0 {
                        finished = true
                        handedOff = true
                        lock.unlock()
                        return fd
                    }
                    error = errno
                } else { error = EINTR }
                lock.unlock()
            }
            lastError = error
            // Retrying never writes a token or a command. Close the failed
            // descriptor before the observer or the next admission can run.
            Darwin.close(fd)
            handedOff = true
            if state.cancelled { throw Failure.cancelled }
            if remaining == 0 { break }
            guard [EINTR, EHOSTUNREACH, ENETUNREACH, ENETDOWN, ECONNREFUSED, ETIMEDOUT].contains(error) else {
                throw Failure.system(error)
            }
            failedAttempt?(error)
            pause()
        }
        if state.cancelled { throw Failure.cancelled }
        throw Failure.timedOut(lastError: lastError)
    }
}
