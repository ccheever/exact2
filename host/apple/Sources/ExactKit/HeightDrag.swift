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

/// Recognition may have displacement but zero instantaneous velocity (notably
/// a coalesced iOS-on-Mac drag). Direction still belongs to the actual input.
enum HeightDragDirection {
    static func accepts(velocityX: Double, velocityY: Double, translationX: Double, translationY: Double) -> Bool {
        guard velocityX.isFinite, velocityY.isFinite, translationX.isFinite, translationY.isFinite else { return false }
        if velocityX == 0 && velocityY == 0 { return abs(translationY) > abs(translationX) }
        return abs(velocityY) > abs(velocityX)
    }
}
