// The C ABI, wrapped: payloads in, JSON batches out (exact.h). `Exact` is
// synchronous and is called only by `runtime`'s dedicated thread; UIKit/AppKit
// never enters the runner, kernel, layout, or CoreText measurement on main.
//
// @ref LLP 1008 §11 (one runtime owner; complete batches published to main)
import CExact
import Foundation

struct Batch: @unchecked Sendable {
    let ops: [[String: Any]]
    let timers: Bool
    let motion: Bool
    /// The runner's clock after the call (milliseconds); an advance a timer
    /// refused stops at that timer's due time.
    let clock: Double?
    let error: String?
}

enum Exact {
    static func read(_ len: UInt32) -> Batch {
        let data = Data(bytes: exact_out(), count: Int(len))
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return Batch(ops: [], timers: false, motion: false, clock: nil, error: "unreadable batch")
        }
        return Batch(
            ops: obj["ops"] as? [[String: Any]] ?? [],
            timers: obj["timers"] as? Bool ?? false,
            motion: obj["motion"] as? Bool ?? false,
            clock: obj["clock"] as? Double,
            error: obj["error"] as? String)
    }

    static func write(_ text: String) -> Int {
        let bytes = Array(text.utf8)
        let ptr = exact_in(bytes.count)!
        bytes.withUnsafeBufferPointer { ptr.update(from: $0.baseAddress!, count: bytes.count) }
        return bytes.count
    }

    static func boot(width: CGFloat, height: CGFloat) -> Batch {
        read(exact_boot(measureText, nil, exactWake, nil, Float(width), Float(height)))
    }

    static func prepare() {
        exact_prepare(measureText, nil, exactWake, nil)
    }

    static func present(width: CGFloat, height: CGFloat) -> Batch {
        read(exact_present(Float(width), Float(height)))
    }

    /// Every queued reply into the runner: the batch of their commits.
    static func pump(now: Double) -> Batch { read(exact_pump(now)) }
    /// The dev loop's restart: boot from plan bytes, state carried.
    static func bootPlan(_ bytes: Data, width: CGFloat, height: CGFloat) -> Batch {
        let ptr = exact_in(bytes.count)!
        bytes.withUnsafeBytes { ptr.update(from: $0.bindMemory(to: UInt8.self).baseAddress!, count: bytes.count) }
        return read(exact_boot_plan(bytes.count, measureText, nil, exactWake, nil, Float(width), Float(height)))
    }
    static func press(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(view, 0, 0, now)) }
    /// The pointer over the view (`true`) or gone from it.
    static func hover(_ view: UInt32, over: Bool, now: Double) -> Batch { read(exact_dispatch(view, over ? 2 : 3, 0, now)) }
    static func focus(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(view, 4, 0, now)) }
    static func blur(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(view, 5, 0, now)) }
    static func submit(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(view, 7, 0, now)) }
    /// A key down at the view, by the web's key name.
    static func key(_ view: UInt32, _ name: String, now: Double) -> Batch {
        let n = write(name)
        return read(exact_dispatch(view, 6, n, now))
    }
    static func change(_ view: UInt32, _ value: String, now: Double) -> Batch {
        let n = write(value)
        return read(exact_dispatch(view, 1, n, now))
    }
    static func advance(now: Double) -> Batch { read(exact_advance(now)) }
    static func resize(width: CGFloat, height: CGFloat) -> Batch { read(exact_resize(Float(width), Float(height))) }
    static func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) -> Batch { read(exact_insets(Float(top), Float(right), Float(bottom), Float(left))) }
    static func keyboard(_ height: CGFloat) -> Batch { read(exact_keyboard(Float(height))) }
    static func tick(now: Double) -> Batch { read(exact_tick(now)) }
    static func intrinsic(_ view: UInt32, width: CGFloat, height: CGFloat) -> Batch { read(exact_intrinsic(view, Float(width), Float(height))) }
    /// The agent API (LLP 1012): a request in, its reply out — JSON, not a batch.
    static func agent(_ request: String) -> String {
        let n = write(request)
        let len = exact_agent(n)
        return String(decoding: Data(bytes: exact_out(), count: Int(len)), as: UTF8.self)
    }
}

/// The Apple host's one runner/kernel owner. Every C call runs in FIFO order
/// here; its immutable result is delivered to main as one complete frame.
/// The counter includes results already queued to main, so `barrier` also
/// waits until their presenter applies have completed.
final class ExactRuntime: @unchecked Sendable {
    private let condition = NSCondition()
    private var jobs: [() -> Void] = []
    private var publications: [() -> Void] = []
    private var pending = 0
    private var waiters: [() -> Void] = []

    init() {
        let worker = Thread { [self] in
            Thread.current.name = "exact.runtime"
            while true {
                condition.lock()
                while jobs.isEmpty { condition.wait() }
                let job = jobs.removeFirst()
                condition.unlock()
                autoreleasepool { job() }
            }
        }
        worker.qualityOfService = .userInteractive
        worker.start()
    }

    private func enqueue(_ job: @escaping () -> Void) {
        condition.lock()
        jobs.append(job)
        condition.signal()
        condition.unlock()
    }

    /// Queue a prepared result for main. UIKit/AppKit lifecycle boundaries
    /// may claim it before the ordinary asynchronous delivery runs.
    private func publish(_ publication: @escaping () -> Void) {
        condition.lock()
        publications.append(publication)
        condition.unlock()
        DispatchQueue.main.async { [self] in publishReady() }
    }

    /// Apply every result the runtime has already prepared. This never waits
    /// for preparation: an empty publication queue is an immediate no-op.
    func publishReady() {
        precondition(Thread.isMainThread)
        condition.lock()
        let ready = publications
        publications.removeAll(keepingCapacity: true)
        condition.unlock()
        for publication in ready { publication() }
    }

    private func submit(_ work: @escaping () -> Batch, then completion: ((Batch) -> Void)? = nil) {
        precondition(Thread.isMainThread, "Exact runtime input must be captured on main")
        pending += 1
        enqueue { [self] in
            let batch = work()
            publish { [self] in
                if let completion { completion(batch) } else { apply(batch) }
                finished()
            }
        }
    }

    private func finished() {
        precondition(Thread.isMainThread)
        pending -= 1
        guard pending == 0 else { return }
        let ready = waiters
        waiters.removeAll(keepingCapacity: true)
        for waiter in ready { waiter() }
    }

    /// Run after every result submitted before this call has been applied.
    func barrier(_ completion: @escaping () -> Void) {
        precondition(Thread.isMainThread)
        if pending == 0 { completion() } else { waiters.append(completion) }
    }

    func boot(width: CGFloat, height: CGFloat, then completion: @escaping (Batch) -> Void) {
        submit({ Exact.boot(width: width, height: height) }, then: completion)
    }

    /// Begin baked-plan boot before a platform viewport exists. The FIFO
    /// makes a later `present` wait behind this without blocking main.
    func prepare() {
        precondition(Thread.isMainThread)
        enqueue { Exact.prepare() }
    }

    func present(width: CGFloat, height: CGFloat, then completion: @escaping (Batch) -> Void) {
        submit({ Exact.present(width: width, height: height) }, then: completion)
    }

    func bootPlan(_ bytes: Data, width: CGFloat, height: CGFloat, then completion: @escaping (Batch) -> Void) {
        submit({ Exact.bootPlan(bytes, width: width, height: height) }, then: completion)
    }

    func pump(now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.pump(now: now) }, then: completion)
    }

    func press(_ view: UInt32, now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.press(view, now: now) }, then: completion)
    }

    func hover(_ view: UInt32, over: Bool, now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.hover(view, over: over, now: now) }, then: completion)
    }

    func focus(_ view: UInt32, now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.focus(view, now: now) }, then: completion)
    }

    func blur(_ view: UInt32, now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.blur(view, now: now) }, then: completion)
    }

    func submit(_ view: UInt32, now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.submit(view, now: now) }, then: completion)
    }

    func key(_ view: UInt32, _ name: String, now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.key(view, name, now: now) }, then: completion)
    }

    func change(_ view: UInt32, _ value: String, now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.change(view, value, now: now) }, then: completion)
    }

    func advance(now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.advance(now: now) }, then: completion)
    }

    func resize(width: CGFloat, height: CGFloat, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.resize(width: width, height: height) }, then: completion)
    }

    func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.insets(top: top, right: right, bottom: bottom, left: left) }, then: completion)
    }

    func keyboard(_ height: CGFloat, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.keyboard(height) }, then: completion)
    }

    func tick(now: Double, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.tick(now: now) }, then: completion)
    }

    func intrinsic(_ view: UInt32, width: CGFloat, height: CGFloat, then completion: ((Batch) -> Void)? = nil) {
        submit({ Exact.intrinsic(view, width: width, height: height) }, then: completion)
    }

    /// Prepare one viewport publication. Insets must reach the kernel before
    /// the size, but main applies the batches in one turn (and, on iOS, one
    /// keyboard animation transaction). `keyboard` is `env(keyboard-inset-height)`.
    func viewport(insets: (CGFloat, CGFloat, CGFloat, CGFloat)?, size: CGSize?, keyboard: CGFloat? = nil, then completion: @escaping ([Batch]) -> Void) {
        precondition(Thread.isMainThread, "Exact runtime input must be captured on main")
        pending += 1
        enqueue { [self] in
            var batches: [Batch] = []
            if let i = insets { batches.append(Exact.insets(top: i.0, right: i.1, bottom: i.2, left: i.3)) }
            if let k = keyboard { batches.append(Exact.keyboard(k)) }
            if let s = size { batches.append(Exact.resize(width: s.width, height: s.height)) }
            publish { [self] in
                completion(batches)
                finished()
            }
        }
    }

    func agent(_ request: String, then completion: @escaping (String) -> Void) {
        precondition(Thread.isMainThread, "Exact runtime input must be captured on main")
        pending += 1
        enqueue { [self] in
            let reply = Exact.agent(request)
            publish { [self] in
                completion(reply)
                finished()
            }
        }
    }
}

let runtime = ExactRuntime()
