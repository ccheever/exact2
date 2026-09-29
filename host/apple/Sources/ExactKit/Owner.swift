// The one thread that owns every Rust runtime in the process.
// @ref LLP 1071 T1/T2/T5 — the registry, runners, kernels and data sources
// live here from `exact_create` to `exact_destroy`; main submits each call as
// one job and waits for it. The owner reaches main only through `callMain`,
// which main serves from its wait loop, so neither ever waits on the other
// while the other waits on it.
import Foundation

final class Owner: @unchecked Sendable {
    static let shared = Owner()

    // A job's and a call's body is released before it is marked done: the
    // waiter's non-escaping closure must have no other owner when it returns.
    private final class Job {
        var body: (() -> Void)?
        var done = false
        init(_ body: @escaping () -> Void) { self.body = body }
    }
    private final class MainCall {
        var body: (() -> Void)?
        /// 0 posted, 1 claimed, 2 done: each call runs once.
        var state = 0
        init(_ body: @escaping () -> Void) { self.body = body }
    }

    private let condition = NSCondition()
    private var jobs: [Job] = []
    /// Notifications (T5): run after the job in progress, in order.
    private var later: [Job] = []
    private var mailbox: [MainCall] = []
    private var started = false
    private var thread: pthread_t?
    /// Main is running a call the owner waits on (T5): it cannot wait on
    /// the owner. Touched on main only.
    private var serving = 0

    private init() {}

    /// Whether the caller is the owner thread.
    var isOwner: Bool {
        guard let thread else { return false }
        return pthread_equal(pthread_self(), thread) != 0
    }

    private func start() {
        condition.lock()
        defer { condition.unlock() }
        guard !started else { return }
        started = true
        let worker = Thread { [self] in
            condition.lock()
            thread = pthread_self()
            condition.broadcast()
            condition.unlock()
            run()
        }
        worker.name = "exact.owner"
        worker.qualityOfService = .userInteractive
        worker.stackSize = 8 << 20
        worker.start()
        while thread == nil { condition.wait() }
    }

    private func run() {
        while true {
            condition.lock()
            while jobs.isEmpty && later.isEmpty { condition.wait() }
            let job = jobs.isEmpty ? later.removeFirst() : jobs.removeFirst()
            condition.unlock()
            autoreleasepool { job.body?() }
            condition.lock()
            job.body = nil
            job.done = true
            condition.broadcast()
            condition.unlock()
        }
    }

    /// Run `body` on the owner and return its result; on the owner, inline.
    /// Main waits, serving the owner's `callMain` requests meanwhile. From a
    /// call main serves for the owner, nothing can run: `busy` answers.
    func sync<T>(_ body: () -> T, busy: @autoclosure () -> T) -> T {
        if isOwner { return body() }
        if Thread.isMainThread && serving > 0 {
            NSLog("exact: a runtime call from inside a callback the owner is waiting on was refused (LLP 1071 T5)")
            return busy()
        }
        start()
        return withoutActuallyEscaping(body) { body in
            var result: T?
            let job = Job { result = body() }
            condition.lock()
            jobs.append(job)
            condition.broadcast()
            let main = Thread.isMainThread
            while !job.done {
                if main, let call = mailbox.first(where: { $0.state == 0 }) {
                    serve(call)
                    continue
                }
                condition.wait()
            }
            condition.unlock()
            return result!
        }
    }

    func sync<T>(_ body: () -> T) -> T {
        sync(body, busy: Owner.unserved())
    }

    private static func unserved<T>() -> T {
        preconditionFailure("exact: a runtime call from inside a callback the owner is waiting on (LLP 1071 T5)")
    }

    /// A notification (T5): synchronous, unless main is serving a callback
    /// the owner waits on; then it runs after the owner's current job.
    func syncOrLater(_ body: @escaping () -> Void) {
        if isOwner { body(); return }
        if Thread.isMainThread && serving > 0 {
            condition.lock()
            later.append(Job(body))
            condition.broadcast()
            condition.unlock()
            return
        }
        sync(body, busy: ())
    }

    /// Run `body` on main and wait for it. From the owner this is the one
    /// door (T5): main serves it from its wait loop, or from a main-queue
    /// hop when it is not waiting. From main, inline; from any other thread,
    /// as `DispatchQueue.main.sync` always did.
    func callMain<T>(_ body: () -> T) -> T {
        if Thread.isMainThread { return body() }
        guard isOwner else { return DispatchQueue.main.sync(execute: body) }
        return withoutActuallyEscaping(body) { body in
            var result: T?
            let call = MainCall { result = body() }
            condition.lock()
            mailbox.append(call)
            condition.broadcast()
            condition.unlock()
            DispatchQueue.main.async { [self] in
                condition.lock()
                if call.state == 0 { serve(call) }
                condition.unlock()
            }
            condition.lock()
            while call.state != 2 { condition.wait() }
            condition.unlock()
            return result!
        }
    }

    /// Serve one posted call on main; the lock is held on entry and exit
    /// and released while the call runs.
    private func serve(_ call: MainCall) {
        call.state = 1
        mailbox.removeAll { $0 === call }
        condition.unlock()
        serving += 1
        call.body?()
        serving -= 1
        condition.lock()
        call.body = nil
        call.state = 2
        condition.broadcast()
    }
}
