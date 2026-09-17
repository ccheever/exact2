// Exact u64 identities are decimal strings; JSON owns every copied byte.
import Foundation
import CoreGraphics

struct RegionFrame: Sendable {
    let key: UInt64
    let id: UInt32
    let kind: String
    let box: CGRect
    let extent: CGSize
    let artifact: UInt64
    // Ordinary TextSelection uses vertical distance first, then horizontal.
    // A giant paragraph contains its early lines even when its midpoint is far.
    func hitDistance(_ point: CGPoint) -> CGFloat {
        let dy = max(0, max(box.minY - point.y, point.y - box.maxY))
        let dx = max(0, max(box.minX - point.x, point.x - box.maxX))
        return dy * 10000 + dx
    }
}
struct RegionSnapshot {
    let incarnation: UInt64
    let owner: UInt32
    let content: UInt32
    let placeholder: UInt32
    let request: UInt64
    let source: UInt64
    let publication: UInt64
    let current: Bool
    let frames: [RegionFrame]
    static func uint(_ value: Any?) -> UInt64? { (value as? String).flatMap(UInt64.init) }
    init?(_ op: [String: Any]) {
        guard op["op"] as? String == "region", let incarnation = Self.uint(op["incarnation"]),
              let owner = op["owner"] as? UInt32, let content = op["content"] as? UInt32,
              let pending = op["pending"] as? UInt32, let request = Self.uint(op["request"]),
              let source = Self.uint(op["source"]), let publication = Self.uint(op["publication"]),
              let rows = op["frames"] as? [[String: Any]], rows.count <= 4096 else { return nil }
        var frames: [RegionFrame] = []
        for row in rows {
            guard let key = Self.uint(row["key"]), let id = row["id"] as? UInt32,
                  let kind = row["kind"] as? String, let box = row["box"] as? [Double], box.count == 4,
                  let extent = row["extent"] as? [Double], extent.count == 2,
                  (box + extent).allSatisfy(\.isFinite), box[2] >= 0, box[3] >= 0,
                  let artifact = Self.uint(row["artifact"]) else { return nil }
            frames.append(RegionFrame(key: key, id: id, kind: kind,
                box: CGRect(x: box[0], y: box[1], width: box[2], height: box[3]),
                extent: CGSize(width: extent[0], height: extent[1]), artifact: artifact))
        }
        self.incarnation = incarnation; self.owner = owner; self.content = content; placeholder = pending
        self.request = request; self.source = source; self.publication = publication
        current = op["current"] as? Bool ?? false; self.frames = frames
    }
}
struct RegionShapeRequest: Sendable {
    let id: UInt64
    let sourceID: UInt64
    let source: RegionTextSource
    let width: CGFloat
    let height: CGFloat
    let generation: Int
}
extension RegionTextSource {
    static func capture(wire: [String: Any], engine: TextEngine, dark: Bool) -> RegionTextSource? {
        guard let rows = wire["runs"] as? [[String: Any]], rows.count <= 4096,
              let strutRow = wire["strut"] as? [String: Any] else { return nil }
        func run(_ row: [String: Any]) -> Run? {
            guard let size = row["size"] as? Double, size.isFinite, size >= 0,
                  let weight = row["weight"] as? Int, let family = row["family"] as? Int,
                  let spacing = row["spacing"] as? Double, spacing.isFinite else { return nil }
            return Run(text: row["text"] as? String ?? "", size: size, weight: weight, family: family,
                italic: row["italic"] as? Bool ?? false,
                lineHeight: (row["lineHeight"] as? Double).map { CGFloat($0) }, letterSpacing: spacing,
                color: row[dark ? "darkColor" : "color"] as? [Double],
                decoration: row["decoration"] as? String ?? "", href: row["href"] as? String ?? "")
        }
        let runs = rows.compactMap(run)
        guard runs.count == rows.count, let strut = run(strutRow),
              runs.reduce(0, { $0 + $1.text.utf8.count }) <= 8 * 1024 * 1024 else { return nil }
        return capture(Spec(runs: runs, align: wire["align"] as? Int ?? 0,
            lineClamp: wire["lineClamp"] as? Int ?? 0, color: [0,0,0,255],
            overflowWrap: wire["overflowWrap"] as? Int ?? 0, strut: strut), engine: engine)
    }
}
