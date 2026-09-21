import CExact
import Foundation

struct TransformDragReply: Decodable {
    let accepted: Bool
    let committed: Bool
    let runtime: UInt64?
    let sequence: UInt64?
    let translate: UInt64?
    let scale: UInt64?
    let value: TransformDragPosition?
    let batch: Batch

    init?(_ data: Data) {
        guard let reply = try? JSONDecoder().decode(Self.self, from: data) else { return nil }
        self = reply
    }
    enum CodingKeys: String, CodingKey {
        case accepted, committed, runtime, geometrySequence, translateToken, scaleToken, value, batch
    }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        func key(_ name: CodingKeys) throws -> UInt64? { try c.decodeIfPresent(String.self, forKey: name).flatMap(UInt64.init) }
        accepted = try c.decodeIfPresent(Bool.self, forKey: .accepted) ?? false
        committed = try c.decodeIfPresent(Bool.self, forKey: .committed) ?? false
        runtime = try key(.runtime); sequence = try key(.geometrySequence)
        translate = try key(.translateToken); scale = try key(.scaleToken)
        value = try c.decodeIfPresent([Double].self, forKey: .value).flatMap(TransformDragPosition.init)
        batch = try c.decode(Batch.self, forKey: .batch)
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
