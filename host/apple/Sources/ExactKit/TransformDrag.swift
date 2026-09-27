// Frozen photo wire and value policy; recognition and native geometry live in
// the platform adapter. Kernel resolves IDREFs; Swift never guesses an ID.
import Foundation
import CoreGraphics

struct TransformDragBinding: Equatable {
    let id: UInt32
    let runtime: UInt64
    let handleKey: UInt64
    let target: UInt32?
    let targetKey: UInt64?
    let clip: UInt32?
    let clipKey: UInt64?

    init?(_ op: [String: Any]) {
        func id(_ key: String) -> UInt32? { (op[key] as? Int).flatMap(UInt32.init(exactly:)) }
        func key(_ key: String) -> UInt64? { (op[key] as? String).flatMap(UInt64.init) }
        guard let view = id("id"), let runtime = key("runtime"), let handle = key("handleKey") else { return nil }
        self.id = view; self.runtime = runtime; handleKey = handle
        if ["target", "targetKey", "clip", "clipKey"].allSatisfy({ op[$0] is NSNull }) {
            target = nil; targetKey = nil; clip = nil; clipKey = nil
        } else {
            guard let target = id("target"), let targetKey = key("targetKey"),
                  let clip = id("clip"), let clipKey = key("clipKey") else { return nil }
            self.target = target; self.targetKey = targetKey; self.clip = clip; self.clipKey = clipKey
        }
    }
}

struct TransformDragPacket {
    let op: UInt32
    let runtime: UInt64
    let handleKey: UInt64
    let targetKey: UInt64
    let clipKey: UInt64
    let sequence: UInt64
    var translateToken: UInt64 = 0
    var scaleToken: UInt64 = 0
    let values: [Double]
    let now: Double

    func encoded() -> Data? {
        guard values.count == 6 else { return nil }
        var bytes = Data(capacity: 120)
        func append<T: FixedWidthInteger>(_ value: T) {
            var word = value.littleEndian
            withUnsafeBytes(of: &word) { bytes.append(contentsOf: $0) }
        }
        append(UInt32(2)); append(op)
        for word in [runtime, handleKey, targetKey, clipKey, sequence, translateToken, scaleToken] { append(word) }
        for value in values { append(value.bitPattern) }
        append(now.bitPattern)
        return bytes
    }
}

struct TransformDragPosition {
    let x: Double
    let y: Double
    let scale: Double
    var values: [Double] { [x, y, scale] }
    init?(x: Double, y: Double, scale: Double) {
        let maximum = Double(Float.greatestFiniteMagnitude)
        guard [x, y, scale].allSatisfy({ $0.isFinite && abs($0) <= maximum }),
              scale > 0, Float(scale) > 0 else { return nil }
        self.x = x; self.y = y; self.scale = scale
    }
    init?(_ values: [Double]) {
        guard values.count == 3 else { return nil }
        self.init(x: values[0], y: values[1], scale: values[2])
    }
    func moved(dx: Double, dy: Double) -> [Double]? {
        TransformDragPosition(x: x + dx, y: y + dy, scale: scale)?.values
    }
    /// Pinch anchored at its focal point (LLP 1057.001 §4): the content under
    /// `from` stays under `to` while scale multiplies by `factor`, so translate
    /// follows the centroid. Points are relative to the clip's centre, in the
    /// translate's space (x right, y down); `factor` 1 is a pan by `to - from`.
    func focused(from: CGPoint, to: CGPoint, factor: Double) -> TransformDragPosition? {
        guard factor.isFinite, factor > 0 else { return nil }
        return TransformDragPosition(x: Double(to.x) - factor * (Double(from.x) - x),
                                     y: Double(to.y) - factor * (Double(from.y) - y), scale: scale * factor)
    }
}

/// Facts are untransformed layout boxes in the clip's coordinate system plus
/// its mapping to window space. A changed origin invalidates an unchanged size.
struct TransformGeometryFacts: Equatable {
    let targetBounds: CGRect
    let targetFrame: CGRect
    let clipBounds: CGRect
    let windowOrigin: CGPoint
    var dimensions: [Double] {
        [Double(targetBounds.width), Double(targetBounds.height), Double(clipBounds.width), Double(clipBounds.height)]
    }
    var ready: Bool { dimensions.allSatisfy { $0 > 0 } }
    init?(targetBounds: CGRect, targetFrame: CGRect, clipBounds: CGRect,
          windowOrigin: CGPoint, supportedAncestors: Bool) {
        let numbers = [targetBounds.minX, targetBounds.minY, targetBounds.width, targetBounds.height,
                       targetFrame.minX, targetFrame.minY, targetFrame.width, targetFrame.height,
                       clipBounds.minX, clipBounds.minY, clipBounds.width, clipBounds.height,
                       windowOrigin.x, windowOrigin.y]
        guard supportedAncestors, numbers.allSatisfy({ $0.isFinite && abs($0) <= CGFloat(Float.greatestFiniteMagnitude) }),
              targetBounds.origin == .zero, clipBounds.origin == .zero,
              targetBounds.width >= 0, targetBounds.height >= 0,
              targetFrame == clipBounds, targetBounds.size == clipBounds.size else { return nil }
        self.targetBounds = targetBounds; self.targetFrame = targetFrame
        self.clipBounds = clipBounds; self.windowOrigin = windowOrigin
    }
}

extension TransformDragPosition {
    /// NodeView's centered transform is T(x,y) * T(center) * S * T(-center).
    /// Reject skew/rotation/nonuniform or nonpositive scale; never reconstruct
    /// catch from authored targets. The native compositor uses float values.
    init?(matrix: CGAffineTransform, center: CGPoint) {
        guard matrix.b == 0, matrix.c == 0, matrix.a == matrix.d,
              matrix.a.isFinite, matrix.a > 0 else { return nil }
        let x = Double(matrix.tx - center.x * (1 - matrix.a))
        let y = Double(matrix.ty - center.y * (1 - matrix.a))
        self.init(x: Double(Float(x)), y: Double(Float(y)), scale: Double(Float(matrix.a)))
    }
}
