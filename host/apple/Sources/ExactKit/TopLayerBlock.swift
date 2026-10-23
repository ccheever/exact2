import CoreGraphics
import Foundation
import ExactCore

extension BatchOp {
    /// A `block` op's block as a rectangle in its root's own coordinates:
    /// back from the root by where it sits in the block (`x`, `y`: its
    /// margins), at the block's used size. A top-layer box is reported to
    /// the runner by its block's rectangle, which the runner maps through
    /// the block (LLP 1123 D2); a popover's block is the viewport's, its
    /// content's size (stage 14 B), and its margins make it larger than the
    /// border box. Nil for any other op.
    var blockRect: CGRect? {
        guard payload["op"] as? String == "block",
              let w = payload["w"] as? NSNumber, let h = payload["h"] as? NSNumber else { return nil }
        let x = (payload["x"] as? NSNumber)?.doubleValue ?? 0, y = (payload["y"] as? NSNumber)?.doubleValue ?? 0
        return CGRect(x: -x, y: -y, width: w.doubleValue, height: h.doubleValue)
    }
}
