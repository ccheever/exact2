import CExact
import Foundation

struct TransformDragReply {
    let accepted: Bool
    let committed: Bool
    let runtime: UInt64?
    let sequence: UInt64?
    let translate: UInt64?
    let scale: UInt64?
    let value: TransformDragPosition?
    /// Op 13's release velocity, measured by the engine: [vx, vy, vscale].
    let velocity: [Double]?
    let batch: Batch

    init?(_ data: Data) {
        var accepted = false, committed = false
        var runtime: UInt64?, sequence: UInt64?, translate: UInt64?, scale: UInt64?
        var value: TransformDragPosition?, batch: Batch?, velocity: [Double]?
        do {
            try data.withUnsafeBytes { bytes in
                var reader = BatchReader(bytes: bytes.bindMemory(to: UInt8.self))
                try reader.object { r, key in
                    if key == "batch" { batch = try r.batch(); return }
                    if try r.null() { return }
                    switch key {
                    case "accepted": accepted = try r.bool()
                    case "committed": committed = try r.bool()
                    case "runtime": runtime = UInt64(try r.string())
                    case "geometrySequence": sequence = UInt64(try r.string())
                    case "translateToken": translate = UInt64(try r.string())
                    case "scaleToken": scale = UInt64(try r.string())
                    case "value": value = TransformDragPosition(try r.array { try $0.number() })
                    case "velocity": velocity = try r.array { try $0.number() }
                    default: try r.skip()
                    }
                }
                try reader.end()
            }
        } catch { return nil }
        guard let batch else { return nil }
        self.accepted = accepted; self.committed = committed
        self.runtime = runtime; self.sequence = sequence; self.translate = translate; self.scale = scale
        self.value = value; self.batch = batch
        self.velocity = velocity.flatMap { $0.count == 3 && $0.allSatisfy(\.isFinite) ? $0 : nil }
    }

}

extension Runtime {
    /// One paired packet, as an owner job like every other call (LLP 1072
    /// T1/T2): the runtimes live in the owner thread's registry, and a call
    /// from main found none ("no such runtime"), so every transform drag
    /// was refused at its first geometry report.
    func transformMotion(_ packet: TransformDragPacket) -> TransformDragReply? {
        guard !destroyed, let bytes = packet.encoded() else { return nil }
        return on(busy: nil) {
            let length = write(bytes)
            let count = exact_transform_motion(rt, UInt32(length))
            return TransformDragReply(Data(bytes: exact_out(rt), count: Int(count)))
        }
    }
}
