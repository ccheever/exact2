#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// A local assets-stage identity, distinct from the original server command identity.
import Foundation

enum T3OutboxInlineAdmission {
    case existing([String: Any])
    case preparing(UUID)
}

enum T3OutboxInlineReceipt {
    typealias Object = [String: Any]
    private static let keys: Set<String> = ["kind", "operationId", "revision", "origin", "environmentId", "messageId", "threadId", "rowToken", "rowRevision", "record", "template", "attachmentIDs", "state"]
    private static func failure() -> T3Failure {
        T3Failure(kind: "Outbox", message: "The inline reservation does not match its captured queued message.")
    }
    private static func detached(_ value: Object) throws -> Object {
        guard JSONSerialization.isValidJSONObject(value) else { throw failure() }
        let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
        guard data.count <= T3Wire.maximumBytes, let result = try JSONSerialization.jsonObject(with: data) as? Object else { throw failure() }
        return result
    }
    private static func identity(_ value: Object, captured: Bool) -> Bool {
        guard Set(value.keys) == keys, value["kind"] as? String == "outbox-inline",
              let id = value["operationId"] as? String, UUID(uuidString: id)?.uuidString.lowercased() == id,
              let record = value["record"] as? Object, T3MobileOutbox.validateRecord(record), id != record["commandId"] as? String,
              ["origin", "environmentId", "messageId", "threadId"].allSatisfy({ T3MobileOutbox.jsonEqual(value[$0], record[$0]) }),
              let token = value["rowToken"] as? String, !token.isEmpty,
              T3OutboxDeliveryReceipt.integer(value["rowRevision"], positive: true),
              let ids = value["attachmentIDs"] as? [String],
              ids == (record["attachments"] as? [Object] ?? []).compactMap({ $0["id"] as? String }),
              let template = value["template"] as? Object else { return false }
        do { _ = try T3OutboxInlineTemplate.validateMetadata(record, template, captured: captured); return true }
        catch { return false }
    }
    static func valid(_ value: Object, id: String) -> Bool {
        guard value["operationId"] as? String == id, identity(value, captured: true),
              T3OutboxDeliveryReceipt.integer(value["revision"], positive: true) else { return false }
        return value["state"] as? String == "reserved" && value["revision"] as? Int == 1
            || value["state"] as? String == "retired" && value["revision"] as? Int == 2
    }
    static func prepare(_ request: Object, origin: String, environment: String) throws -> Object {
        guard let record = request["record"] as? Object else { throw failure() }
        let value: Object = ["kind": "outbox-inline", "operationId": request["operationId"] ?? NSNull(), "revision": 1,
            "origin": origin, "environmentId": environment, "messageId": request["messageId"] ?? NSNull(),
            "threadId": record["threadId"] ?? NSNull(), "rowToken": request["expectedToken"] ?? NSNull(),
            "rowRevision": request["expectedRevision"] ?? NSNull(), "record": record,
            "template": request["template"] ?? NSNull(), "attachmentIDs": (record["attachments"] as? [Object] ?? []).compactMap { $0["id"] as? String }, "state": "reserved"]
        guard identity(value, captured: false) else { throw failure() }
        return try detached(value)
    }
    static func uncaptured(_ captured: Object) -> Object {
        var value = captured
        value["inline"] = (captured["inline"] as? [Object] ?? []).map { binding -> Object in
            var binding = binding; binding.removeValue(forKey: "sha256"); return binding
        }
        return value
    }
    static func sameInput(_ saved: Object, _ prepared: Object) -> Bool {
        var saved = saved
        guard let template = saved["template"] as? Object else { return false }
        saved["template"] = uncaptured(template)
        let mutable: Set<String> = ["revision", "state"]
        return T3MobileOutbox.jsonEqual(saved.filter { !mutable.contains($0.key) }, prepared.filter { !mutable.contains($0.key) })
    }
    static func captured(_ prepared: Object, template: Object) throws -> Object {
        guard T3MobileOutbox.jsonEqual(uncaptured(template), prepared["template"]) else { throw failure() }
        var value = prepared; value["template"] = template
        guard valid(value, id: value["operationId"] as! String) else { throw failure() }
        return try detached(value)
    }
}
#endif
