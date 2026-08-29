// The C ABI, wrapped: payloads in, JSON batches out (exact.h).
import CExact
import Foundation

struct Batch {
    let ops: [[String: Any]]
    let timers: Bool
    let motion: Bool
    let error: String?
}

enum Exact {
    static func read(_ len: UInt32) -> Batch {
        let data = Data(bytes: exact_out(), count: Int(len))
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return Batch(ops: [], timers: false, motion: false, error: "unreadable batch")
        }
        return Batch(
            ops: obj["ops"] as? [[String: Any]] ?? [],
            timers: obj["timers"] as? Bool ?? false,
            motion: obj["motion"] as? Bool ?? false,
            error: obj["error"] as? String)
    }

    static func write(_ text: String) -> Int {
        let bytes = Array(text.utf8)
        let ptr = exact_in(bytes.count)!
        bytes.withUnsafeBufferPointer { ptr.update(from: $0.baseAddress!, count: bytes.count) }
        return bytes.count
    }

    static func boot(width: CGFloat, height: CGFloat) -> Batch {
        read(exact_boot(measureText, nil, Float(width), Float(height)))
    }
    /// The dev loop's restart: boot from plan bytes, state carried.
    static func bootPlan(_ bytes: Data, width: CGFloat, height: CGFloat) -> Batch {
        let ptr = exact_in(bytes.count)!
        bytes.withUnsafeBytes { ptr.update(from: $0.bindMemory(to: UInt8.self).baseAddress!, count: bytes.count) }
        return read(exact_boot_plan(bytes.count, measureText, nil, Float(width), Float(height)))
    }
    static func press(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(view, 0, 0, now)) }
    static func change(_ view: UInt32, _ value: String, now: Double) -> Batch {
        let n = write(value)
        return read(exact_dispatch(view, 1, n, now))
    }
    static func advance(now: Double) -> Batch { read(exact_advance(now)) }
    static func resize(width: CGFloat, height: CGFloat) -> Batch { read(exact_resize(Float(width), Float(height))) }
    static func tick(now: Double) -> Batch { read(exact_tick(now)) }
}
