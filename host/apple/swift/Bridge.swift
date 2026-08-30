// The C ABI, wrapped: payloads in, JSON batches out (exact.h).
import CExact
import Foundation

struct Batch {
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

    /// A request's reply is queued (LLP 1016 D2), on the executor's thread:
    /// the presenter sets this to hop to its main thread and `pump`.
    nonisolated(unsafe) static var wake: ExactWakeFn? = nil

    static func boot(width: CGFloat, height: CGFloat) -> Batch {
        read(exact_boot(measureText, nil, wake, nil, Float(width), Float(height)))
    }

    /// Every queued reply into the runner: the batch of their commits.
    static func pump(now: Double) -> Batch { read(exact_pump(now)) }
    /// The dev loop's restart: boot from plan bytes, state carried.
    static func bootPlan(_ bytes: Data, width: CGFloat, height: CGFloat) -> Batch {
        let ptr = exact_in(bytes.count)!
        bytes.withUnsafeBytes { ptr.update(from: $0.bindMemory(to: UInt8.self).baseAddress!, count: bytes.count) }
        return read(exact_boot_plan(bytes.count, measureText, nil, wake, nil, Float(width), Float(height)))
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
    static func tick(now: Double) -> Batch { read(exact_tick(now)) }
    static func intrinsic(_ view: UInt32, width: CGFloat, height: CGFloat) -> Batch { read(exact_intrinsic(view, Float(width), Float(height))) }
    /// The agent API (LLP 1012): a request in, its reply out — JSON, not a batch.
    static func agent(_ request: String) -> String {
        let n = write(request)
        let len = exact_agent(n)
        return String(decoding: Data(bytes: exact_out(), count: Int(len)), as: UTF8.self)
    }
}
