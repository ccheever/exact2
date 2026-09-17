// One authored header binding; IDs are resolved by the kernel, never by Swift.
import Foundation

struct HeightDragBinding: Equatable {
    let id: UInt32
    let target: UInt32?
    let handleKey: UInt64
    let targetKey: UInt64?

    init?(_ op: [String: Any]) {
        guard let rawID = op["id"] as? Int, let id = UInt32(exactly: rawID),
              let rawHandle = op["handleKey"] as? String,
              let handleKey = UInt64(rawHandle) else { return nil }
        self.id = id; self.handleKey = handleKey
        if op["target"] is NSNull, op["targetKey"] is NSNull {
            target = nil; targetKey = nil
        } else {
            guard let rawID = op["target"] as? Int, let target = UInt32(exactly: rawID),
                  let rawTarget = op["targetKey"] as? String,
                  let targetKey = UInt64(rawTarget) else { return nil }
            self.target = target; self.targetKey = targetKey
        }
    }
}

/// Both platforms normalize window-space displacement to positive downward.
/// The captured origin is used CSS height, not an authored snap position.
struct HeightDragPosition {
    let base: Double
    init?(base: Double) {
        guard base.isFinite, base >= 0, base <= Double(Float.greatestFiniteMagnitude) else { return nil }
        self.base = base
    }
    func value(downward: Double) -> Double? {
        guard downward.isFinite else { return nil }
        let next = base - downward
        guard next.isFinite, next <= Double(Float.greatestFiniteMagnitude) else { return nil }
        return max(0, next)
    }
}

/// Finite differences of accepted layout heights, never raw finger velocity.
/// A duplicate lift sample retains a recent slope for at most the same 100ms
/// freshness window used by motion's pointer velocity tracker.
struct HeightDragVelocity {
    private(set) var height: Double
    private(set) var time: Double
    private(set) var velocity = 0.0
    init(height: Double, time: Double) { self.height = height; self.time = time }
    @discardableResult mutating func record(height: Double, time: Double, ending: Bool = false) -> Bool {
        guard height.isFinite, height >= 0, time.isFinite, time >= self.time else { return false }
        let elapsed = time - self.time
        if !(ending && height == self.height && elapsed <= 0.1) {
            velocity = elapsed > 0 ? (height - self.height) / elapsed : 0
            if !velocity.isFinite { velocity = 0 }
        }
        self.height = height; self.time = time
        return true
    }
}
