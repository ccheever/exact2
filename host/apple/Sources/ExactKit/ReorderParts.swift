// What a reorder in a List and a grouped list's reorder (`ReorderGroup`)
// share (LLP 1094 D6; LLP 1047.001 D4: the core keeps it, the drag
// capability and grouped lists both use it).
import Foundation

/// The runtime's three Arrange calls (`exact.h`); a test substitutes its own.
package protocol ReorderCalls: AnyObject {
    func reorderBegin(_ handle: UInt32, scrollTop: Double, now: Double) -> Batch
    func reorderMove(_ token: UInt64, dy: Double, scrollTop: Double, inside: Bool, now: Double) -> Batch
    func reorderEnd(_ token: UInt64, drop: Bool, dy: Double, scrollTop: Double, inside: Bool, velocity: Double, now: Double) -> Batch
}
extension Runtime: ReorderCalls {}

/// One `{"op":"reorder"}`: the contact's serial, its List and lifted wrapper.
package struct ReorderState: Equatable {
    package let token: UInt64
    package let list: UInt32
    package let wrapper: UInt32
    package let phase: String
    package let dispatched: Bool
    package init?(_ op: [String: Any]) {
        guard let raw = op["token"] as? String, let token = UInt64(raw),
              let list = op["list"] as? Int, let wrapper = op["wrapper"] as? Int,
              let phase = op["phase"] as? String else { return nil }
        self.token = token; self.list = UInt32(clamping: list); self.wrapper = UInt32(clamping: wrapper)
        self.phase = phase; self.dispatched = op["dispatched"] as? Bool ?? false
    }
    package static func last(in batch: Batch) -> ReorderState? {
        batch.ops.last { $0.op == .reorder }.flatMap { ReorderState($0.payload) }
    }
}

/// The web host's edge scroll: 32-point bands inside the port's top and
/// bottom (and past them), 720 points a second, at most 32 ms of catch-up.
package enum ReorderEdge {
    package static func direction(offset: Double, height: Double) -> Double {
        guard offset.isFinite, height.isFinite else { return 0 }
        return offset < 32 ? -1 : offset > height - 32 ? 1 : 0
    }
    package static func step(direction: Double, dt: Double) -> Double {
        direction * 720 * min(0.032, max(0, dt))
    }
}
