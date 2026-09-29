// The one thread that owns every Rust runtime in the process.
// @ref LLP 1072 T1/T2/T5 — the registry, runners, kernels and data sources
// live here from `exact_create` to `exact_destroy`; main submits each call as
// one job and waits for it. The owner reaches main only through `callMain`,
// which main serves from its wait loop, so neither ever waits on the other
// while the other waits on it.
import Foundation
final class Owner: @unchecked Sendable {
    static let shared = Owner()

    // A job's and a call's body is released before its waiter is woken: the
    // waiter's non-escaping closure must have no other owner when it returns.
    private final class Job {
        var body: (() -> Void)?
        var done = false
        /// Signalled when the job is done, or when a call waits for main.
        let wake = DispatchSemaphore(value: 0)
        init(_ body: @escaping () -> Void) { self.body = body }
    }
    private final class MainCall {
        var body: (() -> Void)?
        /// 0 posted, 1 claimed, 2 done: each call runs once.
        var state = 0
        let done = DispatchSemaphore(value: 0)
        init(_ body: @escaping () -> Void) { self.body = body }
    }

    // One lock guards the queues and states; semaphores carry the wakes, so
    // each side wakes only the thread that waits: a round trip is two wakes.
    private let lock = NSLock()
    private let work = DispatchSemaphore(value: 0)
    private var jobs: [Job] = []
    /// Notifications (T5): run after the job that posted them.
    private var later: [Job] = []
    private var mailbox: [MainCall] = []
    /// The job main is waiting on: a call for main wakes it.
    private var mainWaits: Job?
    private var thread: pthread_t?
    /// Main is running a call the owner waits on (T5): it cannot wait on
    /// the owner. Touched on main only.
    private var serving = 0

    private init() {
        let started = DispatchSemaphore(value: 0)
        let worker = Thread { [self] in
            lock.lock()
            thread = pthread_self()
            lock.unlock()
            started.signal()
            run()
        }
        worker.name = "exact.owner"
        worker.qualityOfService = .userInteractive
        worker.stackSize = 8 << 20
        worker.start()
        started.wait()
    }

    /// Whether the caller is the owner thread.
    var isOwner: Bool {
        // Set before `init` returns, and never again.
        guard let thread else { return false }
        return pthread_equal(pthread_self(), thread) != 0
    }

    private func run() {
        while true {
            work.wait()
            lock.lock()
            // A notification was posted during an earlier job: it goes first.
            let job = later.isEmpty ? jobs.removeFirst() : later.removeFirst()
            lock.unlock()
            autoreleasepool { job.body?() }
            lock.lock()
            job.body = nil
            job.done = true
            lock.unlock()
            job.wake.signal()
        }
    }

    /// Run `body` on the owner and return its result; on the owner, inline.
    /// Main waits, serving the owner's `callMain` requests meanwhile. From a
    /// call main serves for the owner, nothing can run: `busy` answers.
    func sync<T>(_ body: () -> T, busy: @autoclosure () -> T) -> T {
        if isOwner { return body() }
        let main = Thread.isMainThread
        if main && serving > 0 {
            NSLog("exact: a runtime call from inside a callback the owner is waiting on was refused (LLP 1072 T5)")
            return busy()
        }
        return withoutActuallyEscaping(body) { body in
            var result: T?
            let job = Job { result = body() }
            lock.lock()
            jobs.append(job)
            if main { mainWaits = job }
            lock.unlock()
            work.signal()
            while true {
                lock.lock()
                if job.done {
                    if main { mainWaits = nil }
                    lock.unlock()
                    break
                }
                // A call posted before this job was registered is served too.
                let call = main ? mailbox.first(where: { $0.state == 0 }) : nil
                if let call { claim(call) }
                lock.unlock()
                if let call { serve(call) } else { job.wake.wait() }
            }
            return result!
        }
    }

    func sync<T>(_ body: () -> T) -> T {
        sync(body, busy: Owner.unserved())
    }

    private static func unserved<T>() -> T {
        preconditionFailure("exact: a runtime call from inside a callback the owner is waiting on (LLP 1072 T5)")
    }

    /// Run `body` on the owner after the jobs before it, without waiting
    /// (LLP 1072 T3: the collection fill). What it produces goes back to
    /// main by its own publication.
    func post(_ body: @escaping () -> Void) {
        lock.lock()
        jobs.append(Job(body))
        lock.unlock()
        work.signal()
    }

    /// A notification (T5): on the owner, now; from anywhere else, queued
    /// in order behind the jobs before it, never waited for.
    func notify(_ body: @escaping () -> Void) {
        if isOwner { body() } else { post(body) }
    }

    /// A notification (T5): synchronous, unless main is serving a callback
    /// the owner waits on; then it runs after the owner's current job.
    func syncOrLater(_ body: @escaping () -> Void) {
        if isOwner { body(); return }
        if Thread.isMainThread && serving > 0 {
            lock.lock()
            later.append(Job(body))
            lock.unlock()
            work.signal()
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
            lock.lock()
            mailbox.append(call)
            let waiting = mainWaits
            lock.unlock()
            waiting?.wake.signal()
            // Main may be about to wait on a job; the wait loop checks the
            // mailbox first, and this hop covers main not waiting at all.
            DispatchQueue.main.async { [self] in
                lock.lock()
                let mine = call.state == 0
                if mine { claim(call) }
                lock.unlock()
                if mine { serve(call) }
            }
            call.done.wait()
            return result!
        }
    }

    /// Take a posted call for main; the lock is held.
    private func claim(_ call: MainCall) {
        call.state = 1
        mailbox.removeAll { $0 === call }
    }

    /// Run a claimed call on main, then release its waiter.
    private func serve(_ call: MainCall) {
        serving += 1
        call.body?()
        serving -= 1
        lock.lock()
        call.body = nil
        call.state = 2
        lock.unlock()
        call.done.signal()
    }
}
