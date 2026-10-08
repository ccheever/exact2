// @ref llp/1107.004-home-projection.decision.md#decision
// Wire values only; no native geometry or transport ownership.
import Foundation

enum T3HomeSwipePacket {
    static func token(_ value: String) -> Bool { !value.isEmpty && !value.contains("|") && !value.contains(where: { $0.isWhitespace || $0.isNewline }) }
    static func row(owner: String, row: String, gesture: Int, sequence: Int, phase: String,
                    x: Double, y: Double, vx: Double, vy: Double, width: Double, height: Double) -> String? {
        let values = [x, y, vx, vy, width, height]
        guard token(owner), token(row), gesture > 0, sequence >= 0,
              ["begin", "change", "end", "cancel"].contains(phase), values.allSatisfy(\.isFinite), width > 0, height > 0 else { return nil }
        return (["1", owner, row, String(gesture), String(sequence), phase] + values.map { String($0) }).joined(separator: "|")
    }
}

#if os(iOS)
struct T3HomeSwipeRowIdentity: Equatable {
    let owner: String, row: String, endpoint: String
    let enabled: Bool
    init?(_ element: ExactElement) {
        guard let owner = element.data[.mobileSwipeOwner], T3HomeSwipePacket.token(owner),
              let row = element.data[.mobileSwipeRow], T3HomeSwipePacket.token(row),
              let json = element.data[.mobileHomeMenu], let data = json.data(using: .utf8),
              let context = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let origin = context["origin"] as? String, let generation = context["generation"] as? Int,
              let requestRoute = context["requestRoute"] as? String, let environment = context["environmentId"] as? String,
              let thread = context["threadId"] as? String,
              let signature = try? JSONSerialization.data(withJSONObject: [origin, generation, requestRoute, environment, thread]) else { return nil }
        self.owner = owner; self.row = row; endpoint = String(decoding: signature, as: UTF8.self)
        enabled = element.data[.mobileSwipeEnabled] == "true" && context["connected"] as? Bool == true
    }
}
#endif
