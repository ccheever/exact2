#if os(iOS)
// Pinned365aa87982 use-thread-outbox-drain: flush destination before queue removal.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import Foundation

enum T3OutboxDraftHandoff {
    typealias Object = [String: Any]
    private static func fields(_ value: Object, _ keys: [String]) -> Bool { Set(value.keys) == Set(keys) }
    static func valid(_ value: Object, id: String, receipt: Object?) -> Bool {
        guard fields(value, ["version", "operationId", "receiptRevision", "record", "request", "draftKey", "draft"]),
              T3OutboxDeliveryReceipt.integer(value["version"], positive: true), value["version"] as? Int == 1,
              value["operationId"] as? String == id, let receipt, receipt["kind"] as? String == "outbox",
              receipt["stage"] as? String == "start-turn", receipt["operationId"] as? String == id,
              T3OutboxDeliveryReceipt.integer(value["receiptRevision"], positive: true),
              value["receiptRevision"] as? Int == receipt["revision"] as? Int,
              let record = value["record"] as? Object, T3MobileOutbox.validateRecord(record),
              let original = receipt["record"] as? Object,
              ["origin", "environmentId", "threadId", "messageId", "commandId", "createdAt"].allSatisfy({ T3MobileOutbox.jsonEqual(record[$0], original[$0]) }),
              let request = value["request"] as? Object,
              fields(request, ["ownerEpoch", "mutationId", "messageId", "operation", "expectedToken", "expectedRevision", "requireUnheld"]),
              let epoch = request["ownerEpoch"] as? String, !epoch.isEmpty, let mutation = request["mutationId"] as? String,
              mutation.hasPrefix(epoch + ":"), let sequence = Int(mutation.dropFirst(epoch.count + 1)), sequence > 0,
              sequence <= 9_007_199_254_740_991, mutation == epoch + ":" + String(sequence),
              request["operation"] as? String == "remove", request["requireUnheld"] as? Bool == false,
              let token = request["expectedToken"] as? String, !token.isEmpty,
              T3OutboxDeliveryReceipt.integer(request["expectedRevision"], positive: true),
              request["messageId"] as? String == record["messageId"] as? String,
              T3MobileOutbox.validateMutation(request, id: record["messageId"] as! String),
              let key = value["draftKey"] as? String, let draft = value["draft"] as? Object,
              fields(draft, ["text", "images", "files", "metadata", "staged", "workspace", "order", "recovered"]),
              draft["text"] is String, let images = draft["images"] as? [Object], let files = draft["files"] as? [Object],
              draft["order"] is [String], draft["recovered"] is [String: Object],
              ["metadata", "staged", "workspace"].allSatisfy({ draft[$0] is NSNull || draft[$0] is Object }),
              JSONSerialization.isValidJSONObject(value) else { return false }
        let rejected = receipt["state"] as? String == "rejected"
        guard rejected || receipt["state"] as? String == "acknowledged" && original["creation"] is Object
            && (receipt["cleanup"] as? Object)?["phase"] as? String == "edited" else { return false }
        let expected = rejected && record["creation"] is Object ? "new-task:restored-" + (record["messageId"] as! String)
            : (record["environmentId"] as! String) + ":" + (record["threadId"] as! String)
        guard key == expected, files.allSatisfy({ $0["draftKey"] as? String == key }) else { return false }
        if rejected, let creation = record["creation"] as? Object {
            guard let metadata = draft["metadata"] as? Object, metadata["key"] as? String == key,
                  ["origin", "environmentId", "createdAt"].allSatisfy({ T3MobileOutbox.jsonEqual(metadata[$0], record[$0]) }),
                  metadata["projectId"] as? String == creation["projectId"] as? String else { return false }
        }
        // The destination must own every recovered local file before the old receipt can release it.
        return (record["attachments"] as! [Object]).allSatisfy { attachment in
            let candidates = attachment["kind"] as? String == "image" ? images : files
            return candidates.contains { ($0["id"] as? String)?.lowercased() == (attachment["id"] as! String).lowercased() }
        }
    }
    static func projection(_ preferences: Object, key: String) -> Object {
        let controls = preferences["composerControls"] as? Object ?? [:]
        let metadata = (preferences["mobileNewTaskDrafts"] as? Object)?["records"] as? [String: Object] ?? [:]
        let recovered = (preferences["mobileRecoveredDrafts"] as? [String: Object] ?? [:]).filter { $0.value["key"] as? String == key }
        return ["text": (preferences["drafts"] as? Object)?[key] ?? "",
            "images": (preferences["snapshotDrafts"] as? Object)?[key] ?? [Object](),
            "files": (preferences["composerFiles"] as? [Object] ?? []).filter { $0["draftKey"] as? String == key },
            "metadata": metadata[key].map { $0 as Any } ?? NSNull(),
            "staged": (controls["staged"] as? Object)?[key] ?? NSNull(),
            "workspace": (controls["contexts"] as? Object)?[key] ?? NSNull(),
            "order": (preferences["mobileAttachmentOrder"] as? Object)?[key] ?? [String](), "recovered": recovered]
    }
    static func matchesPreferences(_ handoff: Object, preferences: Object) -> Bool {
        guard preferences["version"] as? Int == 1, let id = handoff["operationId"] as? String,
              let key = handoff["draftKey"] as? String,
              let marker = (preferences["mobileOutboxDraftHandoffs"] as? [String: Object])?[id] else { return false }
        return T3MobileOutbox.jsonEqual(marker, handoff) && T3MobileOutbox.jsonEqual(projection(preferences, key: key), handoff["draft"])
    }
}
#endif
