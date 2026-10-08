#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// Native invocation value, never a journal or JS reply. No caller-supplied digest constructor.
import Foundation
import CryptoKit

struct T3OutboxInlinePayload {
    let token: UUID
    let payload: [String: Any]
    let digest: String
    private init(token: UUID, payload: [String: Any], digest: String) {
        self.token = token; self.payload = payload; self.digest = digest
    }
    static func digest(method: String, payload: Any) throws -> String {
        let bytes = try encoded(method: method, payload: payload)
        return SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
    }
    private static func encoded(method: String, payload: Any) throws -> Data {
        let value: [String: Any] = ["method": method, "payload": payload]
        guard JSONSerialization.isValidJSONObject(value) else {
            throw T3Failure(kind: "Outbox", message: "The prepared inline payload is invalid.")
        }
        return try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .withoutEscapingSlashes])
    }
    static func expand(token: UUID, root: URL, record: [String: Any], captured: [String: Any]) throws -> T3OutboxInlinePayload {
        let payload = try T3OutboxInlineTemplate.expand(root: root, record: record, captured: captured)
        let bytes = try encoded(method: "assets.persistChatAttachments", payload: payload)
        // Detach every nested container before handing this value to transport encoding.
        let detached = try JSONSerialization.jsonObject(with: bytes) as! [String: Any]
        let digest = SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
        return T3OutboxInlinePayload(token: token, payload: detached["payload"] as! [String: Any], digest: digest)
    }
}

enum T3OutboxInlineSendAdmission {
    case acknowledged([String: Any])
    case preparing(UUID)
}
#endif
