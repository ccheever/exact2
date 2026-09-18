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
    let batch: Batch

    init?(_ data: Data) {
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let batch = object["batch"] as? [String: Any] else { return nil }
        func key(_ name: String) -> UInt64? { (object[name] as? String).flatMap(UInt64.init) }
        accepted = object["accepted"] as? Bool == true
        committed = object["committed"] as? Bool == true
        runtime = key("runtime"); sequence = key("geometrySequence")
        translate = key("translateToken"); scale = key("scaleToken")
        value = (object["value"] as? [Double]).flatMap(TransformDragPosition.init)
        self.batch = Batch(ops: batch["ops"] as? [[String: Any]] ?? [],
            timers: batch["timers"] as? Bool ?? false, motion: batch["motion"] as? Bool ?? false,
            clock: batch["clock"] as? Double, error: batch["error"] as? String,
            pending: batch["pending"] as? Bool ?? false)
    }
}

extension Runtime {
    func transformMotion(_ packet: TransformDragPacket) -> TransformDragReply? {
        guard !destroyed, let bytes = packet.encoded() else { return nil }
        let length = write(bytes)
        let count = exact_transform_motion(rt, UInt32(length))
        return TransformDragReply(Data(bytes: exact_out(rt), count: Int(count)))
    }
}
